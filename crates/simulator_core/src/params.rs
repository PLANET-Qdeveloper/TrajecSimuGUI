pub mod aero;
pub mod body_mass;
pub mod engine;
pub mod launch_env;
pub mod parachute;
pub mod sim_control;

pub use aero::{AeroParams, Cd0AlphaMachTable};
pub use body_mass::BodyMassParams;
pub use engine::{EngineParams, FuelParams, TankParams};
pub use launch_env::{InitialPosition, LaunchEnvParams};
pub use parachute::ParachuteParams;
pub use sim_control::SimControl;

use crate::error::{Result, SimulatorError};
use serde::{Deserialize, Serialize};

/// Complete rocket simulation parameters.
///
/// All values are in SI units. Unit conversions to JSBSim's internal
/// fps/lbs system happen inside `XmlContext::from` or the C++ bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RocketParams {
    pub body_mass: BodyMassParams,
    pub engine: EngineParams,
    pub aero: AeroParams,
    pub launch_env: LaunchEnvParams,
    pub sim: SimControl,
    #[serde(default)]
    pub parachute: ParachuteParams,
}

impl RocketParams {
    pub fn validate(&self) -> Result<()> {
        if self.body_mass.diameter <= 0.0 {
            return Err(SimulatorError::InvalidParameters(
                "body.diameter must be positive".into(),
            ));
        }
        if self.body_mass.total_mass <= 0.0 {
            return Err(SimulatorError::InvalidParameters(
                "mass.total_mass must be positive".into(),
            ));
        }
        if self.launch_env.rail_length_m <= 0.0 {
            return Err(SimulatorError::InvalidParameters(
                "launch.rail_length_m must be positive".into(),
            ));
        }
        if self.sim.flight_duration <= 0.0 || self.sim.time_step <= 0.0 {
            return Err(SimulatorError::InvalidParameters(
                "sim.flight_duration and sim.time_step must be positive".into(),
            ));
        }
        if self.sim.time_step >= self.sim.flight_duration {
            return Err(SimulatorError::InvalidParameters(
                "sim.time_step must be less than flight_duration".into(),
            ));
        }
        if !((-90.0..=90.0).contains(&self.launch_env.latitude)) {
            return Err(SimulatorError::InvalidParameters(
                "launch.latitude must be in [-90, 90]".into(),
            ));
        }
        if !((-180.0..=180.0).contains(&self.launch_env.longitude)) {
            return Err(SimulatorError::InvalidParameters(
                "launch.longitude must be in [-180, 180]".into(),
            ));
        }
        if self.engine.thrust_table.is_empty() {
            return Err(SimulatorError::InvalidParameters(
                "engine.thrust_table must not be empty".into(),
            ));
        }
        Ok(())
    }
}
