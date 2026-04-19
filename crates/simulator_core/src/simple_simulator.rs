
//! Lightweight simulation components used by the composite orchestrator.
//!
//! Current scope:
//! - launch rail internal 1D kinematics
//! - parachute simplified descent model (skeleton)

pub mod launch_rail;
pub mod jsbsim_stage;
pub mod parachute;

use crate::orchestrator::Phase;
use crate::output::SimulationState;
use crate::progress::EventKind;
use crate::{Result, RocketParams};

pub use launch_rail::LaunchRailStage;
pub use jsbsim_stage::JsbSimStage;
pub use parachute::ParachuteStage;

/// Unified per-step input for every phase runner.
#[derive(Debug, Clone, Copy)]
pub struct StageStepInput {
	/// Reserved for external controls/injections.
	///
	/// Time progression (`dt`, `sim_time`) is owned by each stage runner.
	/// This keeps JSBSim and table-driven stages free to choose adaptive
	/// step widths internally.
	pub _reserved: (),
}

impl Default for StageStepInput {
	fn default() -> Self {
		Self { _reserved: () }
	}
}

/// Unified per-step output for every phase runner.
#[derive(Debug, Clone)]
pub struct StageStepOutput {
	/// External state format shared by all phases.
	pub state: SimulationState,
	/// One-shot events fired in this step.
	pub events: Vec<EventKind>,
	/// Optional phase transition request.
	pub transition_to: Option<Phase>,
	/// Optional termination request.
	pub terminate_requested: bool,
}

/// Common contract for phase runners.
///
/// `RocketParams` is reused as the single configuration source across phases.
/// `SimulationState` is reused as the single external state format across phases.
pub trait StageRunner {
	fn initialize(&mut self, params: &RocketParams) -> Result<()>;
	fn step(&mut self, params: &RocketParams, input: StageStepInput) -> Result<StageStepOutput>;
}