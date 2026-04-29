//! Parachute descent stage: 3-D point mass with implicit drag.
//!
//! Physics
//! ───────
//! The parachute is characterised by a (possibly time-varying) terminal
//! velocity `v_term(t)`, where `t` is measured from the deployment instant.
//! At terminal velocity the drag magnitude balances gravity:
//!
//!     m g = ½ ρ Cd A · v_term²  ⇒  (ρ Cd A) / (2 m) = g / v_term²
//!
//! Using this, the drag acceleration on a point mass moving at ground-frame
//! velocity `v` through still-air-equivalent wind `w` is
//!
//!     a_drag = −(g / v_term²) · |v − w| · (v − w)              [ENU]
//!     a_grav = (0, 0, +g)   (ENU; +z is down in the body-down frame,
//!                             here expressed with down-positive)
//!
//! ENU convention
//! ──────────────
//! This stage works in an east/north/down-positive mixed frame:
//! `v_east`, `v_north` are standard ENU horizontals, and `v_down`
//! is positive-downward (altitude decreases when `v_down > 0`).
//!
//! Modes
//! ─────
//! * `Transient`   — semi-implicit Euler on the full drag ODE.
//! * `SteadyState` — analytic: horizontal velocity equals wind, vertical
//!   velocity equals `v_term(t)`. Promotes to this mode once the vehicle
//!   has been within `settle_tol_frac` of its terminal envelope for
//!   `settle_hold_steps` consecutive integrator steps.

use crate::orchestrator::Phase;
use crate::output::{
    Acceleration, AeroState, AngularRates, Attitude, Position, SimulationState, Velocity,
};
use crate::progress::EventKind;
use crate::simple_simulator::env::{self, G0_MPS2};
use crate::simple_simulator::{StageRunner, StageStepInput, StageStepOutput};
use crate::{Result, RocketParams};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Transient,
    SteadyState,
}

#[derive(Debug, Clone)]
pub struct ParachuteStage {
    mode: Mode,

    lat_deg: f64,
    lon_deg: f64,
    alt_agl_m: f64,

    v_east_mps: f64,
    v_north_mps: f64,
    /// Positive downward. Descent → `v_down_mps > 0`.
    v_down_mps: f64,

    sim_time_sec: f64,
    deploy_sim_time_sec: f64,

    settle_run_steps: u32,
    landed: bool,
}

impl ParachuteStage {
    pub fn new() -> Self {
        Self {
            mode: Mode::Transient,
            lat_deg: 0.0,
            lon_deg: 0.0,
            alt_agl_m: 0.0,
            v_east_mps: 0.0,
            v_north_mps: 0.0,
            v_down_mps: 0.0,
            sim_time_sec: 0.0,
            deploy_sim_time_sec: 0.0,
            settle_run_steps: 0,
            landed: false,
        }
    }

    /// Seed the stage with the vehicle's state at the moment of parachute
    /// deployment. Called by the orchestrator right after `initialize`.
    pub fn seed_from_ballistic_handoff(
        &mut self,
        deploy_sim_time_sec: f64,
        pos: &Position,
        vel_enu_down_mps: [f64; 3],
    ) {
        self.mode = Mode::Transient;
        self.lat_deg = pos.lat_deg;
        self.lon_deg = pos.lon_deg;
        self.alt_agl_m = pos.alt_agl_m;
        self.v_east_mps = vel_enu_down_mps[0];
        self.v_north_mps = vel_enu_down_mps[1];
        self.v_down_mps = vel_enu_down_mps[2];
        self.sim_time_sec = deploy_sim_time_sec;
        self.deploy_sim_time_sec = deploy_sim_time_sec;
        self.settle_run_steps = 0;
        self.landed = false;
    }

    fn wind_enu_at_current_alt(&self, params: &RocketParams) -> [f64; 3] {
        let alt_msl_m = params.launch_env.elevation + self.alt_agl_m;
        env::wind_enu_at_alt(&params.launch_env.winds_table, alt_msl_m)
    }

    fn terminal_velocity_mps(&self, params: &RocketParams) -> f64 {
        let t_since = (self.sim_time_sec - self.deploy_sim_time_sec).max(0.0);
        lookup_terminal_mps(&params.parachute.terminal_velocity_table, t_since)
    }

