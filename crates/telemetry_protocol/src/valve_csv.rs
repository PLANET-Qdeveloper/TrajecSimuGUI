//! Valve-board telemetry: CRLF-terminated ASCII CSV.
//!
//! ```text
//! <time>[s],<phase>,<pressure>[kPa],<voltage>[V],<current>[A],<temperature>[deg],<nos>[NOS]\r\n
//! ```
//!
//! See `ref/NSE2026_PROTOCOL.md`, "バルブ基板テレメトリ".

/// Splits a raw CSV line into columns for the UI's user-configurable
/// index -> meaning mapping, stripping a single trailing `[unit]` suffix
/// from each column when present. This makes no assumption about which
/// column means what or how many columns exist —
/// the wire format's column *order* is fixed by the firmware, but which
/// index the UI treats as which telemetry field is user-configurable.
pub fn split_csv_columns(line: &str) -> Vec<String> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    trimmed
        .split(',')
        .map(|raw| match raw.rfind('[') {
            Some(open) if raw.ends_with(']') => raw[..open].to_string(),
            _ => raw.to_string(),
        })
        .collect()
}

/// Accumulates raw bytes into a persistent buffer and yields complete
/// (`\n`-terminated) lines as they arrive, tolerating reads that split or
/// concatenate lines arbitrarily. A capacity bound prevents unbounded
/// growth if no newline ever arrives (spec recommends ~256-1024 bytes).
pub struct ValveLineBuffer {
    buf: Vec<u8>,
    max_len: usize,
}

impl ValveLineBuffer {
    pub fn new(max_len: usize) -> Self {
        Self {
            buf: Vec::new(),
            max_len,
        }
    }

    /// Feeds bytes and returns any complete lines (trailing `\n` stripped).
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            lines.push(String::from_utf8_lossy(&line[..line.len() - 1]).into_owned());
        }
        if self.buf.len() > self.max_len {
            // No newline within the cap: drop the stale fragment so memory
            // doesn't grow unbounded, and resynchronize on the next line.
            self.buf.clear();
        }
        lines
    }
}

impl Default for ValveLineBuffer {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_columns_and_strips_unit_suffix() {
        let cols =
            split_csv_columns("12[s],VALVE_OPEN,145[kPa],11.92[V],0.42[A],24.10[deg],0[NOS]\r\n");
        assert_eq!(
            cols,
            vec!["12", "VALVE_OPEN", "145", "11.92", "0.42", "24.10", "0"]
        );
    }

    #[test]
    fn line_buffer_handles_split_and_concatenated_reads() {
        let mut buf = ValveLineBuffer::default();
        let lines = buf.feed(b"12[s],VALVE_OPEN,145[kPa],11.92[V],0.42[A],24.10[deg],0[NOS]\r\n1");
        assert_eq!(lines.len(), 1);

        let mut buf2 = ValveLineBuffer::default();
        let lines = buf2.feed(b"a,b\r\nc,d\r\n");
        assert_eq!(lines, vec!["a,b\r".to_string(), "c,d\r".to_string()]);

        let mut buf3 = ValveLineBuffer::default();
        assert!(buf3.feed(b"a,b").is_empty());
        let lines = buf3.feed(b"\r\n");
        assert_eq!(lines, vec!["a,b\r".to_string()]);
    }

    #[test]
    fn line_buffer_drops_stale_fragment_past_cap() {
        let mut buf = ValveLineBuffer::new(8);
        let lines = buf.feed(b"01234567890123");
        assert!(lines.is_empty());
        // Next newline-terminated line should parse cleanly, proving the
        // buffer resynchronized instead of growing forever.
        let lines = buf.feed(b"x,y\n");
        assert_eq!(lines, vec!["x,y".to_string()]);
    }
}
