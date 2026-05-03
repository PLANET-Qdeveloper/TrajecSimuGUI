//! Convert user-facing `Config` + CSV tables into a `RocketParams`.

use std::cmp::min;
use anyhow::{bail, Result};

use simulator_core::params::{
    AeroParams, BodyMassParams, EngineParams, FuelParams, LaunchEnvParams, ParachuteParams,
    RocketParams, SimControl, TankParams,
};
use simulator_core::progress::DelayedBranchTrigger;
use simulator_core::EventKind;

use crate::config::Config;
use crate::csv_loader;

/// Top altitude for the calm-wind sentinel table.  JSBSim clamps at
/// endpoints, so any value well above expected apogee is fine.
const WINDS_TABLE_TOP_M: f64 = 30_000.0;

/// Altitude grid for power-law tables:
///   0..=200 m   →  5 m steps
///   210..=1000 m → 10 m steps
///   1100..=10000 m → 100 m steps
const POWER_LAW_MAX_M: u32 = 10_000;

/// Build a power-law winds-aloft table `[[alt_m, speed_mps, dir_deg], …]`.
///
/// - `speed_mps`    — reference wind speed at `h_ref_m`
/// - `direction_deg`— meteorological "from" direction (constant with altitude)
/// - `h_ref_m`      — reference height (≥ 10 m; caller must clamp)
/// - `alpha`        — power-law exponent (1/6 ≈ open terrain)
pub fn power_law_winds_table(
    speed_mps: f64,
    direction_deg: f64,
    h_ref_m: f64,
    alpha: f64,
) -> Vec<[f64; 3]> {
    let altitudes = (0u32..=200)
        .step_by(5)
        .chain((210u32..=1000).step_by(10))
        .chain((1100u32..=POWER_LAW_MAX_M).step_by(100));

    altitudes
        .map(|h_m| {
            let h = h_m as f64;
            let v = if h_m == 0 {
                0.0
            } else {
                speed_mps * (h / h_ref_m).powf(alpha)
            };
            [h, v, direction_deg]
        })
        .collect()
}

/// Build the winds-aloft table from config.
///
/// Source precedence:
/// - `launch.wind_table` CSV (preferred when present)
/// - `wind_speed_mps` + `wind_direction_deg` → power-law
/// - neither → calm (2-point zero table)
fn build_winds_table(cfg: &Config) -> Result<Vec<[f64; 3]>> {
    match (
        &cfg.launch.wind_table,
        cfg.launch.wind_speed_mps,
        cfg.launch.wind_direction_deg,
    ) {
        // ── CSV table ────────────────────────────────────────────────────
        (Some(path), _, _) => csv_loader::load_wind_table(path),

        // ── Power law ────────────────────────────────────────────────────
        (None, Some(speed), Some(direction)) => {
            let h_ref = cfg
                .launch
                .wind_reference_alt
                .unwrap_or(cfg.launch.elevation)
                .max(10.0);
            let alpha = cfg.launch.wind_power_exponent;
            Ok(power_law_winds_table(speed, direction, h_ref, alpha))
        }

        // ── Incomplete scalar pair ────────────────────────────────────────
        (None, Some(_), None) => {
            bail!("wind_direction_deg is required when wind_speed_mps is set")
        }
        (None, None, Some(_)) => {
            bail!("wind_speed_mps is required when wind_direction_deg is set")
        }

        // ── No wind configured ───────────────────────────────────────────
        (None, None, None) => Ok(vec![
            [0.0, 0.0, 0.0],
            [WINDS_TABLE_TOP_M, 0.0, 0.0],
        ]),
    }
}