    fn build_state(
        &self,
        wind_enu: [f64; 3],
        accel_enu_down: [f64; 3],
    ) -> SimulationState {
        let v_h = (self.v_east_mps.powi(2) + self.v_north_mps.powi(2)).sqrt();
        let v_rel_e = self.v_east_mps - wind_enu[0];
        let v_rel_n = self.v_north_mps - wind_enu[1];
        let v_rel_d = self.v_down_mps;
        let airspeed = (v_rel_e.powi(2) + v_rel_n.powi(2) + v_rel_d.powi(2)).sqrt();

        // Velocity-aligned pseudo-body frame: x forward (horizontal heading),
        // z down. Yaw is true heading; pitch is descent angle below horizontal.
        let yaw_deg = if v_h > 1e-9 {
            self.v_east_mps.atan2(self.v_north_mps).to_degrees()
        } else {
            0.0
        };
        let pitch_deg = if v_h > 1e-9 || self.v_down_mps.abs() > 1e-9 {
            (-self.v_down_mps.atan2(v_h.max(1e-9))).to_degrees()
        } else {
            0.0
        };

        // Rotate ENU-down acceleration into the pseudo-body (x=heading, z=down).
        let (heading_e, heading_n) = if v_h > 1e-9 {
            (self.v_east_mps / v_h, self.v_north_mps / v_h)
        } else {
            (0.0, 1.0)
        };
        let ax_body = accel_enu_down[0] * heading_e + accel_enu_down[1] * heading_n;
        let az_body = accel_enu_down[2];

        SimulationState {
            time_sec: self.sim_time_sec,
            position: Position {
                lat_deg: self.lat_deg,
                lon_deg: self.lon_deg,
                alt_agl_m: self.alt_agl_m,
            },
            velocity: Velocity {
                true_airspeed_mps: airspeed,
                ground_speed_mps: v_h,
                u_mps: v_h,
                v_mps: 0.0,
                w_mps: self.v_down_mps,
            },
            attitude: Attitude {
                pitch_deg,
                roll_deg: 0.0,
                yaw_deg,
            },
            angular_rates: AngularRates::default(),
            acceleration: Acceleration {
                ax_mps2: ax_body,
                ay_mps2: 0.0,
                az_mps2: az_body,
            },
            aero: AeroState::default(),
            thrust_n: 0.0,
            mach: 0.0,
        }
    }
}

impl Default for ParachuteStage {
    fn default() -> Self {
        Self::new()
    }
}

impl StageRunner for ParachuteStage {
    fn initialize(&mut self, _params: &RocketParams) -> Result<()> {
        *self = Self::new();
        Ok(())
    }

