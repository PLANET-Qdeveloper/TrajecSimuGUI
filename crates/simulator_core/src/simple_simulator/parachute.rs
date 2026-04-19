use crate::output::{
	Acceleration, AeroState, AngularRates, Attitude, Position, SimulationState, Velocity,
};
use crate::simple_simulator::{StageRunner, StageStepInput, StageStepOutput};
use crate::{Result, RocketParams};

/// Parachute phase skeleton.
#[derive(Debug, Clone)]
pub struct ParachuteStage {
	altitude_m: f64,
	descent_rate_mps: f64,
	sim_time_sec: f64,
}

impl ParachuteStage {
	pub fn new() -> Self {
		Self {
			altitude_m: 0.0,
			descent_rate_mps: 5.0,
			sim_time_sec: 0.0,
		}
	}

	fn choose_dt_sec(&self, params: &RocketParams) -> f64 {
		// Placeholder policy:
		// Can be replaced with table-driven/adaptive dt later.
		params.sim.time_step
	}
}

impl Default for ParachuteStage {
	fn default() -> Self {
		Self::new()
	}
}

impl StageRunner for ParachuteStage {
	fn initialize(&mut self, _params: &RocketParams) -> Result<()> {
		self.altitude_m = 0.0;
		self.sim_time_sec = 0.0;
		Ok(())
	}

	fn step(&mut self, params: &RocketParams, _input: StageStepInput) -> Result<StageStepOutput> {
		let dt_sec = self.choose_dt_sec(params);
		self.altitude_m = (self.altitude_m - self.descent_rate_mps * dt_sec).max(0.0);
		self.sim_time_sec += dt_sec;

		Ok(StageStepOutput {
			state: SimulationState {
				time_sec: self.sim_time_sec,
				position: Position {
					lat_deg: params.launch_env.latitude,
					lon_deg: params.launch_env.longitude,
					alt_agl_m: self.altitude_m,
				},
				velocity: Velocity {
					true_airspeed_mps: self.descent_rate_mps,
					ground_speed_mps: 0.0,
				},
				attitude: Attitude::default(),
				angular_rates: AngularRates::default(),
				acceleration: Acceleration::default(),
				aero: AeroState::default(),
				thrust_n: 0.0,
				mach: 0.0,
			},
			events: Vec::new(),
			transition_to: None,
			terminate_requested: false,
		})
	}
}

