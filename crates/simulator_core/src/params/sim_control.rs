use serde::{Deserialize, Serialize};

fn default_csv_sample_interval() -> u32 {
    1
}

fn default_kml_sample_interval() -> u32 {
    10
}

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
    /// CSV writer decimation: emit one row every N trajectory steps.
    /// `1` keeps every step.
    #[serde(default = "default_csv_sample_interval")]
    pub csv_sample_interval: u32,
    /// KML writer decimation: emit one point every N trajectory steps.
    /// Defaults to `10` so KML stays readable for typical flight lengths.
    #[serde(default = "default_kml_sample_interval")]
    pub kml_sample_interval: u32,
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
            apogee_mode: 0,
            csv_sample_interval: default_csv_sample_interval(),
            kml_sample_interval: default_kml_sample_interval(),
            start_sim_time_sec: 0.0,
        }
    }
}
