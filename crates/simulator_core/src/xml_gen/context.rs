//! Template context: maps `RocketParams` (SI) to the variable names used
//! in the Jinja2 templates.
//!
//! The current templates expect several values to be pre-converted into
//! JSBSim internal units (fps/lbs/rad). Those conversions are done here.

use std::f64::consts::PI;

use serde::Serialize;

use crate::params::RocketParams;

const MPS_TO_FPS: f64 = 3.280_839_895_013_123;
const N_TO_LBF: f64 = 0.224_808_943_870_96;
const KG_TO_LBS: f64 = 2.204_622_621_848_775_7;
const DEG_TO_RAD: f64 = PI / 180.0;

/// Serialised to `minijinja::Value` and passed to templates.
#[derive(Debug, Serialize)]
pub struct XmlContext {
    // ── SimControl ─────────────────────────────────────────────────────────
    pub flight_duration: f64,
    pub time_step: f64,

    // ── LaunchEnvParams / liftoff.xml ─────────────────────────────────────
    pub latitude: f64,
    pub longitude: f64,
    pub elevation: f64,
    pub pitch: f64,
    pub roll: f64,
    pub yaw: f64,
    pub velocity_u: f64,
    pub velocity_v: f64,
    pub velocity_w: f64,

    /// [[alt_m, dir_rad, speed_fps], …]
    pub winds_table: Vec<[f64; 3]>,

    // ── BodyMassParams / aircraft.xml ─────────────────────────────────────
    pub frontal_area_m2: f64,
    pub diameter: f64,
    pub body_radius: f64,

    // ── AeroParams ────────────────────────────────────────────────────────
    pub cp_x: f64,
    pub cp_y: f64,
    pub cp_z: f64,
    pub cp_mach_table: Vec<[f64; 2]>,
    /// CD0(alpha, mach) tableData format:
    /// first row = mach keys, remaining rows = [alpha, cd...].
    pub cd0_alpha_mach_table: Vec<Vec<f64>>,
    /// Integrated CN-alpha slope vs Mach: [[mach, CNα_total], …].
    pub cn_table: Vec<[f64; 2]>,
    /// Integrated CS-beta slope vs Mach: [[mach, CSβ_total], …].
    pub cs_table: Vec<[f64; 2]>,
    pub roll_damping_coefficient: f64,
    pub pitch_damping_coefficient: f64,
    pub yaw_damping_coefficient: f64,

    // ── BodyMassParams (mass part) ───────────────────────────────────────
    pub inertia_xx: f64,
    pub inertia_yy: f64,
    pub inertia_zz: f64,
    pub inertia_xy: f64,
    pub inertia_xz: f64,
    pub inertia_yz: f64,
    /// JSBSim `emptywt` in kg.
    pub emptywt_kg: f64,
    pub cg_x: f64,
    pub cg_y: f64,
    pub cg_z: f64,

    // ── EngineParams — oxidizer tank ──────────────────────────────────────
    pub oxidizer_x: f64,
    pub oxidizer_y: f64,
    pub oxidizer_z: f64,
    pub oxidizer_drain_x: f64,
    pub oxidizer_drain_y: f64,
    pub oxidizer_drain_z: f64,
    pub oxidizer_contents: f64,
    pub oxidizer_contents_lbs: f64,

    // ── EngineParams — fuel grain ─────────────────────────────────────────
    pub fuel_x: f64,
    pub fuel_y: f64,
    pub fuel_z: f64,
    pub fuel_contents_kg: f64,
    /// Burnable fuel mass in lbs — template's `<gain>` for the fuel tank's
    /// `contents-lbs` channel: `(contents - after_burn) · KG_TO_LBS`.
    pub fuel_drain_lbs: f64,

    // ── Thruster / curves ─────────────────────────────────────────────────
    /// Thruster position relative to CG (m), used by `<external_reactions>`.
    pub thruster_rel_x: f64,
    pub thruster_rel_y: f64,
    pub thruster_rel_z: f64,
    /// [[t_sec, thrust_lbf], …]
    pub thrust_table: Vec<[f64; 2]>,
    /// [[t_sec, fraction 0–1], …]
    pub fuel_remaining_table: Vec<[f64; 2]>,
}

