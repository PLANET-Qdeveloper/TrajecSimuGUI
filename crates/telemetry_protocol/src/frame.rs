//! PLANET-Q common frame: `0x7E` start / `0x7F` end, `0x7D`-escaped byte
//! stuffing, CRC-16/CCITT-FALSE over the unstuffed payload only.
//!
//! See `ref/NSE2026_PROTOCOL.md` ("PLANET-Q共通フレーム").

use crate::crc::crc16_ccitt_false;

const START: u8 = 0x7E;
const END: u8 = 0x7F;
const ESC: u8 = 0x7D;
const ESC_START: u8 = 0x81;
const ESC_END: u8 = 0x80;
const ESC_ESC: u8 = 0x7D;
// Destination + Source + one-byte Length + the largest possible Payload + CRC.
const MAX_BODY_LEN: usize = 3 + u8::MAX as usize + 2;

/// A decoded (destination, source, payload) frame. `payload` is the
/// unstuffed TLV byte sequence with the CRC already verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub destination_id: u8,
    pub source_id: u8,
    pub payload: Vec<u8>,
}

/// Reasons a candidate frame was discarded. Callers use these to drive the
/// error counters described in the spec ("エラー処理と監視").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    TooShort,
    LengthMismatch,
    CrcMismatch,
}

/// Stuffs `body` (Destination, Source, Length, Payload, CRC-hi, CRC-lo, in
/// that order) and wraps it with unescaped start/end markers.
fn stuff(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 2);
    out.push(START);
    for &b in body {
        match b {
            START => out.extend_from_slice(&[ESC, ESC_START]),
            END => out.extend_from_slice(&[ESC, ESC_END]),
            ESC => out.extend_from_slice(&[ESC, ESC_ESC]),
            _ => out.push(b),
        }
    }
    out.push(END);
    out
}

/// Encodes a frame for transmission: computes the CRC over `payload`,
/// builds the body, and byte-stuffs it.
pub fn encode_frame(destination_id: u8, source_id: u8, payload: &[u8]) -> Vec<u8> {
    let crc = crc16_ccitt_false(payload);
    let mut body = Vec::with_capacity(3 + payload.len() + 2);
    body.push(destination_id);
    body.push(source_id);
    body.push(payload.len() as u8);
    body.extend_from_slice(payload);
    body.push((crc >> 8) as u8);
    body.push((crc & 0xFF) as u8);
    stuff(&body)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    WaitStart,
    InFrame,
    Escaped,
}

