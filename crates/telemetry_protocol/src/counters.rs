//! Per-connection monitoring counters, per the spec's "エラー処理と監視"
//! recommendations. One [`LinkCounters`] is kept per open serial port.

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct LinkCounters {
    /// Successfully decoded frames (main board) or CSV lines (valve board).
    pub good_count: u64,
    pub crc_errors: u64,
    pub length_errors: u64,
    pub unknown_tag: u64,
    pub bad_lines: u64,
    /// Milliseconds since Unix epoch of the last successful decode.
    pub last_good_unix_ms: Option<f64>,
    pub disconnects: u64,
}

impl LinkCounters {
    pub fn record_good(&mut self, now_unix_ms: f64) {
        self.good_count += 1;
        self.last_good_unix_ms = Some(now_unix_ms);
    }
}
