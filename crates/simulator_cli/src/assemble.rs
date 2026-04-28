//! Convert user-facing `Config` + CSV tables into a `RocketParams`.

use anyhow::{Result, bail};

use simulator_core::EventKind;
use simulator_core::params::{
    AeroParams, BodyMassParams, EngineParams, FuelParams, LaunchEnvParams, ParachuteParams,
    RocketParams, SimControl, TankParams,
};
use simulator_core::progress::DelayedBranchTrigger;

use crate::config::Config;
use crate::csv_loader;

/// Sentinel upper altitude for the 2-point constant-wind table. JSBSim's
/// wind lookup clamps at the table endpoints so any value comfortably
/// above expected flight apogee works.
const WINDS_TABLE_TOP_M: f64 = 30_000.0;

pub fn assemble(cfg: &Config) -> Result<RocketParams> {
    let thrust_table = csv_loader::load_1d(&cfg.engine.thrust_table)?;
    let cp_mach_table = csv_loader::load_1d(&cfg.aero.cp_mach_table)?;
    let cn_table = csv_loader::load_1d(&cfg.aero.cn_table)?;
    let cs_table = csv_loader::load_1d(&cfg.aero.cs_table)?;
    let cd0_alpha_mach_table = csv_loader::load_cd_table_deg(&cfg.aero.cd0_alpha_mach_table)?;

    // Single scalar wind expanded to a 2-point winds-aloft table.
    let winds_table = vec![
        [0.0, cfg.launch.wind_speed_mps, cfg.launch.wind_direction_deg],
        [
            WINDS_TABLE_TOP_M,
            cfg.launch.wind_speed_mps,
            cfg.launch.wind_direction_deg,
        ],
    ];

    let parachute = match &cfg.parachute {
        None => ParachuteParams::default(),
        Some(p) => {
            let terminal_velocity_table = csv_loader::load_1d(&p.terminal_velocity_table)?;
            ParachuteParams {
                terminal_velocity_table,
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
            thrust_table,
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
            cp_mach_table,
            cd0_alpha_mach_table,
            cn_table,
            cs_table,
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
            terrain: None,
            pitch: cfg.launch.pitch,
            roll: cfg.launch.roll,
            yaw: cfg.launch.yaw,
            winds_table,
            initial_body_velocity_mps: [0.0; 3],
            initial_position_override: None,
        },
        sim: SimControl {
            flight_duration: cfg.sim.flight_duration,
            time_step: cfg.sim.time_step,
            apogee_mode: cfg.sim.apogee_mode,
            state_sample_interval: cfg.sim.state_sample_interval,
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
        write_file(
            dir,
            "vterm.csv",
            "t,v\n0.0,20.0\n60.0,20.0\n",
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
                wind_speed_mps: 3.0,
                wind_direction_deg: 270.0,
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
                apogee_mode: 0,
                state_sample_interval: 1,
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
    fn scalar_wind_expands_to_two_point_table() {
        let dir = tmpdir("wind");
        make_fixtures(&dir);
        let cfg = base_cfg(&dir, false);
        let params = assemble(&cfg).unwrap();
        let w = &params.launch_env.winds_table;
        assert_eq!(w.len(), 2);
        assert!((w[0][0] - 0.0).abs() < 1e-9);
        assert!((w[1][0] - WINDS_TABLE_TOP_M).abs() < 1e-9);
        assert!((w[0][1] - 3.0).abs() < 1e-9);
        assert!((w[1][1] - 3.0).abs() < 1e-9);
        assert!((w[0][2] - 270.0).abs() < 1e-9);
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
