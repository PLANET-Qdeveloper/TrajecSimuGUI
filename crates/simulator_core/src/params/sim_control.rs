use serde::{Deserialize, Serialize};

/// Simulation run-control parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimControl {
    /// Maximum simulation duration (s). Maps to `<run end="…">`.
    pub flight_duration: f64,
    /// Simulation time step (s). Maps to `<run dt="…">`.
    pub time_step: f64,
    /// Output decimation rate: record every Nth step to the output trajectory.
    pub output_decimation_rate: usize,
    /// Initial simulation time (s). Applied via `FGFDMExec::Setsim_time`
    /// right after `RunIC`, which itself resets sim-time to 0. Used by
    /// the orchestrator to keep timestamps continuous across the
    /// launch-rail → JSBSim handoff.
    #[serde(default)]
    pub start_sim_time_sec: f64,
}

impl Default for SimControl {
    fn default() -> Self {
        Self {
            flight_duration: 120.0,
            time_step: 0.01,
            output_decimation_rate: 1,
            start_sim_time_sec: 0.0,
        }
    }
}