pub fn assemble(cfg: &Config) -> Result<RocketParams> {
    let thrust_table = csv_loader::load_1d(&cfg.engine.thrust_table)?;
    let cp_mach_table = csv_loader::load_1d(&cfg.aero.cp_mach_table)?;
    let cn_table = csv_loader::load_1d(&cfg.aero.cn_table)?;
    let cs_table = csv_loader::load_1d(&cfg.aero.cs_table)?;
    let cd0_alpha_mach_table = csv_loader::load_cd_table_deg(&cfg.aero.cd0_alpha_mach_table)?;

    let winds_table = build_winds_table(cfg)?;

    let parachute = match &cfg.parachute {
        None => ParachuteParams::default(),
        Some(p) => {
            let terminal_velocity_table = csv_loader::load_1d(&p.terminal_velocity_table)?;
            ParachuteParams {
                terminal_velocity_table: terminal_velocity_table.into(),
                deploy_trigger: Some(DelayedBranchTrigger {
                    origin: EventKind::Apogee,
                    delay_sec: p.deploy_delay_sec,
                }),
                ..Default::default()
            }
        }
    };

    let launch_mass_kg = cfg.body.dry_mass_with_fuel_section + cfg.engine.tank.tank_contents;
    let fuel_mass = cfg.engine.fuel.fuel_section_weight;
    let fuel_after_burn = cfg.engine.fuel.fuel_section_weight_after_burn;
    if fuel_after_burn > fuel_mass {
        bail!(
            "engine.fuel.mass_after_burn ({fuel_after_burn}) must be <= engine.fuel.mass ({fuel_mass})"
        );
    }

    let params = RocketParams {
        body_mass: BodyMassParams {
            diameter: cfg.body.diameter,
            total_mass: launch_mass_kg,
            cg: cfg.body.cg,
            inertia: cfg.body.inertia,
        },
        engine: EngineParams {
            thrust_table: thrust_table.into(),
            thruster_pos: cfg.engine.thruster_pos,
            tank: TankParams {
                position: cfg.engine.tank.position,
                drain_position: cfg.engine.tank.drain_position,
                contents: cfg.engine.tank.tank_contents,
            },
            fuel: FuelParams {
                position: cfg.engine.fuel.position,
                contents: fuel_mass,
                after_burn: fuel_after_burn,
            },
        },
        aero: AeroParams {
            cp_at_launch: cfg.aero.cp_at_launch,
            cp_mach_table: cp_mach_table.into(),
            cd0_alpha_mach_table,
            cn_table: cn_table.into(),
            cs_table: cs_table.into(),
            roll_damping_coefficient: cfg.aero.roll_damping,
            pitch_damping_coefficient: cfg.aero.pitch_damping,
            yaw_damping_coefficient: cfg.aero.yaw_damping,
        },
        launch_env: LaunchEnvParams {
            latitude: cfg.launch.latitude,
            longitude: cfg.launch.longitude,
            elevation: cfg.launch.elevation,
            launcher_height: cfg.launch.launcher_height,
            rail_length_m: cfg.launch.rail_length,
            pitch: cfg.launch.pitch,
            roll: cfg.launch.roll,
            yaw: cfg.launch.yaw,
            winds_table: winds_table.into(),
            initial_body_velocity_mps: [0.0; 3],
            initial_position_override: None,
        },
        sim: SimControl {
            flight_duration: cfg.sim.flight_duration,
            time_step: cfg.sim.time_step,
            output_decimation_rate: min(cfg.sim.csv_sample_interval, cfg.sim.kml_sample_interval) as usize,
            start_sim_time_sec: 0.0,
        },
        parachute,
    };
    params.validate()?;
    Ok(params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    fn write_file(dir: &std::path::Path, name: &str, content: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        p
    }

    fn make_fixtures(dir: &std::path::Path) {
        write_file(dir, "thrust.csv", "time,thrust\n0.0,2000.0\n1.0,2000.0\n");
        write_file(dir, "cp_mach.csv", "mach,cp\n0.0,1.2\n2.0,1.2\n");
        write_file(dir, "cn.csv", "0.0,2.0\n2.0,2.0\n");
        write_file(dir, "cs.csv", "0.0,2.0\n2.0,2.0\n");
        write_file(
            dir,
            "cd2d.csv",
            "alpha_deg,0.0,2.0\n0.0,0.4,0.4\n10.0,0.5,0.5\n",
        );
        write_file(dir, "vterm.csv", "t,v\n0.0,20.0\n60.0,20.0\n");
        write_file(
            dir,
            "wind_table.csv",
            "alt_m,speed_mps,dir_deg\n0.0,0.0,270.0\n500.0,5.0,270.0\n10000.0,10.0,270.0\n",
        );
    }

    fn base_cfg(dir: &std::path::Path, with_chute: bool) -> Config {
        use crate::config::*;
        Config {
            launch: LaunchConfig {
                latitude: 35.0,
                longitude: 139.0,
                elevation: 5.0,
                launcher_height: 5.0,
                rail_length: 5.0,
                pitch: 89.0,
                roll: 0.0,
                yaw: 0.0,
                wind_speed_mps: Some(3.0),
                wind_direction_deg: Some(270.0),
                wind_reference_alt: None,
                wind_power_exponent: 1.0 / 6.0,
                wind_table: None,
            },
            body: BodyConfig {
                diameter: 0.15,
                dry_mass_with_fuel_section: 28.0,
                cg: [1.0, 0.0, 0.0],
                inertia: [15.0, 15.0, 0.2, 0.0, 0.0, 0.0],
            },
            engine: EngineConfig {
                thrust_table: dir.join("thrust.csv"),
                thruster_pos: [2.0, 0.0, 0.0],
                tank: TankConfig {
                    position: [0.8, 0.0, 0.0],
                    drain_position: None,
                    tank_contents: 2.0,
                },
                fuel: FuelConfig {
                    position: [0.8, 0.0, 0.0],
                    fuel_section_weight: 1.5,
                    fuel_section_weight_after_burn: 0.1,
                },
            },
            aero: AeroConfig {
                cp_at_launch: [1.2, 0.0, 0.0],
                cp_mach_table: dir.join("cp_mach.csv"),
                cd0_alpha_mach_table: dir.join("cd2d.csv"),
                cn_table: dir.join("cn.csv"),
                cs_table: dir.join("cs.csv"),
                roll_damping: 0.0,
                pitch_damping: 0.0,
                yaw_damping: 0.0,
            },
            parachute: if with_chute {
                Some(ParachuteConfig {
                    terminal_velocity_table: dir.join("vterm.csv"),
                    deploy_delay_sec: 1.5,
                })
            } else {
                None
            },
            sim: SimConfig {
                flight_duration: 60.0,
                time_step: 0.01,
                csv_sample_interval: 1,
                kml_sample_interval: 10,
            },
        }
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("assemble_tests_{tag}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn power_law_wind_generates_variable_step_points() {
        let dir = tmpdir("wind_power_law");
        make_fixtures(&dir);
        let cfg = base_cfg(&dir, false);
        let params = assemble(&cfg).unwrap();
        let w = &params.launch_env.winds_table;
        // 0..=200 by 5 (41) + 210..=1000 by 10 (80) + 1100..=10000 by 100 (90)
        assert_eq!(w.len(), 211);
        assert!((w[0][0] - 0.0).abs() < 1e-9, "first alt = 0 m");
        assert!((w[w.len() - 1][0] - POWER_LAW_MAX_M as f64).abs() < 1e-9, "last alt = 10 000 m");
        assert!(w.iter().any(|row| (row[0] - 200.0).abs() < 1e-9), "contains 200 m");
        assert!(w.iter().any(|row| (row[0] - 210.0).abs() < 1e-9), "contains 210 m");
        assert!(w.iter().any(|row| (row[0] - 1000.0).abs() < 1e-9), "contains 1000 m");
        assert!(w.iter().any(|row| (row[0] - 1100.0).abs() < 1e-9), "contains 1100 m");
        // Surface wind is zero
        assert!((w[0][1] - 0.0).abs() < 1e-9, "surface speed = 0");
        // Direction is constant
        assert!((w[100][2] - 270.0).abs() < 1e-9);
    }

    #[test]
    fn power_law_speed_increases_with_altitude() {
        let dir = tmpdir("wind_power_law_mono");
        make_fixtures(&dir);
        let cfg = base_cfg(&dir, false);
        let params = assemble(&cfg).unwrap();
        let w = &params.launch_env.winds_table;
        // Speed at 100 m should be lower than at 1000 m
        let v100 = w
            .iter()
            .find(|row| (row[0] - 100.0).abs() < 1e-9)
            .expect("100 m point exists")[1];
        let v1000 = w
            .iter()
            .find(|row| (row[0] - 1000.0).abs() < 1e-9)
            .expect("1000 m point exists")[1];
        assert!(v1000 > v100, "power-law speed should increase with altitude");
    }

    #[test]
    fn wind_table_loaded_from_csv() {
        let dir = tmpdir("wind_csv");
        make_fixtures(&dir);
        let mut cfg = base_cfg(&dir, false);
        cfg.launch.wind_speed_mps = None;
        cfg.launch.wind_direction_deg = None;
        cfg.launch.wind_table = Some(dir.join("wind_table.csv"));
        let params = assemble(&cfg).unwrap();
        let w = &params.launch_env.winds_table;
        assert_eq!(w.len(), 3);
        assert!((w[1][0] - 500.0).abs() < 1e-9);
        assert!((w[1][1] - 5.0).abs() < 1e-9);
    }

    #[test]
    fn calm_wind_when_nothing_configured() {
        let dir = tmpdir("wind_calm");
        make_fixtures(&dir);
        let mut cfg = base_cfg(&dir, false);
        cfg.launch.wind_speed_mps = None;
        cfg.launch.wind_direction_deg = None;
        let params = assemble(&cfg).unwrap();
        let w = &params.launch_env.winds_table;
        assert_eq!(w.len(), 2);
        assert!((w[0][1] - 0.0).abs() < 1e-9);
        assert!((w[1][1] - 0.0).abs() < 1e-9);
    }

    #[test]
    fn prefers_wind_table_when_scalar_is_also_set() {
        let dir = tmpdir("wind_conflict");
        make_fixtures(&dir);
        let mut cfg = base_cfg(&dir, false);
        cfg.launch.wind_table = Some(dir.join("wind_table.csv"));
        // wind_speed_mps / wind_direction_deg are already set in base_cfg.
        // CSV should take precedence over the scalar power-law inputs.
        let params = assemble(&cfg).unwrap();
        let w = &params.launch_env.winds_table;
        assert_eq!(w.len(), 3);
        assert!((w[0][0] - 0.0).abs() < 1e-9);
        assert!((w[1][0] - 500.0).abs() < 1e-9);
        assert!((w[1][1] - 5.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_speed_without_direction() {
        let dir = tmpdir("wind_no_dir");
        make_fixtures(&dir);
        let mut cfg = base_cfg(&dir, false);
        cfg.launch.wind_direction_deg = None;
        let err = assemble(&cfg).unwrap_err();
        assert!(
            err.to_string().contains("wind_direction_deg"),
            "expected direction-missing error, got: {err}"
        );
    }

    #[test]
    fn missing_parachute_section_yields_disabled_default() {
        let dir = tmpdir("no_chute");
        make_fixtures(&dir);
        let cfg = base_cfg(&dir, false);
        let params = assemble(&cfg).unwrap();
        assert!(params.parachute.terminal_velocity_table.is_empty());
        assert!(params.parachute.deploy_trigger.is_none());
    }

    #[test]
    fn parachute_section_wires_apogee_trigger_with_delay() {
        let dir = tmpdir("chute");
        make_fixtures(&dir);
        let cfg = base_cfg(&dir, true);
        let params = assemble(&cfg).unwrap();
        assert_eq!(params.parachute.terminal_velocity_table.len(), 2);
        let trig = params
            .parachute
            .deploy_trigger
            .as_ref()
            .expect("trigger wired");
        assert_eq!(trig.origin, EventKind::Apogee);
        assert!((trig.delay_sec - 1.5).abs() < 1e-9);
    }

    #[test]
    fn assemble_rejects_missing_csv_file() {
        let dir = tmpdir("missing");
        make_fixtures(&dir);
        let mut cfg = base_cfg(&dir, false);
        cfg.engine.thrust_table = dir.join("does_not_exist.csv");
        let err = assemble(&cfg).unwrap_err();
        assert!(
            err.to_string().contains("does_not_exist.csv") || err.to_string().contains("reading"),
            "expected missing-file error, got: {err}"
        );
    }

    #[test]
    fn derives_launch_mass_from_dry_with_fuel_section_plus_tank() {
        let dir = tmpdir("derived_launch_mass");
        make_fixtures(&dir);
        let mut cfg = base_cfg(&dir, false);
        cfg.body.dry_mass_with_fuel_section = 17.0;
        cfg.engine.tank.tank_contents = 3.0;
        let params = assemble(&cfg).unwrap();
        assert!((params.body_mass.total_mass - 20.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_after_burn_greater_than_fuel_mass() {
        let dir = tmpdir("fuel_after_burn_invalid");
        make_fixtures(&dir);
        let mut cfg = base_cfg(&dir, false);
        cfg.engine.fuel.fuel_section_weight = 0.5;
        cfg.engine.fuel.fuel_section_weight_after_burn = 0.6;
        let err = assemble(&cfg).unwrap_err();
        assert!(
            err.to_string().contains("mass_after_burn") || err.to_string().contains("<="),
            "expected after-burn validation error, got: {err}"
        );
    }
}
