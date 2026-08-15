//! NSE2026 serial telemetry protocol implementation: CRC-16/CCITT-FALSE,
//! PLANET-Q common frame byte-stuffing/decoding, main-board TLV decoding,
//! valve-board CSV parsing, and a pressure->altitude helper.
//!
//! See `ref/NSE2026_PROTOCOL.md` for the wire-format specification this
//! module implements against, including all test vectors used below.

pub mod altitude;
pub mod counters;
pub mod crc;
pub mod frame;
pub mod tlv;
pub mod valve_csv;

pub use altitude::pressure_to_altitude_m;
pub use counters::LinkCounters;
pub use crc::crc16_ccitt_false;
pub use frame::{encode_frame, Frame, FrameDecoder, FrameError};
pub use tlv::{decode_main_payload, DecodedValue, TlvDecodeResult, TlvError};
pub use valve_csv::{split_csv_columns, ValveLineBuffer};

/// Main board's logical frame IDs on the PLANET-Q common frame (not the
/// E22/E220 module's own 16-bit radio address).
pub const MAIN_DESTINATION_ID: u8 = 0x0B;
pub const MAIN_SOURCE_ID: u8 = 0x0A;
