//! Launch rail phase: 1-DOF kinematics along the rail axis.
//!
//! Forces along the rail (+ = forward / up-the-rail):
//!   thrust            : T(t)  via linear interp of `engine.thrust_table`
//!   gravity component : −m·g·sin(pitch)
//!   aerodynamic drag  : ignored on the rail (low-speed, short duration)
//!
//! Wind is projected onto the rail axis and reflected in
//! `SimulationState.velocity.true_airspeed_mps` as the body-axis
//! relative airspeed.
//!
//! Constraint: the rocket cannot translate below its initial position
//! (the launch pad is solid). If the along-rail displacement would go
//! negative, it is clamped to 0 and the along-rail velocity is zeroed,
//! so that a rocket whose thrust is below gravity simply sits on the
//! pad.

use crate::output::{
    Acceleration, AeroState, AngularRates, Attitude, Position, SimulationState, Velocity,
};
use crate::progress::EventKind;
use crate::simple_simulator::env::{self, latlon_to_local, G0_MPS2};
use crate::simple_simulator::{StageRunner, StageStepInput, StageStepOutput};
use crate::{Result, RocketParams};

/// Launch rail phase: 1-D along-rail integrator.
#[derive(Debug, Clone)]
pub struct LaunchRailStage {
    /// Distance travelled along the rail axis, from the pad (m). Always ≥ 0.
    distance_m: f64,
    /// Velocity along the rail axis (m/s). Forward-up is positive.
    velocity_mps: f64,
    sim_time_sec: f64,
    /// Running thrust-impulse integral ∫T dt (N·s).
    consumed_impulse_ns: f64,
    /// Total impulse of the thrust curve (N·s), computed once in `initialize`.
    total_impulse_ns: f64,
    launch_clear_emitted: bool,
}

impl LaunchRailStage {
    pub fn new() -> Self {
        Self {
            distance_m: 0.0,
            velocity_mps: 0.0,
            sim_time_sec: 0.0,
            consumed_impulse_ns: 0.0,
            total_impulse_ns: 0.0,
            launch_clear_emitted: false,
        }
    }

    fn choose_dt_sec(&self, params: &RocketParams) -> f64 {
        // Shared dt with JSBSim by default; adequate for trapezoidal Euler
        // given the short rail transit time (~0.1–1 s).
        params.sim.time_step
    }

    /// Unit vector of the rail in ENU (east, north, up).
    ///
    /// Convention (matches the existing position mapping):
    ///   pitch = elevation from horizontal (90° = vertical up)
    ///   yaw   = compass heading           (0°  = north, 90° = east)
    fn rail_axis_enu(params: &RocketParams) -> [f64; 3] {
        let pitch = params.launch_env.pitch.to_radians();
        let yaw = params.launch_env.yaw.to_radians();
        let cp = pitch.cos();
        [yaw.sin() * cp, yaw.cos() * cp, pitch.sin()]
    }

    /// Linear interpolation of the thrust curve at time `t`.
    /// Before the first sample → first value; after the last sample → 0.
    fn interp_thrust_n(thrust_table: &[[f64; 2]], t: f64) -> f64 {
        if thrust_table.is_empty() {
            return 0.0;
        }
        if t <= thrust_table[0][0] {
            return thrust_table[0][1];
        }
        let n = thrust_table.len();
        if t >= thrust_table[n - 1][0] {
            return 0.0;
        }
        for w in thrust_table.windows(2) {
            let (a, b) = (w[0], w[1]);
            if t >= a[0] && t <= b[0] {
                let u = (t - a[0]) / (b[0] - a[0]);
                return a[1] + u * (b[1] - a[1]);
            }
        }
        0.0
    }

    /// Trapezoidal integration of the thrust curve → total impulse.
    fn integrate_total_impulse(thrust_table: &[[f64; 2]]) -> f64 {
        let mut sum = 0.0;
        for w in thrust_table.windows(2) {
            let (a, b) = (w[0], w[1]);
            sum += 0.5 * (a[1] + b[1]) * (b[0] - a[0]);
        }
        sum
    }

    fn propellant_total_kg(params: &RocketParams) -> f64 {
        let engine = &params.engine;
        let fuel_burn = (engine.fuel.contents - engine.fuel.after_burn).max(0.0);
        engine.tank.contents + fuel_burn
    }