    fn step(&mut self, params: &RocketParams, _input: StageStepInput) -> Result<StageStepOutput> {

        let v_term = self.terminal_velocity_mps(params).max(1e-6);
        let wind_enu = self.wind_enu_at_current_alt(params);
        let mut dt = params.sim.time_step;
        let mut accel_enu_down = [0.0; 3];

        match self.mode {
            Mode::Transient => {
                // Relative air velocity (ENU, vertical-down-positive).
                let v_rel_e = self.v_east_mps - wind_enu[0];
                let v_rel_n = self.v_north_mps - wind_enu[1];
                let v_rel_d = self.v_down_mps; // wind vertical = 0
                let speed_rel =
                    (v_rel_e.powi(2) + v_rel_n.powi(2) + v_rel_d.powi(2)).sqrt();

                let k = G0_MPS2 / (v_term * v_term);
                let a_e = -k * speed_rel * v_rel_e;
                let a_n = -k * speed_rel * v_rel_n;
                let a_d = G0_MPS2 - k * speed_rel * v_rel_d;
                accel_enu_down = [a_e, a_n, a_d];

                // Semi-implicit Euler: update velocity first, advect position
                // with the updated velocity. More stable than forward Euler
                // in the stiff-drag regime near terminal velocity.
                self.v_east_mps += a_e * dt;
                self.v_north_mps += a_n * dt;
                self.v_down_mps += a_d * dt;

                // Settle check against the steady-state envelope.
                let err_e = (self.v_east_mps - wind_enu[0]).abs();
                let err_n = (self.v_north_mps - wind_enu[1]).abs();
                let err_d = (self.v_down_mps - v_term).abs();
                let tol = params.parachute.settle_tol_frac * v_term;
                if err_e < tol && err_n < tol && err_d < tol {
                    self.settle_run_steps = self.settle_run_steps.saturating_add(1);
                    if self.settle_run_steps >= params.parachute.settle_hold_steps {
                        self.mode = Mode::SteadyState;
                    }
                } else {
                    self.settle_run_steps = 0;
                }
            }
            Mode::SteadyState => {
                dt = params.sim.time_step * 10.0;
                self.v_east_mps = wind_enu[0];
                self.v_north_mps = wind_enu[1];
                self.v_down_mps = v_term;
            }
        }

        // Advect position using the (newly updated) velocity.
        let east_step = self.v_east_mps * dt;
        let north_step = self.v_north_mps * dt;
        let (new_lat, new_lon) =
            env::advance_latlon_by_enu(self.lat_deg, self.lon_deg, east_step, north_step);
        self.lat_deg = new_lat;
        self.lon_deg = new_lon;
        self.alt_agl_m -= self.v_down_mps * dt;

        self.sim_time_sec += dt;

        // Terrain-aware termination.
        let mut events = Vec::new();
        let mut terminate = false;
        if !self.landed && env::hit_terrain(params, self.lat_deg, self.lon_deg, self.alt_agl_m) {
            let terrain_h_m = params
                .launch_env
                .terrain
                .as_ref()
                .map(|t| t.altitude_m(self.lat_deg, self.lon_deg))
                .unwrap_or(0.0);
            self.alt_agl_m = terrain_h_m;
            self.v_east_mps = 0.0;
            self.v_north_mps = 0.0;
            self.v_down_mps = 0.0;
            self.landed = true;
            terminate = true;
            events.push(EventKind::ParachuteLanded);
        }

        let state = self.build_state(wind_enu, accel_enu_down);

        Ok(StageStepOutput {
            state,
            events,
            transition_to: if terminate {
                Some(Phase::Completed)
            } else {
                None
            },
            terminate_requested: terminate,
        })
    }
}

// ─── Pure helpers ──────────────────────────────────────────────────────────

