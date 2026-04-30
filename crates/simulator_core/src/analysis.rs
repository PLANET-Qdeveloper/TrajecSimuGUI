//! Post-simulation analysis: derived events detected from the trajectory.
//!
//! Run as a single pass over the assembled `UnifiedSimulationOutput` after
//! the orchestrator finishes. Appends derived events to
//! `UnifiedSimulationOutput.events`: `MaxQ`, `MaxAxialAcceleration`,
//! `MaxLateralAcceleration`, `MaxAngularRate`. Each carries the full
//! `SimulationState` snapshot on its `EventStamp.state` field so downstream
//! consumers can read the vehicle state at the peak.

use serde::{Deserialize, Serialize};

use crate::orchestrator::UnifiedSimulationOutput;
use crate::output::SimulationState;
use crate::params::RocketParams;
use crate::progress::{EventKind, EventSource, EventStamp};

/// Output of [`analyze`]. Currently empty — derived events are appended
/// directly to `UnifiedSimulationOutput.events`. Reserved as the home for
/// future analysis artefacts (frequency-domain summaries, statistics, …).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalysisOutput {}

/// Run all post-simulation analyses, mutating `output` in place.
pub fn analyze(output: &mut UnifiedSimulationOutput, _params: &RocketParams) {
    let derived = detect_derived_events(&output.mainline.trajectory);
    output.events.extend(derived);
    output
        .events
        .sort_by(|a, b| a.sim_time_sec.total_cmp(&b.sim_time_sec));
}

