//! Main-board telemetry payload: a packed TLV (`[Tag:1][Length:1][Value]`)
//! sequence. Decoding walks Tag/Length pairs in order rather than assuming
//! fixed offsets, per the spec's recommendation, so unknown/reordered tags
//! degrade gracefully instead of desyncing the whole payload.

use serde::Serialize;
use std::collections::HashMap;

/// A decoded TLV value, typed per the known-tag table below. Unknown tags
/// are kept as raw bytes so callers can still count/inspect them.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum DecodedValue {
    F32(f32),
    U8(u8),
    U32(u32),
    Raw(Vec<u8>),
}

impl DecodedValue {
    /// Numeric representation for display/plotting, when applicable.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            DecodedValue::F32(v) => Some(*v as f64),
            DecodedValue::U8(v) => Some(*v as f64),
            DecodedValue::U32(v) => Some(*v as f64),
            DecodedValue::Raw(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueKind {
    F32,
    U8,
    U32,
}

/// The current main-board TLV tag table (`ref/NSE2026_PROTOCOL.md`).
const KNOWN_TAGS: &[(u8, ValueKind)] = &[
    (0xA1, ValueKind::F32),
    (0xA2, ValueKind::F32),
    (0xA3, ValueKind::F32),
    (0xA4, ValueKind::F32),
    (0xA5, ValueKind::F32),
    (0xA6, ValueKind::U8),
    (0xA7, ValueKind::U8),
    (0xA8, ValueKind::U32),
    (0xA9, ValueKind::F32),
    (0xAA, ValueKind::F32),
    (0xAB, ValueKind::F32),
    (0xAC, ValueKind::F32),
    (0xAD, ValueKind::F32),
    (0xAE, ValueKind::U8),
    (0xAF, ValueKind::U8),
    (0xB0, ValueKind::U8),
];

fn known_kind(tag: u8) -> Option<ValueKind> {
    KNOWN_TAGS
        .iter()
        .find(|(t, _)| *t == tag)
        .map(|(_, kind)| *kind)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlvError {
    Truncated,
    LengthMismatch {
        tag: u8,
        declared: usize,
        actual: usize,
    },
    DuplicateTag(u8),
}

/// Result of decoding a TLV payload: values keyed by tag, plus counts for
/// the spec's recommended monitoring counters.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TlvDecodeResult {
    pub values: HashMap<u8, DecodedValue>,
    pub unknown_tags: u32,
}

/// Decodes a TLV payload, walking Tag/Length pairs sequentially. Known tags
/// are parsed to their typed value; unknown tags are skipped by `Length`
/// and counted. A length mismatch on a *known* tag (value runs past the end
/// of the payload) is an error, per spec ("既知TagのLength不一致...はエラー
/// にすべきです").
pub fn decode_main_payload(payload: &[u8]) -> Result<TlvDecodeResult, TlvError> {
    let mut out = TlvDecodeResult::default();
    let mut i = 0usize;
    while i < payload.len() {
        if i + 2 > payload.len() {
            return Err(TlvError::Truncated);
        }
        let tag = payload[i];
        let len = payload[i + 1] as usize;
        let value_start = i + 2;
        let value_end = value_start + len;
        if value_end > payload.len() {
            return Err(TlvError::Truncated);
        }
        let raw = &payload[value_start..value_end];

        if out.values.contains_key(&tag) {
            return Err(TlvError::DuplicateTag(tag));
        }

        match known_kind(tag) {
            Some(ValueKind::F32) => {
                if len != 4 {
                    return Err(TlvError::LengthMismatch {
                        tag,
                        declared: len,
                        actual: 4,
                    });
                }
                let bytes: [u8; 4] = raw.try_into().unwrap();
                out.values
                    .insert(tag, DecodedValue::F32(f32::from_le_bytes(bytes)));
            }
            Some(ValueKind::U8) => {
                if len != 1 {
                    return Err(TlvError::LengthMismatch {
                        tag,
                        declared: len,
                        actual: 1,
                    });
                }
                out.values.insert(tag, DecodedValue::U8(raw[0]));
            }
            Some(ValueKind::U32) => {
                if len != 4 {
                    return Err(TlvError::LengthMismatch {
                        tag,
                        declared: len,
                        actual: 4,
                    });
                }
                let bytes: [u8; 4] = raw.try_into().unwrap();
                out.values
                    .insert(tag, DecodedValue::U32(u32::from_le_bytes(bytes)));
            }
            None => {
                out.unknown_tags += 1;
                out.values.insert(tag, DecodedValue::Raw(raw.to_vec()));
            }
        }

        i = value_end;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::encode_frame;

    /// The complete 81-byte payload example from the spec.
    fn spec_payload() -> Vec<u8> {
        vec![
            0xA1, 0x04, 0x00, 0x00, 0x80, 0x3F, 0xA2, 0x04, 0x00, 0x00, 0x00, 0x40, 0xA3, 0x04,
            0x00, 0x00, 0x40, 0x40, 0xA4, 0x04, 0x00, 0x00, 0x80, 0x40, 0xA5, 0x04, 0x00, 0x00,
            0xA0, 0x40, 0xA6, 0x01, 0x01, 0xA7, 0x01, 0x02, 0xA8, 0x04, 0x04, 0x03, 0x02, 0x01,
            0xA9, 0x04, 0x00, 0x00, 0xC0, 0x40, 0xAA, 0x04, 0x00, 0x00, 0xE0, 0x40, 0xAB, 0x04,
            0x00, 0x00, 0x00, 0x41, 0xAC, 0x04, 0x00, 0x00, 0x10, 0x41, 0xAD, 0x04, 0x00, 0x00,
            0x20, 0x41, 0xAE, 0x01, 0x0B, 0xAF, 0x01, 0x01, 0xB0, 0x01, 0x00,
        ]
    }

    #[test]
    fn decodes_full_spec_example() {
        let payload = spec_payload();
        assert_eq!(payload.len(), 81);
        let result = decode_main_payload(&payload).expect("decode ok");
        assert_eq!(result.unknown_tags, 0);
        assert_eq!(result.values.len(), 16);
        assert_eq!(result.values[&0xA1].as_f64(), Some(1.0));
        assert_eq!(result.values[&0xA2].as_f64(), Some(2.0));
        assert_eq!(result.values[&0xA3].as_f64(), Some(3.0));
        assert_eq!(result.values[&0xA4].as_f64(), Some(4.0));
        assert_eq!(result.values[&0xA5].as_f64(), Some(5.0));
        assert_eq!(result.values[&0xA6], DecodedValue::U8(1));
        assert_eq!(result.values[&0xA7], DecodedValue::U8(2));
        assert_eq!(result.values[&0xA8], DecodedValue::U32(0x01020304));
        assert_eq!(result.values[&0xA9].as_f64(), Some(6.0));
        assert_eq!(result.values[&0xAA].as_f64(), Some(7.0));
        assert_eq!(result.values[&0xAB].as_f64(), Some(8.0));
        assert_eq!(result.values[&0xAC].as_f64(), Some(9.0));
        assert_eq!(result.values[&0xAD].as_f64(), Some(10.0));
        assert_eq!(result.values[&0xAE], DecodedValue::U8(11));
        assert_eq!(result.values[&0xAF], DecodedValue::U8(1));
        assert_eq!(result.values[&0xB0], DecodedValue::U8(0));
    }

    #[test]
    fn full_wire_frame_matches_spec_crc_and_decodes() {
        let payload = spec_payload();
        let wire = encode_frame(0x0B, 0x0A, &payload);
        // Spec: CRC is D13F and the frame contains no reserved bytes, so
        // stuffing is a no-op and the full frame is 88 bytes.
        assert_eq!(wire.len(), 88);
        assert_eq!(wire[wire.len() - 3], 0xD1);
        assert_eq!(wire[wire.len() - 2], 0x3F);

        let mut decoder = crate::frame::FrameDecoder::new();
        let frames = decoder.feed(&wire);
        assert_eq!(frames.len(), 1);
        let result = decode_main_payload(&frames[0].payload).unwrap();
        assert_eq!(result.values.len(), 16);
    }

    #[test]
    fn unknown_tag_is_skipped_and_counted() {
        // Known tag A6 (phase, u8) followed by an unknown tag 0xFF with a
        // 3-byte value, followed by another known tag AE (num_sats, u8).
        let payload = [0xA6, 0x01, 0x07, 0xFF, 0x03, 9, 9, 9, 0xAE, 0x01, 0x05];
        let result = decode_main_payload(&payload).unwrap();
        assert_eq!(result.unknown_tags, 1);
        assert_eq!(result.values[&0xA6], DecodedValue::U8(7));
        assert_eq!(result.values[&0xAE], DecodedValue::U8(5));
    }

    #[test]
    fn length_mismatch_on_known_tag_is_an_error() {
        // A6 (phase) declared with length 4 instead of 1.
        let payload = [0xA6, 0x04, 1, 2, 3, 4];
        assert!(matches!(
            decode_main_payload(&payload),
            Err(TlvError::LengthMismatch { tag: 0xA6, .. })
        ));
    }

    #[test]
    fn duplicate_tag_is_an_error() {
        let payload = [0xA6, 0x01, 1, 0xA6, 0x01, 2];
        assert_eq!(
            decode_main_payload(&payload),
            Err(TlvError::DuplicateTag(0xA6))
        );
    }
}
