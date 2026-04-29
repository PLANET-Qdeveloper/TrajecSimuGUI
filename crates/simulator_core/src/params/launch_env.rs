
use std::sync::Arc;

use serde::{Deserialize, Serialize};

fn default_rail_length_m() -> f64 {
    5.0
}

/// Geodetic position override for JSBSim's initial conditions.
///
/// Written into the `liftoff.xml.j2` `<latitude>`, `<longitude>` and
/// `<altitude>` (AGL) tags when `LaunchEnvParams::initial_position_override`
/// is set. Used by the orchestrator to start JSBSim from the launch-rail
/// exit point rather than the pad origin.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct InitialPosition {
    /// Geodetic latitude (degrees north).
    pub latitude_deg: f64,
    /// Longitude (degrees east).
    pub longitude_deg: f64,
    /// Altitude above ground level (m).
    pub altitude_agl_m: f64,
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
    ///
    /// Held behind an `Arc` so multiple parallel simulations can share a
    /// single terrain dataset without cloning it. Skipped by serde because
    /// trait objects cannot round-trip through YAML/JSON; user-facing
    /// configs describe terrain symbolically and the loader injects the
    /// concrete trait object at assemble time.
    #[serde(skip)]
    pub terrain: Option<Arc<dyn crate::terrain::Terrain>>,
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
    #[serde(with = "crate::arc_serde::slice")]
    pub winds_table: Arc<[[f64; 3]]>,

    /// Initial body-axis velocity written into JSBSim's initial
    /// conditions as `[u, v, w]` (m/s).
    ///
    /// Default `[0, 0, 0]` is the correct value for a cold launch from
    /// the pad. The orchestrator overwrites this with the launch-rail
    /// exit velocity at the `OnRail → Ballistic` handoff so that JSBSim
    /// starts its integration from the rail-clear state rather than rest.
    #[serde(default)]
    pub initial_body_velocity_mps: [f64; 3],

    /// Optional override of the JSBSim initial-condition position.
    ///
    /// `None` → use the pad coordinates (`latitude`, `longitude`,
    /// `elevation`). The orchestrator sets this to the launch-rail exit
    /// point at the `OnRail → Ballistic` handoff.
    #[serde(default)]
    pub initial_position_override: Option<InitialPosition>,
}
