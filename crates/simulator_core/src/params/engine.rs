use serde::{Deserialize, Serialize};

/// Propellant tank (oxidiser / pressurant).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TankParams {
    /// Centre position from nose, body-axis [x, y, z] (m).
    pub position: [f64; 3],
    /// Drain (bottom) position [x, y, z] (m).
    ///
    /// If omitted, `position` is used as the fallback in XML generation.
    pub drain_position: Option<[f64; 3]>,
    /// Initial fill (kg).
    pub contents: f64,
}

/// Solid/hybrid fuel grain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelParams {
    /// Grain centre position [x, y, z] (m).
    pub position: [f64; 3],
    /// Initial grain mass (kg).
    pub contents: f64,
    /// Mass remaining after burnout (kg).
    pub after_burn: f64,
}

/// Propulsion system parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineParams {
    /// Thrust vs time table: `[[t_sec, thrust_N], …]`.
    /// Template converts N → lbf internally.
    pub thrust_table: Vec<[f64; 2]>,

    /// Thruster exit position from nose, body-axis [x, y, z] (m).
    pub thruster_pos: [f64; 3],

    pub tank: TankParams,
    pub fuel: FuelParams,
}
