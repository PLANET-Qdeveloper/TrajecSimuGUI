use serde::{Deserialize, Serialize};

fn default_rail_length_m() -> f64 {
    5.0
}

/// Terrain model placeholder.
///
/// Accepts latitude/longitude and returns terrain altitude [m].
/// Interpolation and storage backend will be added later.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TerrainModel {
    /// Reserved for future terrain source/config fields.
    #[serde(default)]
    pub _reserved: Option<String>,
}

impl TerrainModel {
    /// Terrain altitude at geodetic location.
    ///
    /// Current placeholder always returns `0.0`.
    pub fn altitude_m(&self, _lat_deg: f64, _lon_deg: f64) -> f64 {
        0.0
    }
}

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
    /// Launcher rail length (m). Used by rail-length based `launch_clear`.
    #[serde(default = "default_rail_length_m")]
    pub rail_length_m: f64,
    /// Optional terrain model used for terrain-aware landing termination.
    #[serde(default)]
    pub terrain: Option<TerrainModel>,
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
