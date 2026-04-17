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
    /// 0 = full flight, 1 = terminate at apogee.
    pub apogee_mode: u8,

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

    // ── Environment / simulation.xml ──────────────────────────────────────
    pub launcher_height: f64,
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

        let mut winds = p.launch_env.winds_table.clone();
        winds.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let winds_table = winds
            .into_iter()
            .map(|[alt_m, speed_mps, dir_deg]| [alt_m, dir_deg * DEG_TO_RAD, speed_mps * MPS_TO_FPS])
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

        let sum_thrust_lbf_num: f64 = thrust_table
            .iter()
            .map(|[_, thrust_lbf]|  thrust_lbf)
            .sum();


        let fuel_remaining_table = thrust_table
            .iter()
            .scan(0.0, |acc, &[t_sec, thrust_lbf]| {
                *acc += thrust_lbf;
                let remaining_fuel_ratio = 1.0 - (*acc / sum_thrust_lbf_num);
                Some([t_sec, remaining_fuel_ratio.max(0.0)])
            })
            .collect();


        let body_radius = p.body_mass.diameter / 2.0;

        let mut cd0_alpha_mach_table = Vec::with_capacity(1 + p.aero.cd0_alpha_mach_table.rows.len());
        cd0_alpha_mach_table.push(p.aero.cd0_alpha_mach_table.mach_keys.clone());
        cd0_alpha_mach_table.extend(p.aero.cd0_alpha_mach_table.rows.clone());

        Self {
            // SimControl
            flight_duration: p.sim.flight_duration,
            time_step: p.sim.time_step,
            apogee_mode: p.sim.apogee_mode,

            // Launch / liftoff
            latitude: p.launch_env.latitude,
            longitude: p.launch_env.longitude,
            elevation: p.launch_env.elevation,
            pitch: p.launch_env.pitch,
            roll: p.launch_env.roll,
            yaw: p.launch_env.yaw,
            velocity_u: 0.0,
            velocity_v: 0.0,
            velocity_w: 0.0,

            // Environment
            launcher_height: p.launch_env.launcher_height,
            winds_table,

            // Body
            frontal_area_m2: PI * body_radius * body_radius,
            diameter: p.body_mass.diameter,
            body_radius,

            // Aero
            cp_x: p.aero.cp_at_launch[0],
            cp_y: p.aero.cp_at_launch[1],
            cp_z: p.aero.cp_at_launch[2],
            cp_mach_table: p.aero.cp_mach_table.clone(),
            cd0_alpha_mach_table,
            cn_table: p.aero.cn_table.clone(),
            cs_table: p.aero.cs_table.clone(),
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
            emptywt_kg: p.body_mass.total_mass - f.contents,
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
            // Thruster and curves
            thruster_rel_x: p.engine.thruster_pos[0] - p.body_mass.cg[0],
            thruster_rel_y: p.engine.thruster_pos[1] - p.body_mass.cg[1],
            thruster_rel_z: p.engine.thruster_pos[2] - p.body_mass.cg[2],
            thrust_table,
            fuel_remaining_table,
        }
    }
}