impl From<&RocketParams> for XmlContext {
    fn from(p: &RocketParams) -> Self {
        let t = &p.engine.tank;
        let f = &p.engine.fuel;
        let oxidizer_drain = t.drain_position.unwrap_or(t.position);

        let mut winds = p.launch_env.winds_table.to_vec();
        winds.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let winds_table = winds
            .into_iter()
            .map(|[alt_m, speed_mps, dir_deg]| {
                [alt_m, dir_deg * DEG_TO_RAD, speed_mps * MPS_TO_FPS]
            })
            .collect();

        let mut thrust_table: Vec<_> = p
            .engine
            .thrust_table
            .iter()
            .map(|[t_sec, thrust_n]| [*t_sec, thrust_n * N_TO_LBF])
            .collect();

        if let Some(&[last_t, _]) = thrust_table.last() {
            thrust_table.push([last_t + 0.1, 0.0]); // 例: 0.1秒後に推力0
            thrust_table.push([last_t + 0.2, 0.0]); // 例: さらに0.1秒後に推力0
        } else {
            // 元のデータが空だった場合のフォールバック
            thrust_table.extend([[0.0, 0.0], [0.1, 0.0]]);
        }

        let sum_thrust_lbf_num: f64 = thrust_table.iter().map(|[_, thrust_lbf]| thrust_lbf).sum();

        let fuel_remaining_table = thrust_table
            .iter()
            .scan(0.0, |acc, &[t_sec, thrust_lbf]| {
                *acc += thrust_lbf;
                let remaining_fuel_ratio = 1.0 - (*acc / sum_thrust_lbf_num);
                Some([t_sec, remaining_fuel_ratio.max(0.0)])
            })
            .collect();

        let body_radius = p.body_mass.diameter / 2.0;

        let mut cd0_alpha_mach_table =
            Vec::with_capacity(1 + p.aero.cd0_alpha_mach_table.rows.len());
        cd0_alpha_mach_table.push(p.aero.cd0_alpha_mach_table.mach_keys.to_vec());
        cd0_alpha_mach_table.extend(p.aero.cd0_alpha_mach_table.rows.iter().cloned());

        // Position: rail-exit handoff overrides pad coordinates.
        let (latitude, longitude, altitude_agl_m) = match p.launch_env.initial_position_override {
            Some(ip) => (ip.latitude_deg, ip.longitude_deg, ip.altitude_agl_m),
            None => (
                p.launch_env.latitude,
                p.launch_env.longitude,
                p.launch_env.elevation,
            ),
        };

        Self {
            // SimControl
            flight_duration: p.sim.flight_duration,
            time_step: p.sim.time_step,

            // Launch / liftoff — use handoff override if present.
            latitude,
            longitude,
            elevation: altitude_agl_m,
            pitch: p.launch_env.pitch,
            roll: p.launch_env.roll,
            yaw: p.launch_env.yaw,
            velocity_u: p.launch_env.initial_body_velocity_mps[0],
            velocity_v: p.launch_env.initial_body_velocity_mps[1],
            velocity_w: p.launch_env.initial_body_velocity_mps[2],

            // Environment
            winds_table,

            // Body
            frontal_area_m2: PI * body_radius * body_radius,
            diameter: p.body_mass.diameter,
            body_radius,

            // Aero
            cp_x: p.aero.cp_at_launch[0],
            cp_y: p.aero.cp_at_launch[1],
            cp_z: p.aero.cp_at_launch[2],
            cp_mach_table: p.aero.cp_mach_table.to_vec(),
            cd0_alpha_mach_table,
            cn_table: p.aero.cn_table.to_vec(),
            cs_table: p.aero.cs_table.iter().map(|&[m, c]| [m, -c]).collect(),
            roll_damping_coefficient: p.aero.roll_damping_coefficient,
            pitch_damping_coefficient: p.aero.pitch_damping_coefficient,
            yaw_damping_coefficient: p.aero.yaw_damping_coefficient,

            // Mass
            inertia_xx: p.body_mass.inertia[0],
            inertia_yy: p.body_mass.inertia[1],
            inertia_zz: p.body_mass.inertia[2],
            inertia_xy: p.body_mass.inertia[3],
            inertia_xz: p.body_mass.inertia[4],
            inertia_yz: p.body_mass.inertia[5],
            // `total_mass` includes all propellant at launch, so the JSBSim
            // empty weight must exclude both oxidizer and fuel contents.
            emptywt_kg: p.body_mass.total_mass - t.contents - (f.contents - f.after_burn).max(0.0),
            cg_x: p.body_mass.cg[0],
            cg_y: p.body_mass.cg[1],
            cg_z: p.body_mass.cg[2],

            // Oxidizer tank
            oxidizer_x: t.position[0],
            oxidizer_y: t.position[1],
            oxidizer_z: t.position[2],
            oxidizer_drain_x: oxidizer_drain[0],
            oxidizer_drain_y: oxidizer_drain[1],
            oxidizer_drain_z: oxidizer_drain[2],
            oxidizer_contents: t.contents,
            oxidizer_contents_lbs: t.contents * KG_TO_LBS,

            // Fuel tank
            fuel_x: f.position[0],
            fuel_y: f.position[1],
            fuel_z: f.position[2],
            fuel_contents_kg: (f.contents - f.after_burn).max(0.0),
            fuel_drain_lbs: (f.contents - f.after_burn).max(0.0) * KG_TO_LBS,
            // Thruster and curves
            thruster_rel_x: p.engine.thruster_pos[0] - p.body_mass.cg[0],
            thruster_rel_y: p.engine.thruster_pos[1] - p.body_mass.cg[1],
            thruster_rel_z: p.engine.thruster_pos[2] - p.body_mass.cg[2],
            thrust_table,
            fuel_remaining_table,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{
        AeroParams, BodyMassParams, Cd0AlphaMachTable, EngineParams, FuelParams, LaunchEnvParams,
        SimControl, TankParams,
    };

    fn minimal_params() -> RocketParams {
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
                pitch: 90.0,
                roll: 0.0,
                yaw: 0.0,
                winds_table: Vec::<[f64; 3]>::new().into(),
                initial_body_velocity_mps: [0.0, 0.0, 0.0],
                initial_position_override: None,
            },
            sim: SimControl::default(),
            parachute: Default::default(),
        }
    }