/// Piecewise-linear lookup of the terminal-velocity table.
///
/// * `t_sec` is *time since deployment* (t=0 at `ParachuteOpen`).
/// * Values before the first row or after the last row are clamped to the
///   nearest endpoint (no extrapolation).
/// * Returns `0.0` for an empty table (caller should guard).
fn lookup_terminal_mps(table: &[[f64; 2]], t_sec: f64) -> f64 {
    if table.is_empty() {
        return 0.0;
    }
    if t_sec <= table[0][0] {
        return table[0][1];
    }
    let n = table.len();
    if t_sec >= table[n - 1][0] {
        return table[n - 1][1];
    }
    for w in table.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t_sec >= a[0] && t_sec <= b[0] {
            let u = (t_sec - a[0]) / (b[0] - a[0]);
            return a[1] + u * (b[1] - a[1]);
        }
    }
    table[n - 1][1]
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::params::{
        AeroParams, BodyMassParams, Cd0AlphaMachTable, EngineParams, FuelParams, LaunchEnvParams,
        ParachuteParams, SimControl, TankParams,
    };
    use crate::terrain::{FlatTerrain, Terrain};

    // ── lookup_terminal_mps ────────────────────────────────────────────────

    #[test]
    fn terminal_lookup_empty_returns_zero() {
        assert_eq!(lookup_terminal_mps(&[], 1.0), 0.0);
    }

    #[test]
    fn terminal_lookup_clamps_before_start() {
        let t = [[1.0, 30.0], [5.0, 8.0]];
        assert_eq!(lookup_terminal_mps(&t, 0.0), 30.0);
        assert_eq!(lookup_terminal_mps(&t, -2.0), 30.0);
    }

    #[test]
    fn terminal_lookup_clamps_after_end() {
        let t = [[1.0, 30.0], [5.0, 8.0]];
        assert_eq!(lookup_terminal_mps(&t, 10.0), 8.0);
    }

    #[test]
    fn terminal_lookup_interpolates_linearly() {
        let t = [[0.0, 30.0], [1.0, 10.0]];
        let mid = lookup_terminal_mps(&t, 0.5);
        assert!((mid - 20.0).abs() < 1e-9, "got {mid}");
    }

    // ── ParachuteStage — helpers ───────────────────────────────────────────

    fn make_params(
        winds: Vec<[f64; 3]>,
        v_term_table: Vec<[f64; 2]>,
        settle_tol_frac: f64,
        settle_hold_steps: u32,
        terrain: Option<Arc<dyn Terrain>>,
    ) -> RocketParams {
        RocketParams {
            body_mass: BodyMassParams {
                diameter: 0.1,
                total_mass: 10.0,
                cg: [0.5, 0.0, 0.0],
                inertia: [1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
            },
            engine: EngineParams {
                thrust_table: vec![[0.0, 0.0]].into(),
                thruster_pos: [1.0, 0.0, 0.0],
                tank: TankParams {
                    position: [0.5, 0.0, 0.0],
                    drain_position: None,
                    contents: 0.001,
                },
                fuel: FuelParams {
                    position: [0.5, 0.0, 0.0],
                    contents: 0.001,
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
                cn_table: vec![[0.0, 2.0]].into(),
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
                terrain,
                pitch: 90.0,
                roll: 0.0,
                yaw: 0.0,
                winds_table: winds.into(),
                initial_body_velocity_mps: [0.0, 0.0, 0.0],
                initial_position_override: None,
            },
            sim: SimControl {
                flight_duration: 600.0,
                time_step: 0.05,
                apogee_mode: 0,
                csv_sample_interval: 1,
                kml_sample_interval: 10,
                start_sim_time_sec: 0.0,
            },
            parachute: ParachuteParams {
                terminal_velocity_table: v_term_table.into(),
                deploy_trigger: None,
                settle_tol_frac,
                settle_hold_steps,
            },
        }
    }

    fn seeded_stage(
        params: &RocketParams,
        alt_agl_m: f64,
        vel_enu_down: [f64; 3],
    ) -> ParachuteStage {
        let mut s = ParachuteStage::new();
        s.initialize(params).unwrap();
        s.seed_from_ballistic_handoff(
            0.0,
            &Position {
                lat_deg: params.launch_env.latitude,
                lon_deg: params.launch_env.longitude,
                alt_agl_m,
            },
            vel_enu_down,
        );
        s
    }

    // ── Physics ────────────────────────────────────────────────────────────

    #[test]
    fn transient_converges_to_terminal_no_wind() {
        let v_term = 8.0;
        let params = make_params(
            vec![],
            vec![[0.0, v_term], [60.0, v_term]],
            0.05,
            5,
            None,
        );
        // Start at rest, high altitude — should accelerate down toward v_term.
        let mut stage = seeded_stage(&params, 1000.0, [0.0, 0.0, 0.0]);

        // 10 s sim at dt=0.05 → 200 steps. Terminal is reached well within that.
        for _ in 0..200 {
            stage.step(&params, StageStepInput::default()).unwrap();
        }

        assert!(
            (stage.v_down_mps - v_term).abs() < 0.05 * v_term,
            "v_down={} not near v_term={}",
            stage.v_down_mps,
            v_term
        );
        assert!(stage.v_east_mps.abs() < 1e-6);
        assert!(stage.v_north_mps.abs() < 1e-6);
    }

    #[test]
    fn transient_promotes_to_steady_state_after_hold() {
        let v_term = 8.0;
        let params = make_params(
            vec![],
            vec![[0.0, v_term], [60.0, v_term]],
            0.05,
            5,
            None,
        );
        let mut stage = seeded_stage(&params, 1000.0, [0.0, 0.0, 0.0]);
        for _ in 0..400 {
            stage.step(&params, StageStepInput::default()).unwrap();
        }
        assert_eq!(stage.mode, Mode::SteadyState);
    }

    #[test]
    fn no_horizontal_drift_in_still_air_from_rest() {
        let v_term = 6.0;
        let params = make_params(
            vec![],
            vec![[0.0, v_term], [60.0, v_term]],
            0.05,
            5,
            None,
        );
        let start_lat = params.launch_env.latitude;
        let start_lon = params.launch_env.longitude;
        let mut stage = seeded_stage(&params, 500.0, [0.0, 0.0, 0.0]);
        for _ in 0..100 {
            stage.step(&params, StageStepInput::default()).unwrap();
        }
        assert!((stage.lat_deg - start_lat).abs() < 1e-9);
        assert!((stage.lon_deg - start_lon).abs() < 1e-9);
    }

    #[test]
    fn steady_state_follows_wind_exactly() {
        let v_term = 5.0;
        // 10 m/s wind from the north (flows toward south → v_north = -10).
        let winds = vec![
            [0.0, 10.0, 0.0],
            [10000.0, 10.0, 0.0],
        ];
        let params = make_params(
            winds,
            vec![[0.0, v_term], [60.0, v_term]],
            0.05,
            5,
            None,
        );
        let mut stage = seeded_stage(&params, 1000.0, [0.0, -10.0, v_term]);
        // Force steady-state mode directly.
        stage.mode = Mode::SteadyState;

        stage.step(&params, StageStepInput::default()).unwrap();
        assert!(stage.v_east_mps.abs() < 1e-9);
        assert!((stage.v_north_mps - (-10.0)).abs() < 1e-9);
        assert!((stage.v_down_mps - v_term).abs() < 1e-9);
    }

    #[test]
    fn steady_state_follows_time_varying_terminal() {
        // Chute shrinks v_term from 30 → 8 over 1 s (drogue → main transition).
        let table = vec![[0.0, 30.0], [1.0, 8.0], [60.0, 8.0]];
        let params = make_params(vec![], table, 0.05, 5, None);
        let mut stage = seeded_stage(&params, 1000.0, [0.0, 0.0, 30.0]);
        stage.mode = Mode::SteadyState;

        // SteadyState uses `dt = sim.time_step · 10 = 0.5 s`. Each step looks
        // up v_term at the step-entry time then advances the clock, so after
        // 2 steps the *second* lookup used t = 1·dt = 0.5 s — the midpoint
        // of the 0→1 s ramp.
        for _ in 0..2 {
            stage.step(&params, StageStepInput::default()).unwrap();
        }
        let expected = 30.0 + (8.0 - 30.0) * 0.5;
        assert!(
            (stage.v_down_mps - expected).abs() < 1e-6,
            "v_down={} expected≈{}",
            stage.v_down_mps,
            expected
        );
    }

    #[test]
    fn landed_fires_when_altitude_reaches_terrain() {
        let params = make_params(
            vec![],
            vec![[0.0, 10.0], [60.0, 10.0]],
            0.05,
            5,
            Some(Arc::new(FlatTerrain::new(0.0))),
        );
        // Start almost on the ground, descending at 10 m/s.
        let mut stage = seeded_stage(&params, 0.2, [0.0, 0.0, 10.0]);
        let out = stage.step(&params, StageStepInput::default()).unwrap();

        assert!(out.terminate_requested);
        assert_eq!(out.transition_to, Some(Phase::Completed));
        assert!(out.events.contains(&EventKind::ParachuteLanded));
        assert!(stage.landed);
        assert_eq!(stage.alt_agl_m, 0.0);
    }

    #[test]
    fn transition_from_high_speed_to_steady_state_is_smooth() {
        let params = make_params(
            vec![],
            vec![[0.0, 10.0], [60.0, 10.0]],
            0.05,
            5,
            Some(Arc::new(FlatTerrain::new(0.0))),
        );
        let mut stage = seeded_stage(&params, 1000.0, [0.0, 0.0, 100.0]);
        for _ in 0..10 {
            stage.step(&params, StageStepInput::default()).unwrap();
            println!("time: {:.2}s, v_down: {:.2}m/s", stage.sim_time_sec, stage.v_down_mps)
        }
        assert!(stage.v_east_mps.abs() < 1e-9);
        assert!(stage.mode == Mode::Transient);
        insta::assert_snapshot!(stage.v_down_mps);


    }

}
