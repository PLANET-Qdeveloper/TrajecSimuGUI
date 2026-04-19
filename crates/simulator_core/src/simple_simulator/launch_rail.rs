use crate::orchestrator::Phase;
use crate::output::{
	Acceleration, AeroState, AngularRates, Attitude, Position, SimulationState, Velocity,
};
use crate::progress::EventKind;
use crate::simple_simulator::{StageRunner, StageStepInput, StageStepOutput};
use crate::{Result, RocketParams};

const EARTH_RADIUS_M: f64 = 6_378_137.0;

/// Launch rail phase (internal 1D) skeleton.
#[derive(Debug, Clone)]
pub struct LaunchRailStage {
	distance_m: f64,
	velocity_mps: f64,
	sim_time_sec: f64,
	launch_clear_emitted: bool,
}

impl LaunchRailStage {
	pub fn new() -> Self {
		Self {
			distance_m: 0.0,
			velocity_mps: 0.0,
			sim_time_sec: 0.0,
			launch_clear_emitted: false,
		}
	}

	fn choose_dt_sec(&self, params: &RocketParams) -> f64 {
		// Placeholder policy:
		// For now use configured base dt.
		// Later this can be switched to table-driven/adaptive dt.
		params.sim.time_step
	}

	fn to_public_state(&self, params: &RocketParams, sim_time_sec: f64) -> SimulationState {
		let launch = &params.launch_env;

		let pitch = launch.pitch.to_radians();
		let yaw = launch.yaw.to_radians();

		let horizontal_m = self.distance_m * pitch.cos();
		let up_m = self.distance_m * pitch.sin();

		let north_m = horizontal_m * yaw.cos();
		let east_m = horizontal_m * yaw.sin();

		let dlat_deg = (north_m / EARTH_RADIUS_M).to_degrees();
		let lon_scale = launch.latitude.to_radians().cos().abs().max(1e-6);
		let dlon_deg = (east_m / (EARTH_RADIUS_M * lon_scale)).to_degrees();

		let speed = self.velocity_mps.abs();

		SimulationState {
			time_sec: sim_time_sec,
			position: Position {
				lat_deg: launch.latitude + dlat_deg,
				lon_deg: launch.longitude + dlon_deg,
				alt_agl_m: up_m.max(0.0),
			},
			velocity: Velocity {
				true_airspeed_mps: speed,
				ground_speed_mps: speed,
			},
			attitude: Attitude {
				pitch_deg: launch.pitch,
				roll_deg: launch.roll,
				yaw_deg: launch.yaw,
			},
			angular_rates: AngularRates::default(),
			acceleration: Acceleration::default(),
			aero: AeroState::default(),
			thrust_n: 0.0,
			mach: 0.0,
		}
	}
}

impl Default for LaunchRailStage {
	fn default() -> Self {
		Self::new()
	}
}

impl StageRunner for LaunchRailStage {
	fn initialize(&mut self, _params: &RocketParams) -> Result<()> {
		self.distance_m = 0.0;
		self.velocity_mps = 0.0;
		self.sim_time_sec = 0.0;
		self.launch_clear_emitted = false;
		Ok(())
	}

	fn step(&mut self, params: &RocketParams, _input: StageStepInput) -> Result<StageStepOutput> {
		let dt_sec = self.choose_dt_sec(params);

		// Skeleton-only dynamics placeholder.
		self.velocity_mps += 1.0 * dt_sec;
		self.distance_m += self.velocity_mps * dt_sec;
		self.sim_time_sec += dt_sec;

		let mut events = Vec::new();
		let mut transition_to = None;

		if !self.launch_clear_emitted && self.distance_m >= params.launch_env.rail_length_m {
			self.launch_clear_emitted = true;
			events.push(EventKind::LaunchClear);
			transition_to = Some(Phase::Ballistic);
		}

		Ok(StageStepOutput {
			state: self.to_public_state(params, self.sim_time_sec),
			events,
			transition_to,
			terminate_requested: false,
		})
	}
}

