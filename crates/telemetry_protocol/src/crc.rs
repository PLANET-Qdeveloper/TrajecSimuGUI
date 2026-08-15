//! CRC-16/CCITT-FALSE, used to protect the PLANET-Q common frame payload.
//!
//! Polynomial `0x1021`, initial value `0xFFFF`, no input/output reflection,
//! xorout `0x0000`. See `ref/NSE2026_PROTOCOL.md`.

/// Computes CRC-16/CCITT-FALSE over `data`.
pub fn crc16_ccitt_false(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_check_value() {
        // Spec: the CRC of ASCII "123456789" is 0x29B1.
        assert_eq!(crc16_ccitt_false(b"123456789"), 0x29B1);
    }
}
