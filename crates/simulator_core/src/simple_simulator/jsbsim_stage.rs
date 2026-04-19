use crate::jsbsim::JsbSimSimulator;
use crate::orchestrator::Phase;
use crate::progress::EventKind;
use crate::simple_simulator::{StageRunner, StageStepInput, StageStepOutput};
use crate::{Result, RocketParams, Simulator};

/// JSBSim stage adapter.
///
/// Owns JSBSim backend and converts it to unified `StageRunner` contract.
/// Terrain-aware termination is applied internally based on
/// `params.launch_env.terrain`.
pub struct JsbSimStage {
    sim: JsbSimSimulator,
    terrain_terminated: bool,
}

impl JsbSimStage {
    pub fn new() -> Self {
        Self {
            sim: JsbSimSimulator::new(),
            terrain_terminated: false,
        }
    }

    fn hit_terrain(&self, params: &RocketParams, lat_deg: f64, lon_deg: f64, alt_agl_m: f64) -> bool {
        let Some(terrain) = params.launch_env.terrain.as_ref() else {
            return false;
        };
        let terrain_h_m = terrain.altitude_m(lat_deg, lon_deg);
        alt_agl_m <= terrain_h_m
    }
}

impl Default for JsbSimStage {
    fn default() -> Self {
        Self::new()
    }
}

impl StageRunner for JsbSimStage {
    fn initialize(&mut self, params: &RocketParams) -> Result<()> {
        self.terrain_terminated = false;
        self.sim.initialize(params)
    }

    fn step(&mut self, params: &RocketParams, _input: StageStepInput) -> Result<StageStepOutput> {
        let running = self.sim.step()?;
        let state = self.sim.get_state()?;

        let mut events = Vec::new();
        let mut terminate_requested = !running;

        if !self.terrain_terminated
            && self.hit_terrain(
                params,
                state.position.lat_deg,
                state.position.lon_deg,
                state.position.alt_agl_m,
            )
        {
            // JSBSim termination via property injection.
            self.sim.set_property("simulation/terminate", 1.0)?;
            self.terrain_terminated = true;
            terminate_requested = true;
            events.push(EventKind::Landed);
        }

        Ok(StageStepOutput {
            state,
            events,
            transition_to: if terminate_requested { Some(Phase::Completed) } else { None },
            terminate_requested,
        })
    }
}