fn detect_derived_events(traj: &[SimulationState]) -> Vec<EventStamp> {
    if traj.is_empty() {
        return Vec::new();
    }

    // (index of peak, peak value).
    let mut max_q = (0usize, f64::NEG_INFINITY);
    let mut max_axial = (0usize, f64::NEG_INFINITY);
    let mut max_thrust = (0usize, f64::NEG_INFINITY);
    let mut max_airspeed = (0usize, f64::NEG_INFINITY);
    let mut max_dynamic_pressure_alpha = (0usize, f64::NEG_INFINITY);
    let mut max_lateral = (0usize, f64::NEG_INFINITY);
    let mut max_rate = (0usize, f64::NEG_INFINITY);

    for (i, s) in traj.iter().enumerate() {
        if s.aero.qbar_pa > max_q.1 {
            max_q = (i, s.aero.qbar_pa);
        }
        if s.acceleration.ax_mps2 > max_axial.1 {
            max_axial = (i, s.acceleration.ax_mps2);
        }
        if s.thrust_n > max_thrust.1 {
            max_thrust = (i, s.thrust_n);
        }
        if s.velocity.true_airspeed_mps > max_airspeed.1 {
            max_airspeed = (i, s.velocity.true_airspeed_mps);
        }
        if s.aero.qbar_pa * s.aero.alpha_deg > max_dynamic_pressure_alpha.1 {
            max_dynamic_pressure_alpha = (i, s.aero.qbar_pa * s.aero.alpha_deg);
        }
        let lat = (s.acceleration.ay_mps2.powi(2) + s.acceleration.az_mps2.powi(2)).sqrt();
        if lat > max_lateral.1 {
            max_lateral = (i, lat);
        }
        let rate = (s.angular_rates.p_rad_sec.powi(2)
            + s.angular_rates.q_rad_sec.powi(2)
            + s.angular_rates.r_rad_sec.powi(2))
        .sqrt();
        if rate > max_rate.1 {
            max_rate = (i, rate);
        }
    }

    let mk = |kind, idx: usize| EventStamp {
        kind,
        sim_time_sec: traj[idx].time_sec,
        source: EventSource::Analysis,
        state: Some(traj[idx].clone()),
    };
    vec![
        mk(EventKind::MaxQ, max_q.0),
        mk(EventKind::MaxAxialAcceleration, max_axial.0),
        mk(EventKind::MaxThrust, max_thrust.0),
        mk(EventKind::MaxAirspeed, max_airspeed.0),
        mk(EventKind::MaxDynamicPressureAlpha, max_dynamic_pressure_alpha.0),
        mk(EventKind::MaxLateralAcceleration, max_lateral.0),
        mk(EventKind::MaxAngularRate, max_rate.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::SimulationOutput;
    use crate::params::{
        AeroParams, BodyMassParams, Cd0AlphaMachTable, EngineParams, FuelParams,
        LaunchEnvParams, ParachuteParams, RocketParams, SimControl, TankParams,
    };

    fn make_state(t: f64, qbar: f64, ax: f64, ay: f64, az: f64) -> SimulationState {
        SimulationState {
            time_sec: t,
            aero: crate::output::AeroState {
                alpha_deg: 0.0,
                beta_deg: 0.0,
                qbar_pa: qbar,
            },
            acceleration: crate::output::Acceleration {
                ax_mps2: ax,
                ay_mps2: ay,
                az_mps2: az,
            },
            ..Default::default()
        }
    }

    #[test]
    fn detect_max_q_finds_peak() {
        let traj = vec![
            make_state(0.0, 100.0, 0.0, 0.0, 0.0),
            make_state(1.0, 500.0, 0.0, 0.0, 0.0), // peak
            make_state(2.0, 200.0, 0.0, 0.0, 0.0),
        ];
        let events = detect_derived_events(&traj);
        let max_q = events
            .iter()
            .find(|e| e.kind == EventKind::MaxQ)
            .expect("MaxQ");
        assert!((max_q.sim_time_sec - 1.0).abs() < 1e-9);
        assert!(
            (max_q.state.as_ref().unwrap().aero.qbar_pa - 500.0).abs() < 1e-9
        );
        assert_eq!(max_q.source, EventSource::Analysis);
    }

    #[test]
    fn detect_max_lateral_uses_yz_norm() {
        let traj = vec![
            // Lateral magnitude = sqrt(3² + 4²) = 5 at t=1
            make_state(0.0, 0.0, 0.0, 1.0, 1.0),
            make_state(1.0, 0.0, 0.0, 3.0, 4.0),
            make_state(2.0, 0.0, 0.0, 2.0, 2.0),
        ];
        let events = detect_derived_events(&traj);
        let lat = events
            .iter()
            .find(|e| e.kind == EventKind::MaxLateralAcceleration)
            .unwrap();
        assert!((lat.sim_time_sec - 1.0).abs() < 1e-9);
    }

    #[test]
    fn detect_max_angular_rate_uses_pqr_norm() {
        let s0 = SimulationState::default();
        let s1 = SimulationState {
            time_sec: 1.0,
            angular_rates: crate::output::AngularRates {
                p_rad_sec: 1.0,
                q_rad_sec: 2.0,
                r_rad_sec: 2.0,
            },
            ..Default::default()
        };
        let s2 = SimulationState {
            time_sec: 2.0,
            angular_rates: crate::output::AngularRates {
                p_rad_sec: 1.0,
                q_rad_sec: 0.0,
                r_rad_sec: 0.0,
            },
            ..Default::default()
        };
        let traj = vec![s0, s1, s2];

        let events = detect_derived_events(&traj);
        let rate = events
            .iter()
            .find(|e| e.kind == EventKind::MaxAngularRate)
            .unwrap();
        assert!((rate.sim_time_sec - 1.0).abs() < 1e-9);
    }

    #[test]
    fn analyze_appends_derived_events_in_time_order() {
        let mut output = UnifiedSimulationOutput::default();
        output.mainline = SimulationOutput::new();
        output.mainline.trajectory = vec![
            make_state(0.0, 50.0, 0.0, 0.0, 0.0),
            make_state(1.0, 500.0, 10.0, 1.0, 1.0),
            make_state(2.0, 300.0, 5.0, 0.0, 0.0),
        ];
        output.events.push(EventStamp {
            kind: EventKind::Apogee,
            sim_time_sec: 2.0,
            source: EventSource::JsbSim,
            state: None,
        });
        let p = sample_params();
        analyze(&mut output, &p);

        // Derived events should land at t=1.0 (peaks) and Apogee at t=2.0
        let times: Vec<f64> = output.events.iter().map(|e| e.sim_time_sec).collect();
        for w in times.windows(2) {
            assert!(w[0] <= w[1], "events not time-sorted: {times:?}");
        }
        assert!(output.events.iter().any(|e| e.kind == EventKind::MaxQ));
    }

    fn sample_params() -> RocketParams {
        RocketParams {
            body_mass: BodyMassParams {
                diameter: 0.1,
                total_mass: 10.0,
                cg: [0.5, 0.0, 0.0],
                inertia: [1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
            },
            engine: EngineParams {
                thrust_table: vec![[0.0, 100.0], [1.0, 0.0]].into(),
                thruster_pos: [1.0, 0.0, 0.0],
                tank: TankParams {
                    position: [0.5, 0.0, 0.0],
                    drain_position: None,
                    contents: 0.1,
                },
                fuel: FuelParams {
                    position: [0.5, 0.0, 0.0],
                    contents: 0.1,
                    after_burn: 0.0,
                },
            },
            aero: AeroParams {
                cp_at_launch: [0.5, 0.0, 0.0],
                cp_mach_table: vec![[0.0, 0.5]].into(),
                cd0_alpha_mach_table: Cd0AlphaMachTable {
                    mach_keys: vec![0.0].into(),
                    rows: vec![vec![0.0, 0.3]].into(),
                },
                cn_table: vec![[0.0, 2.0], [2.0, 2.0]].into(),
                cs_table: vec![[0.0, 2.0]].into(),
                roll_damping_coefficient: 0.0,
                pitch_damping_coefficient: 0.0,
                yaw_damping_coefficient: 0.0,
            },
            launch_env: LaunchEnvParams {
                latitude: 35.0,
                longitude: 139.0,
                elevation: 0.0,
                launcher_height: 5.0,
                rail_length_m: 5.0,
                pitch: 90.0,
                roll: 0.0,
                yaw: 0.0,
                winds_table: Vec::<[f64; 3]>::new().into(),
                initial_body_velocity_mps: [0.0; 3],
                initial_position_override: None,
            },
            sim: SimControl::default(),
            parachute: ParachuteParams::default(),
        }
    }
}
