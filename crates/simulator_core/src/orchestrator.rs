//! Composite simulation orchestration skeleton.
//!
//! Orchestrates phase flow across:
//! 1) launch rail (internal 1D)
//! 2) ballistic flight (JSBSim)
//! 3) parachute branch

use crate::output::{SimulationOutput, SimulationState};
use crate::progress::{EventKind, EventSource, EventStamp};
use crate::simple_simulator::{JsbSimStage, LaunchRailStage, StageRunner, StageStepInput};
use crate::{Result, RocketParams};

/// High-level phase of composite simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Start,
    OnRail,
    Ballistic,
    Parachute,
    Completed,
}

/// Event-triggered branch definition (one-shot).
#[derive(Debug, Clone)]
pub struct DelayedBranchTrigger {
    /// Origin event for delay timing.
    pub origin: EventKind,
    /// Delay from origin event [s].
    pub delay_sec: f64,
    /// Internal one-shot latch.
    pub fired: bool,
}

/// Unified output including state trajectory and event timeline.
#[derive(Debug, Clone, Default)]
pub struct UnifiedSimulationOutput {
    pub mainline: SimulationOutput,
    pub parachute_branch: SimulationOutput,
    pub events: Vec<EventStamp>,
}

impl UnifiedSimulationOutput {
    pub fn push_mainline(&mut self, state: SimulationState) {
        self.mainline.push(state);
    }

    pub fn push_parachute(&mut self, state: SimulationState) {
        self.parachute_branch.push(state);
    }

    pub fn push_event(&mut self, event: EventStamp) {
        self.events.push(event);
    }
}

/// Skeleton orchestrator. Behavior will be implemented incrementally.
pub struct SimulationOrchestrator {
    phase: Phase,
    output: UnifiedSimulationOutput,
    params: Option<RocketParams>,
    launch_rail: LaunchRailStage,
    jsbsim: JsbSimStage,
}

impl SimulationOrchestrator {
    pub fn new() -> Self {
        Self {
            phase: Phase::Start,
            output: UnifiedSimulationOutput::default(),
            params: None,
            launch_rail: LaunchRailStage::new(),
            jsbsim: JsbSimStage::new(),
        }
    }

    pub fn initialize(&mut self, params: &RocketParams) -> Result<()> {
        self.params = Some(params.clone());

        self.launch_rail.initialize(params)?;
        self.jsbsim.initialize(params)?;

        self.phase = Phase::OnRail;
        self.output.push_event(EventStamp {
            kind: EventKind::Start,
            sim_time_sec: 0.0,
            source: EventSource::Orchestrator,
        });
        Ok(())
    }

    /// Step once. Returns false when completed.
    pub fn step(&mut self) -> Result<bool> {
        if matches!(self.phase, Phase::Completed) {
            return Ok(false);
        }

        let Some(params) = self.params.as_ref() else {
            return Ok(false);
        };

        match self.phase {
            Phase::OnRail => {
                let out = self.launch_rail.step(params, StageStepInput::default())?;
                let out_time_sec = out.state.time_sec;
                self.output.push_mainline(out.state);

                for kind in out.events {
                    self.output.push_event(EventStamp {
                        kind,
                        sim_time_sec: out_time_sec,
                        source: EventSource::LaunchRail,
                    });
                }

                if let Some(next) = out.transition_to {
                    self.phase = next;
                }
                if out.terminate_requested {
                    self.phase = Phase::Completed;
                }
            }
            Phase::Ballistic => {
                let out = self.jsbsim.step(params, StageStepInput::default())?;
                let out_time_sec = out.state.time_sec;
                self.output.push_mainline(out.state);

                for kind in out.events {
                    self.output.push_event(EventStamp {
                        kind,
                        sim_time_sec: out_time_sec,
                        source: EventSource::JsbSim,
                    });
                }

                if let Some(next) = out.transition_to {
                    self.phase = next;
                }
                if out.terminate_requested {
                    self.phase = Phase::Completed;
                }
            }
            Phase::Parachute => {
                // Parachute branch integration will be connected in the next step.
                self.phase = Phase::Completed;
            }
            Phase::Start => {
                self.phase = Phase::OnRail;
            }
            Phase::Completed => {}
        }

        let running = !matches!(self.phase, Phase::Completed);
        Ok(running)
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn output(&self) -> &UnifiedSimulationOutput {
        &self.output
    }

    pub fn into_output(self) -> UnifiedSimulationOutput {
        self.output
    }
}

impl Default for SimulationOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}