    /// Current vehicle mass, using consumed-impulse fraction to deplete propellant.
    fn current_mass_kg(&self, params: &RocketParams) -> f64 {
        let total_prop = Self::propellant_total_kg(params);
        let empty = (params.body_mass.total_mass - total_prop).max(0.0);
        let consumed_fraction = if self.total_impulse_ns > 0.0 {
            (self.consumed_impulse_ns / self.total_impulse_ns).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let remaining_prop = total_prop * (1.0 - consumed_fraction);
        (empty + remaining_prop).max(1e-6)
    }

    fn to_public_state(
        &self,
        params: &RocketParams,
        wind_along_rail_mps: f64,
        thrust_n: f64,
        accel_along_rail: f64,
    ) -> SimulationState {
        let launch = &params.launch_env;

        let pitch = launch.pitch.to_radians();
        let yaw = launch.yaw.to_radians();

        let horizontal_m = self.distance_m * pitch.cos();
        let up_m = params.launch_env.elevation + self.distance_m * pitch.sin();

        let north_m = horizontal_m * yaw.cos();
        let east_m = horizontal_m * yaw.sin();

        let (lat_deg, lon_deg) =
            env::advance_latlon_by_enu(launch.latitude, launch.longitude, east_m, north_m);

        let (down_range_m, local_x_m, local_y_m) =
            latlon_to_local(lat_deg, lon_deg, launch.latitude, launch.longitude, launch.yaw);

        let v = self.velocity_mps;
        let v_rel_along_rail = v - wind_along_rail_mps;

        SimulationState {
            time_sec: self.sim_time_sec,
            position: Position {
                lat_deg,
                lon_deg,
                alt_agl_m: up_m.max(0.0),
                down_range_m,
                local_x_m,
                local_y_m,
            },
            velocity: Velocity {
                true_airspeed_mps: v_rel_along_rail.abs(),
                ground_speed_mps: (v * pitch.cos()).abs(),
                u_mps: v,
                v_mps: 0.0,
                w_mps: 0.0,
            },
            attitude: Attitude {
                pitch_deg: launch.pitch,
                roll_deg: launch.roll,
                yaw_deg: launch.yaw,
            },
            angular_rates: AngularRates::default(),
            acceleration: Acceleration {
                ax_mps2: accel_along_rail,
                ay_mps2: 0.0,
                az_mps2: 0.0,
            },
            aero: AeroState::default(),
            thrust_n,
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
    fn initialize(&mut self, params: &RocketParams) -> Result<()> {
        self.distance_m = 0.0;
        self.velocity_mps = 0.0;
        self.sim_time_sec = 0.0;
        self.consumed_impulse_ns = 0.0;
        self.total_impulse_ns = Self::integrate_total_impulse(&params.engine.thrust_table);
        self.launch_clear_emitted = false;
        Ok(())
    }

    fn step(&mut self, params: &RocketParams, _input: StageStepInput) -> Result<StageStepOutput> {
        let dt = self.choose_dt_sec(params);
        let pitch = params.launch_env.pitch.to_radians();

        // Instantaneous forces along the rail axis (+ = up the rail).
        let thrust_n = Self::interp_thrust_n(&params.engine.thrust_table, self.sim_time_sec);
        let mass_kg = self.current_mass_kg(params);
        let gravity_component = G0_MPS2 * pitch.sin();
        let accel = thrust_n / mass_kg - gravity_component;

        // Trapezoidal Euler — exact for constant acceleration.
        let v_old = self.velocity_mps;
        let v_new = v_old + accel * dt;
        let x_new = self.distance_m + 0.5 * (v_old + v_new) * dt;

        // One-sided constraint: the pad is solid, so the rocket cannot
        // translate below its initial position. This also holds the
        // rocket on the pad when thrust < gravity component.
        if x_new <= 0.0 {
            self.distance_m = 0.0;
            self.velocity_mps = 0.0;
        } else {
            self.distance_m = x_new;
            self.velocity_mps = v_new;
        }

        self.consumed_impulse_ns += thrust_n * dt;
        self.sim_time_sec += dt;

        // Wind projected onto the rail axis.
        let e_rail = Self::rail_axis_enu(params);
        let alt_msl_m = params.launch_env.elevation + self.distance_m * pitch.sin();
        let wind_enu = env::wind_enu_at_alt(&params.launch_env.winds_table, alt_msl_m);
        let wind_along_rail = env::dot3(wind_enu, e_rail);

        let state = self.to_public_state(params, wind_along_rail, thrust_n, accel);

        let mut events = Vec::new();
        let mut completed = false;

        if !self.launch_clear_emitted && self.distance_m >= params.launch_env.rail_length_m {
            self.launch_clear_emitted = true;
            events.push(EventKind::LaunchClear);
            completed = true;
        }

        Ok(StageStepOutput {
            state,
            events,
            completed,
        })
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{
        AeroParams, BodyMassParams, Cd0AlphaMachTable, EngineParams, FuelParams, LaunchEnvParams,
        SimControl, TankParams,
    };

    /// Minimal `RocketParams` for rail tests.
    fn make_params(
        pitch_deg: f64,
        yaw_deg: f64,
        thrust_table: Vec<[f64; 2]>,
        winds_table: Vec<[f64; 3]>,
        total_mass: f64,
    ) -> RocketParams {
        RocketParams {
            body_mass: BodyMassParams {
                diameter: 0.1,
                total_mass,
                cg: [0.5, 0.0, 0.0],
                inertia: [1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
            },
            engine: EngineParams {
                thrust_table: thrust_table.into(),
                thruster_pos: [1.0, 0.0, 0.0],
                tank: TankParams {
                    position: [0.5, 0.0, 0.0],
                    drain_position: None,
                    // Tiny propellant so mass is effectively constant in tests.
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
                rail_length_m: 5.0,
                pitch: pitch_deg,
                roll: 0.0,
                yaw: yaw_deg,
                winds_table: winds_table.into(),
                initial_body_velocity_mps: [0.0, 0.0, 0.0],
                initial_position_override: None,
            },
            sim: SimControl {
                flight_duration: 10.0,
                time_step: 0.01,
                output_decimation_rate: 1,
                start_sim_time_sec: 0.0,
            },
            parachute: Default::default(),
        }
    }

    /// Zero thrust, vertical rail → rocket must stay on the pad (pad is solid).
    #[test]
    fn hold_down_when_thrust_zero_vertical() {
        let params = make_params(90.0, 0.0, vec![[0.0, 0.0], [1.0, 0.0]], vec![], 10.0);
        let mut stage = LaunchRailStage::new();
        stage.initialize(&params).unwrap();

        for _ in 0..100 {
            let out = stage.step(&params, StageStepInput::default()).unwrap();
            assert_eq!(out.state.position.alt_agl_m, 0.0);
            assert_eq!(out.state.velocity.u_mps, 0.0);
        }
        assert_eq!(stage.distance_m, 0.0);
        assert_eq!(stage.velocity_mps, 0.0);
    }

    /// Thrust below gravity on a vertical rail → still held on the pad.
    #[test]
    fn thrust_below_gravity_vertical_stays_put() {
        let mass = 10.0;
        let half_weight_n = 0.5 * mass * G0_MPS2;
        let params = make_params(
            90.0,
            0.0,
            vec![[0.0, half_weight_n], [10.0, half_weight_n]],
            vec![],
            mass,
        );
        let mut stage = LaunchRailStage::new();
        stage.initialize(&params).unwrap();

        for _ in 0..200 {
            let _ = stage.step(&params, StageStepInput::default()).unwrap();
        }
        assert_eq!(stage.distance_m, 0.0);
        assert_eq!(stage.velocity_mps, 0.0);
    }

    /// Horizontal rail, zero thrust → no along-rail gravity, rocket stays put.
    #[test]
    fn no_motion_when_thrust_zero_horizontal() {
        let params = make_params(0.0, 0.0, vec![[0.0, 0.0], [1.0, 0.0]], vec![], 10.0);
        let mut stage = LaunchRailStage::new();
        stage.initialize(&params).unwrap();

        for _ in 0..50 {
            let _ = stage.step(&params, StageStepInput::default()).unwrap();
        }
        assert!(stage.distance_m.abs() < 1e-12);
        assert!(stage.velocity_mps.abs() < 1e-12);
    }

    /// Constant thrust strong enough to clear the rail: trapezoidal Euler
    /// is exact for constant acceleration, so velocity must match `a·t`.
    #[test]
    fn constant_thrust_vertical_matches_analytic() {
        let mass = 10.0;
        // Large thrust, large total impulse, tiny propellant → ≈ constant mass.
        let thrust = 300.0;
        let params = make_params(90.0, 0.0, vec![[0.0, thrust], [10.0, thrust]], vec![], mass);
        let mut stage = LaunchRailStage::new();
        stage.initialize(&params).unwrap();

        let a_expected = thrust / mass - G0_MPS2;
        let dt = params.sim.time_step;

        // Tolerance loose enough to absorb the tiny mass change from
        // impulse-fraction depletion (propellant is deliberately minimal).
        for i in 1..=20 {
            let out = stage.step(&params, StageStepInput::default()).unwrap();
            let t = i as f64 * dt;
            let v_expected = a_expected * t;
            let x_expected = 0.5 * a_expected * t * t;
            assert!(
                (out.state.velocity.u_mps - v_expected).abs() < 1e-4,
                "v: got {}, expected {}",
                out.state.velocity.u_mps,
                v_expected
            );
            assert!(
                (out.state.position.alt_agl_m - x_expected).abs() < 1e-4,
                "x: got {}, expected {}",
                out.state.position.alt_agl_m,
                x_expected
            );
        }
    }

    /// `LaunchClear` must fire exactly once, and request transition to Ballistic.
    #[test]
    fn launch_clear_fires_once() {
        let mass = 10.0;
        let params = make_params(90.0, 0.0, vec![[0.0, 1000.0], [10.0, 1000.0]], vec![], mass);
        let mut stage = LaunchRailStage::new();
        stage.initialize(&params).unwrap();

        let mut clears = 0;
        let mut saw_transition = false;
        for _ in 0..2000 {
            let out = stage.step(&params, StageStepInput::default()).unwrap();
            clears += out
                .events
                .iter()
                .filter(|e| matches!(e, EventKind::LaunchClear))
                .count();
            if out.completed == true {
                saw_transition = true;
            }
            if stage.distance_m >= 5.0 + 1.0 {
                break;
            }
        }
        assert_eq!(clears, 1);
        assert!(saw_transition);
    }

    /// Crosswind (perpendicular to vertical rail) must project to ≈0
    /// along the rail — airspeed ≈ body-axis speed.
    #[test]
    fn crosswind_does_not_affect_airspeed_on_vertical_rail() {
        // Rail: pitch=90, yaw=0 → e_rail = (0, 0, 1) (straight up).
        // Wind from east (dir_deg=90) at 10 m/s → ENU = (-10, 0, 0).
        // Dot with rail axis ≈ 0.
        let mass = 10.0;
        let params = make_params(
            90.0,
            0.0,
            vec![[0.0, 300.0], [10.0, 300.0]],
            vec![[0.0, 10.0, 90.0], [1000.0, 10.0, 90.0]],
            mass,
        );
        let mut stage = LaunchRailStage::new();
        stage.initialize(&params).unwrap();

        for _ in 0..10 {
            let out = stage.step(&params, StageStepInput::default()).unwrap();
            // |v_rel| should equal |v_body| within float noise.
            assert!(
                (out.state.velocity.true_airspeed_mps - out.state.velocity.u_mps.abs()).abs()
                    < 1e-9,
                "airspeed {} vs body-u {}",
                out.state.velocity.true_airspeed_mps,
                out.state.velocity.u_mps
            );
        }
    }

    /// Headwind along a horizontal rail must increase airspeed beyond body speed.
    ///
    /// Horizontal rail towards north: pitch=0, yaw=0 → e_rail = (0, 1, 0).
    /// Wind from north at 10 m/s → ENU = (0, -10, 0).
    /// `wind · e_rail` = −10 (wind opposes rocket motion along rail).
    /// v_rel = v_rocket − (−10) = v_rocket + 10.
    #[test]
    fn headwind_increases_airspeed_on_horizontal_rail() {
        let mass = 10.0;
        let wind = 10.0;
        let params = make_params(
            0.0,
            0.0,
            vec![[0.0, 200.0], [10.0, 200.0]],
            vec![[0.0, wind, 0.0], [1000.0, wind, 0.0]],
            mass,
        );
        let mut stage = LaunchRailStage::new();
        stage.initialize(&params).unwrap();

        for _ in 0..5 {
            let out = stage.step(&params, StageStepInput::default()).unwrap();
            let body_v = out.state.velocity.u_mps;
            let airspeed = out.state.velocity.true_airspeed_mps;
            assert!(
                (airspeed - (body_v + wind)).abs() < 1e-9,
                "airspeed {} != body_v {} + wind {}",
                airspeed,
                body_v,
                wind
            );
        }
    }

    /// Wind interpolation clamps outside table range.
    #[test]
    fn wind_table_is_altitude_clamped() {
        let table = vec![[0.0, 5.0, 0.0], [1000.0, 15.0, 0.0]];
        let below = env::wind_enu_at_alt(&table, -100.0);
        let above = env::wind_enu_at_alt(&table, 5000.0);
        // north component = -speed * cos(0) = -speed
        assert!((below[1] - (-5.0)).abs() < 1e-12);
        assert!((above[1] - (-15.0)).abs() < 1e-12);
    }
}
