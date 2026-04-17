use serde::{Deserialize, Serialize};

/// Simulation run-control parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimControl {
    /// Maximum simulation duration (s). Maps to `<run end="…">`.
    pub flight_duration: f64,
    /// Simulation time step (s). Maps to `<run dt="…">`.
    pub time_step: f64,
    /// Apogee behaviour.
    /// `0` = continue to landing (full flight).
    /// `1` = terminate at apogee detection.
    pub apogee_mode: u8,
    /// How many JSBSim steps between `get_state()` calls (1 = every step).
    pub state_sample_interval: u32,
}

impl Default for SimControl {
    fn default() -> Self {
        Self {
            flight_duration: 120.0,
            time_step: 0.01,
            apogee_mode: 0,
            state_sample_interval: 1,
        }
    }
}