    /// Default initial_body_velocity (zero) passes through.
    #[test]
    fn zero_initial_body_velocity_passes_through() {
        let ctx = XmlContext::from(&minimal_params());
        assert_eq!(ctx.velocity_u, 0.0);
        assert_eq!(ctx.velocity_v, 0.0);
        assert_eq!(ctx.velocity_w, 0.0);
    }

    /// Non-zero initial body velocity (rail-exit handoff) appears in
    /// the context unchanged, so `liftoff.xml.j2` receives it as-is.
    #[test]
    fn rail_exit_body_velocity_passes_through() {
        let mut params = minimal_params();
        params.launch_env.initial_body_velocity_mps = [25.0, 0.5, -0.25];
        let ctx = XmlContext::from(&params);
        assert_eq!(ctx.velocity_u, 25.0);
        assert_eq!(ctx.velocity_v, 0.5);
        assert_eq!(ctx.velocity_w, -0.25);
    }

    /// Without an override, pad coordinates flow into the context.
    #[test]
    fn pad_coordinates_used_when_no_override() {
        let mut params = minimal_params();
        params.launch_env.latitude = 35.5;
        params.launch_env.longitude = 139.7;
        params.launch_env.elevation = 12.0;
        let ctx = XmlContext::from(&params);
        assert_eq!(ctx.latitude, 35.5);
        assert_eq!(ctx.longitude, 139.7);
        assert_eq!(ctx.elevation, 12.0);
    }

    /// `initial_position_override` takes precedence over pad coordinates.
    #[test]
    fn initial_position_override_replaces_pad_coords() {
        let mut params = minimal_params();
        params.launch_env.latitude = 35.5;
        params.launch_env.longitude = 139.7;
        params.launch_env.elevation = 12.0;
        params.launch_env.initial_position_override = Some(crate::params::InitialPosition {
            latitude_deg: 36.0,
            longitude_deg: 140.1,
            altitude_agl_m: 5.0,
        });
        let ctx = XmlContext::from(&params);
        assert_eq!(ctx.latitude, 36.0);
        assert_eq!(ctx.longitude, 140.1);
        assert_eq!(ctx.elevation, 5.0);
    }
}
