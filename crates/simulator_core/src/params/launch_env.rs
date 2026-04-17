use serde::{Deserialize, Serialize};

/// Integrated launch-site + environment parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEnvParams {
    /// Geodetic latitude (degrees north).
    pub latitude: f64,
    /// Longitude (degrees east).
    pub longitude: f64,
    /// Launch pad elevation above sea level (m).
    pub elevation: f64,
    /// Launcher height above ground (m). Wind model activates above this.
    pub launcher_height: f64,
    /// Initial pitch angle (degrees). 90 = vertical.
    pub pitch: f64,
    /// Initial roll angle (degrees).
    pub roll: f64,
    /// Initial yaw / heading (degrees).
    pub yaw: f64,

    /// Winds-aloft table: `[[altitude_m, speed_mps, direction_deg], …]`.
    ///
    /// `altitude_m` — height above sea level (m).
    /// `speed_mps`  — wind speed (m/s).
    /// `direction_deg` — meteorological wind direction (degrees, 0 = north).
    pub winds_table: Vec<[f64; 3]>,
}
