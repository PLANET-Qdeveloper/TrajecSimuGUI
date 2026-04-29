//! Composite simulation orchestration skeleton.
//!
//! Orchestrates phase flow across:
//! 1) launch rail (internal 1D)
//! 2) ballistic flight (JSBSim)
//! 3) parachute branch

use crate::analysis::AnalysisOutput;
use crate::output::{SimulationOutput, SimulationState};
use crate::params::InitialPosition;
use crate::progress::{EventKind, EventSource, EventStamp};
use crate::simple_simulator::{
    JsbSimStage, LaunchRailStage, ParachuteStage, StageRunner, StageStepInput,
};
use crate::{Result, RocketParams};

/// Rail-exit state captured for handoff to JSBSim.
struct RailHandoff {
    time_sec: f64,
    body_velocity_mps: [f64; 3],
    lat_deg: f64,
    lon_deg: f64,
    alt_agl_m: f64,
}

/// Ballistic → parachute handoff state captured at deployment.
struct ParachuteHandoff {
    deploy_sim_time_sec: f64,
    position: crate::output::Position,
    vel_enu_down_mps: [f64; 3],
}

/// High-level phase of composite simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Start,
    OnRail,
    Ballistic,
    Parachute,
    Completed,
}

/// Unified output including state trajectory and event timeline.
#[derive(Debug, Clone, Default)]
pub struct UnifiedSimulationOutput {
    pub mainline: SimulationOutput,
    pub parachute_branch: SimulationOutput,
    pub events: Vec<EventStamp>,
    /// Diagnostics + derived events produced by `analysis::analyze`.
    /// Empty unless the runner explicitly invokes the analysis pass.
    pub analysis: AnalysisOutput,
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
    parachute: ParachuteStage,

    /// Sim-time at which the parachute `deploy_trigger.origin` event was
    /// first observed. `None` until the origin event fires.
    deploy_origin_time_sec: Option<f64>,
    parachute_deployed: bool,
}

impl SimulationOrchestrator {
    pub fn new() -> Self {
        Self {
            phase: Phase::Start,
            output: UnifiedSimulationOutput::default(),
            params: None,
            launch_rail: LaunchRailStage::new(),
            jsbsim: JsbSimStage::new(),
            parachute: ParachuteStage::new(),
            deploy_origin_time_sec: None,
            parachute_deployed: false,
        }
    }

    pub fn initialize(&mut self, params: &RocketParams) -> Result<()> {
        self.params = Some(params.clone());

        self.launch_rail.initialize(params)?;
        // JSBSim is initialized lazily at the `OnRail → Ballistic` handoff
        // so the rail-exit body-axis velocity can be written into its IC.
        // ParachuteStage is also initialized lazily at deployment.
        self.deploy_origin_time_sec = None;
        self.parachute_deployed = false;

        self.phase = Phase::OnRail;
        self.output.push_event(EventStamp {
            kind: EventKind::Start,
            sim_time_sec: 0.0,
            source: EventSource::Orchestrator,
            state: None,
        });
        Ok(())
    }