/// Streaming decoder. Feed it arbitrary chunks of received bytes (which may
/// split or concatenate frames arbitrarily) and it emits completed,
/// CRC-verified frames. Malformed frames are discarded and decoding
/// resynchronizes at the next `0x7E`.
pub struct FrameDecoder {
    state: State,
    body: Vec<u8>,
    errors: Vec<FrameError>,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self {
            state: State::WaitStart,
            body: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Feeds a chunk of bytes and returns all frames completed within it.
    /// Any frame-level errors encountered are recorded and can be drained
    /// with [`FrameDecoder::take_errors`].
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<Frame> {
        let mut frames = Vec::new();
        for &byte in chunk {
            match self.state {
                State::WaitStart => {
                    if byte == START {
                        self.body.clear();
                        self.state = State::InFrame;
                    }
                    // Other bytes (startup ASCII, noise) are discarded.
                }
                State::InFrame => {
                    if byte == ESC {
                        self.state = State::Escaped;
                    } else if byte == END {
                        match Self::validate(&self.body) {
                            Ok(frame) => frames.push(frame),
                            Err(e) => self.errors.push(e),
                        }
                        self.state = State::WaitStart;
                    } else if byte == START {
                        // Mid-frame restart: discard what we had and begin anew.
                        self.body.clear();
                        // Stay in InFrame.
                    } else {
                        self.body.push(byte);
                    }
                }
                State::Escaped => {
                    match byte {
                        ESC_START => self.body.push(START),
                        ESC_END => self.body.push(END),
                        ESC_ESC => self.body.push(ESC),
                        _ => {
                            // Invalid escape sequence: discard the in-progress
                            // frame and resynchronize. If the invalid escaped
                            // byte is itself START, reuse it as the boundary of
                            // the next candidate instead of consuming it.
                            self.body.clear();
                            self.state = if byte == START {
                                State::InFrame
                            } else {
                                State::WaitStart
                            };
                            continue;
                        }
                    }
                    self.state = State::InFrame;
                }
            }
            if self.body.len() > MAX_BODY_LEN {
                self.body.clear();
                self.state = State::WaitStart;
                self.errors.push(FrameError::LengthMismatch);
            }
        }
        frames
    }

    /// Drains and returns errors accumulated since the last call.
    pub fn take_errors(&mut self) -> Vec<FrameError> {
        std::mem::take(&mut self.errors)
    }

    fn validate(body: &[u8]) -> Result<Frame, FrameError> {
        if body.len() < 5 {
            return Err(FrameError::TooShort);
        }
        let destination_id = body[0];
        let source_id = body[1];
        let payload_length = body[2] as usize;
        if body.len() != 3 + payload_length + 2 {
            return Err(FrameError::LengthMismatch);
        }
        let payload = &body[3..3 + payload_length];
        let crc_hi = body[3 + payload_length] as u16;
        let crc_lo = body[3 + payload_length + 1] as u16;
        let received_crc = (crc_hi << 8) | crc_lo;
        let computed_crc = crc16_ccitt_false(payload);
        if received_crc != computed_crc {
            return Err(FrameError::CrcMismatch);
        }
        Ok(Frame {
            destination_id,
            source_id,
            payload: payload.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stuffing_example_from_spec() {
        // Destination 0B, Source 0A, payload 7E 7F 7D, CRC B8C8.
        let payload = [0x7E, 0x7F, 0x7D];
        let wire = encode_frame(0x0B, 0x0A, &payload);
        assert_eq!(
            wire,
            vec![0x7E, 0x0B, 0x0A, 0x03, 0x7D, 0x81, 0x7D, 0x80, 0x7D, 0x7D, 0xB8, 0xC8, 0x7F]
        );
    }

    #[test]
    fn round_trip_through_decoder() {
        let payload = [0x7E, 0x7F, 0x7D, 0x00, 0x01, 0x02];
        let wire = encode_frame(0x0B, 0x0A, &payload);
        let mut decoder = FrameDecoder::new();
        let frames = decoder.feed(&wire);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].destination_id, 0x0B);
        assert_eq!(frames[0].source_id, 0x0A);
        assert_eq!(frames[0].payload, payload);
    }

    #[test]
    fn splits_across_multiple_reads() {
        let payload = [1, 2, 3, 4, 5];
        let wire = encode_frame(0x0B, 0x0A, &payload);
        let mut decoder = FrameDecoder::new();
        let mut frames = Vec::new();
        for byte in &wire {
            frames.extend(decoder.feed(std::slice::from_ref(byte)));
        }
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, payload);
    }

    #[test]
    fn concatenated_frames_in_one_read() {
        let a = encode_frame(0x0B, 0x0A, &[1, 2, 3]);
        let b = encode_frame(0x0B, 0x0A, &[4, 5]);
        let mut wire = a.clone();
        wire.extend_from_slice(&b);
        let mut decoder = FrameDecoder::new();
        let frames = decoder.feed(&wire);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].payload, vec![1, 2, 3]);
        assert_eq!(frames[1].payload, vec![4, 5]);
    }

    #[test]
    fn crc_mismatch_is_reported_and_recovers() {
        let mut wire = encode_frame(0x0B, 0x0A, &[1, 2, 3]);
        // Corrupt a payload byte without touching the CRC.
        let corrupt_idx = wire.len() - 4;
        wire[corrupt_idx] ^= 0xFF;
        let good = encode_frame(0x0B, 0x0A, &[9, 9]);
        wire.extend_from_slice(&good);

        let mut decoder = FrameDecoder::new();
        let frames = decoder.feed(&wire);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, vec![9, 9]);
        assert_eq!(decoder.take_errors(), vec![FrameError::CrcMismatch]);
    }

    #[test]
    fn startup_ascii_is_ignored_before_start_marker() {
        let mut wire = b"BOOTED!\r\n[INIT_RESULT] ok\r\n".to_vec();
        wire.extend_from_slice(&encode_frame(0x0B, 0x0A, &[7]));
        let mut decoder = FrameDecoder::new();
        let frames = decoder.feed(&wire);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, vec![7]);
    }

    #[test]
    fn oversized_candidate_is_discarded_and_decoder_recovers() {
        let mut wire = vec![START];
        wire.extend(std::iter::repeat_n(0, MAX_BODY_LEN + 1));
        wire.extend_from_slice(&encode_frame(0x0B, 0x0A, &[7, 8]));

        let mut decoder = FrameDecoder::new();
        let frames = decoder.feed(&wire);

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, vec![7, 8]);
        assert_eq!(decoder.take_errors(), vec![FrameError::LengthMismatch]);
        assert!(decoder.body.len() <= MAX_BODY_LEN);
    }

    #[test]
    fn start_after_invalid_escape_begins_next_frame() {
        let good = encode_frame(0x0B, 0x0A, &[7, 8, 9]);
        let mut wire = vec![START, 0x01, ESC];
        wire.extend_from_slice(&good);

        let mut decoder = FrameDecoder::new();
        let frames = decoder.feed(&wire);

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, vec![7, 8, 9]);
    }
}