    /// Step once. Returns false when completed.
    pub fn step(&mut self) -> Result<bool> {
        if matches!(self.phase, Phase::Completed) {
            return Ok(false);
        }
        if self.params.is_none() {
            return Ok(false);
        }

        // Staged outputs from the match arm, applied after the borrow
        // of `self.params` is released so we can mutate it for handoff.
        let mut next_phase: Option<Phase> = None;
        let mut rail_handoff: Option<RailHandoff> = None;
        let mut parachute_handoff: Option<ParachuteHandoff> = None;

        match self.phase {
            Phase::OnRail => {
                let params = self.params.as_ref().expect("params present");
                let out = self.launch_rail.step(params, StageStepInput::default())?;
                let out_time_sec = out.state.time_sec;
                let exit_handoff = RailHandoff {
                    time_sec: out.state.time_sec,
                    body_velocity_mps: [
                        out.state.velocity.u_mps,
                        out.state.velocity.v_mps,
                        out.state.velocity.w_mps,
                    ],
                    lat_deg: out.state.position.lat_deg,
                    lon_deg: out.state.position.lon_deg,
                    alt_agl_m: out.state.position.alt_agl_m,
                };
                let snapshot = if out.events.is_empty() {
                    None
                } else {
                    Some(out.state.clone())
                };
                self.output.push_mainline(out.state);

                for kind in out.events {
                    self.output.push_event(EventStamp {
                        kind,
                        sim_time_sec: out_time_sec,
                        source: EventSource::LaunchRail,
                        state: snapshot.clone(),
                    });
                }

                if out.completed {
                    next_phase = Some(Phase::Ballistic);
                    rail_handoff = Some(exit_handoff);
                }
            }
            Phase::Ballistic => {
                let params = self.params.as_ref().expect("params present");
                let out = self.jsbsim.step(params, StageStepInput::default())?;
                let out_time_sec = out.state.time_sec;

                // Latch the deployment origin event the first time it fires.
                if let Some(trig) = params.parachute.deploy_trigger.as_ref() {
                    if self.deploy_origin_time_sec.is_none()
                        && out.events.contains(&trig.origin)
                    {
                        self.deploy_origin_time_sec = Some(out_time_sec);
                    }
                }

                // Check whether the parachute should deploy this step. All
                // conditions must hold: not already deployed, v_term table
                // non-empty, trigger configured, origin seen, delay elapsed.
                let should_deploy = !self.parachute_deployed
                    && !params.parachute.terminal_velocity_table.is_empty()
                    && params
                        .parachute
                        .deploy_trigger
                        .as_ref()
                        .zip(self.deploy_origin_time_sec)
                        .is_some_and(|(trig, t0)| {
                            out_time_sec - t0 >= trig.delay_sec
                        });

                if should_deploy {
                    parachute_handoff = Some(ParachuteHandoff {
                        deploy_sim_time_sec: out_time_sec,
                        position: out.state.position.clone(),
                        vel_enu_down_mps: body_velocity_to_enu_down(
                            [
                                out.state.velocity.u_mps,
                                out.state.velocity.v_mps,
                                out.state.velocity.w_mps,
                            ],
                            out.state.attitude.roll_deg,
                            out.state.attitude.pitch_deg,
                            out.state.attitude.yaw_deg,
                        ),
                    });
                }

                let snapshot = if out.events.is_empty() {
                    None
                } else {
                    Some(out.state.clone())
                };
                self.output.push_mainline(out.state);

                for kind in out.events {
                    self.output.push_event(EventStamp {
                        kind,
                        sim_time_sec: out_time_sec,
                        source: EventSource::JsbSim,
                        state: snapshot.clone(),
                    });
                }

                // Prefer parachute transition over any transition JSBSim
                // itself requested (e.g. Landed from simulation/terminate).
                if next_phase.is_none() {
                    if out.completed {
                        next_phase = Some(Phase::Parachute);
                    }
                }
            }
            Phase::Parachute => {
                let params = self.params.as_ref().expect("params present");
                let out = self.parachute.step(params, StageStepInput::default())?;
                let out_time_sec = out.state.time_sec;
                let snapshot = if out.events.is_empty() {
                    None
                } else {
                    Some(out.state.clone())
                };
                self.output.push_parachute(out.state);

                for kind in out.events {
                    self.output.push_event(EventStamp {
                        kind,
                        sim_time_sec: out_time_sec,
                        source: EventSource::Parachute,
                        state: snapshot.clone(),
                    });
                }

                if out.completed {
                    next_phase = Some(Phase::Completed);
                }
            }
            Phase::Start => {
                next_phase = Some(Phase::OnRail);
            }
            Phase::Completed => {}
        }

        // Apply parachute deployment handoff. Must come before the rail
        // handoff block since the borrow checker requires `self.params`
        // unmutated for the ballistic-arm read above; both handoffs only
        // touch `self`, so order here is independent.
        if let Some(h) = parachute_handoff {
            self.parachute.initialize(
                self.params.as_ref().expect("params present"),
            )?;
            self.parachute
                .seed_from_ballistic_handoff(h.deploy_sim_time_sec, &h.position, h.vel_enu_down_mps);
            self.parachute_deployed = true;
            // Snapshot the last ballistic state we just pushed — that's the
            // vehicle state at the instant the chute deployed.
            let deploy_state = self.output.mainline.trajectory.last().cloned();
            self.output.push_event(EventStamp {
                kind: EventKind::ParachuteOpen,
                sim_time_sec: h.deploy_sim_time_sec,
                source: EventSource::Orchestrator,
                state: deploy_state,
            });
        }

        // Apply handoff IC after the match so we can mutate `self.params`.
        if let Some(h) = rail_handoff {
            let mut handoff = self
                .params
                .as_ref()
                .expect("params present")
                .clone();
            handoff.launch_env.initial_body_velocity_mps = h.body_velocity_mps;
            handoff.launch_env.initial_position_override = Some(InitialPosition {
                latitude_deg: h.lat_deg,
                longitude_deg: h.lon_deg,
                altitude_agl_m: h.alt_agl_m,
            });
            // Seed JSBSim's internal clock with the rail-exit time so
            // both its sim-time-sec-indexed tables (thrust, fuel) and
            // published timestamps pick up from the handoff instant.
            handoff.sim.start_sim_time_sec = h.time_sec;
            self.jsbsim.initialize(&handoff)?;
            self.params = Some(handoff);
        }

        if let Some(p) = next_phase {
            self.phase = p;
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

/// Rotate a body-frame velocity `[u, v, w]` (aerospace: x-forward, y-right,
/// z-down) into an ENU-with-down-positive frame `[east, north, down]`, using
/// the Euler angles `(roll φ, pitch θ, yaw ψ)`.
///
/// The aerospace 3-2-1 (Z-Y-X) rotation takes body → NED; we then swap
/// NED → (east, north, down) which is a trivial component reorder.
pub(crate) fn body_velocity_to_enu_down(
    body_uvw_mps: [f64; 3],
    roll_deg: f64,
    pitch_deg: f64,
    yaw_deg: f64,
) -> [f64; 3] {
    let (u, v, w) = (body_uvw_mps[0], body_uvw_mps[1], body_uvw_mps[2]);
    let (cphi, sphi) = (roll_deg.to_radians().cos(), roll_deg.to_radians().sin());
    let (cth, sth) = (pitch_deg.to_radians().cos(), pitch_deg.to_radians().sin());
    let (cpsi, spsi) = (yaw_deg.to_radians().cos(), yaw_deg.to_radians().sin());

    // R_NED_body · [u, v, w]
    let v_n = u * (cth * cpsi)
        + v * (-cphi * spsi + sphi * sth * cpsi)
        + w * (sphi * spsi + cphi * sth * cpsi);
    let v_e = u * (cth * spsi)
        + v * (cphi * cpsi + sphi * sth * spsi)
        + w * (-sphi * cpsi + cphi * sth * spsi);
    let v_d = u * (-sth) + v * (sphi * cth) + w * (cphi * cth);

    [v_e, v_n, v_d]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_to_enu_identity_at_zero_attitude() {
        // Zero pitch/roll/yaw: body x = north, body y = east, body z = down.
        // Pure-forward (body x) → north.
        let enu = body_velocity_to_enu_down([10.0, 0.0, 0.0], 0.0, 0.0, 0.0);
        assert!(enu[0].abs() < 1e-9, "east should be 0, got {}", enu[0]);
        assert!((enu[1] - 10.0).abs() < 1e-9, "north should be 10, got {}", enu[1]);
        assert!(enu[2].abs() < 1e-9, "down should be 0, got {}", enu[2]);
    }

    #[test]
    fn body_to_enu_yaw_east_turns_forward_into_east() {
        // Yaw 90° east: body forward now points east.
        let enu = body_velocity_to_enu_down([10.0, 0.0, 0.0], 0.0, 0.0, 90.0);
        assert!((enu[0] - 10.0).abs() < 1e-9);
        assert!(enu[1].abs() < 1e-9);
        assert!(enu[2].abs() < 1e-9);
    }

    #[test]
    fn body_to_enu_pitch_up_lifts_forward() {
        // 90° nose-up (pitch +90°): body forward is straight up → down = -u.
        let enu = body_velocity_to_enu_down([10.0, 0.0, 0.0], 0.0, 90.0, 0.0);
        assert!(enu[0].abs() < 1e-9);
        assert!(enu[1].abs() < 1e-9);
        assert!((enu[2] - (-10.0)).abs() < 1e-9, "down should be -10, got {}", enu[2]);
    }
}
