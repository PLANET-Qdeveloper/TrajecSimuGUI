//! Convert legacy Python-TrajecSimu YAML config to the new Rust simulator format.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::config::{
    AeroConfig, BodyConfig, Config, EngineConfig, FuelConfig, LaunchConfig, ParachuteConfig,
    SimConfig, TankConfig,
};

// ─── Legacy format structs ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OldConfig {
    launch: OldLaunch,
    rocket: OldRocket,
    simulation: OldSimulation,
}

#[derive(Debug, Deserialize)]
struct OldLaunch {
    latitude: f64,
    longitude: f64,
    elevation: f64,
    #[serde(default)]
    launcher_length: Option<f64>,
    #[serde(default)]
    pitch: Vec<f64>,
    #[serde(default)]
    yaw: Vec<f64>,
    #[serde(default)]
    roll: f64,
    #[serde(default)]
    ground_wind_speed: Vec<f64>,
    #[serde(default)]
    ground_wind_dir: Vec<f64>,
    #[serde(default)]
    wind_ref_altitude: Option<f64>,
    #[serde(default = "default_wind_power")]
    wind_power_factor: f64,
    #[serde(default)]
    winds_table: Option<PathBuf>,
}

fn default_wind_power() -> f64 {
    0.16666
}

#[derive(Debug, Deserialize)]
struct OldRocket {
    diameter: f64,
    dry_weight: f64,
    cg_x: f64,
    #[serde(default)]
    cg_y: f64,
    #[serde(default)]
    cg_z: f64,
    cp_x: f64,
    #[serde(default)]
    cp_y: f64,
    #[serde(default)]
    cp_z: f64,
    inertia_xx: f64,
    inertia_yy: f64,
    inertia_zz: f64,
    #[serde(default)]
    inertia_xy: f64,
    #[serde(default)]
    inertia_xz: f64,
    #[serde(default)]
    inertia_yz: f64,
    tank_x: f64,
    #[serde(default)]
    tank_y: f64,
    #[serde(default)]
    tank_z: f64,
    #[serde(default)]
    tank_drain_x: Option<f64>,
    #[serde(default)]
    tank_drain_y: f64,
    #[serde(default)]
    tank_drain_z: f64,
    tank_capacity: f64,
    fuel_x: f64,
    #[serde(default)]
    fuel_y: f64,
    #[serde(default)]
    fuel_z: f64,
    fuel_capacity: f64,
    fuel_after_burn: f64,
    thruster_x: f64,
    #[serde(default)]
    thruster_y: f64,
    #[serde(default)]
    thruster_z: f64,
    roll_damping_coefficient: f64,
    pitch_damping_coefficient: f64,
    #[serde(default)]
    yaw_damping_coefficient: Option<f64>,
    // Flat list of [alt_m, vel_mps] pairs; null/missing → no parachute.
    #[serde(default)]
    terminal_velocity: Option<Vec<f64>>,
    thrust_table: PathBuf,
    cd0_table: PathBuf,
    cdmach_table: PathBuf,
    cnmach_table: PathBuf,
    csmach_table: PathBuf,
    cp_mach_table: PathBuf,
}

#[derive(Debug, Deserialize)]
struct OldSimulation {
    flight_duration: f64,
    time_step: f64,
    #[serde(default)]
    parachute_deploy_delay: Option<f64>,
    #[serde(default = "default_output_rate")]
    output_rate: u32,
}

fn default_output_rate() -> u32 {
    1
}

// ─── Public entry point ───────────────────────────────────────────────────

/// Convert a legacy Python-TrajecSimu config YAML to the new Rust format.
///
/// Writes `config.yaml` and a `tables/` subdirectory under `output_dir`.
/// Returns the absolute path to the generated `config.yaml`.
pub fn convert(input_path: &Path, output_dir: &Path) -> Result<PathBuf> {
    let raw = fs::read_to_string(input_path)
        .with_context(|| format!("reading {}", input_path.display()))?;
    let old: OldConfig = serde_yaml::from_str(&raw).with_context(|| "parsing legacy YAML")?;

    let base = input_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    fs::create_dir_all(output_dir).with_context(|| format!("creating {}", output_dir.display()))?;
    let tables_dir = output_dir.join("tables");
    fs::create_dir_all(&tables_dir)?;

    // ── Copy / generate tables ──────────────────────────────────────────
    copy_table(&base, &old.rocket.thrust_table, &tables_dir, "thrust.csv")?;
    copy_table(&base, &old.rocket.cp_mach_table, &tables_dir, "cp_mach.csv")?;
    copy_table(&base, &old.rocket.cnmach_table, &tables_dir, "cn.csv")?;
    copy_table(&base, &old.rocket.csmach_table, &tables_dir, "cs.csv")?;
    build_cd0_alpha_mach(
        &base,
        &old.rocket.cd0_table,
        &old.rocket.cdmach_table,
        &tables_dir,
    )?;

    // ── Wind ────────────────────────────────────────────────────────────
    let (wind_speed, wind_dir, wind_table_rel) = resolve_wind(&old.launch, &base, &tables_dir)?;

    // ── Parachute ────────────────────────────────────────────────────────
    let parachute = build_parachute_config(&old.rocket, &old.simulation, &tables_dir)?;

    // ── Assemble new Config ──────────────────────────────────────────────
    let r = &old.rocket;
    let sim = &old.simulation;
    let cfg = Config {
        launch: LaunchConfig {
            latitude: old.launch.latitude,
            longitude: old.launch.longitude,
            elevation: old.launch.elevation,
            rail_length: old.launch.launcher_length.unwrap_or(5.0),
            pitch: old.launch.pitch.first().copied().unwrap_or(80.0),
            roll: old.launch.roll,
            yaw: old.launch.yaw.first().copied().unwrap_or(0.0),
            wind_speed_mps: wind_speed,
            wind_direction_deg: wind_dir,
            wind_reference_alt: old.launch.wind_ref_altitude,
            wind_power_exponent: old.launch.wind_power_factor,
            wind_table: wind_table_rel,
        },
        body: BodyConfig {
            diameter: r.diameter,
            dry_mass_with_fuel_section: r.dry_weight,
            cg: [r.cg_x, r.cg_y, r.cg_z],
            inertia: [
                r.inertia_xx,
                r.inertia_yy,
                r.inertia_zz,
                r.inertia_xy,
                r.inertia_xz,
                r.inertia_yz,
            ],
        },
        engine: EngineConfig {
            thrust_table: PathBuf::from("tables/thrust.csv"),
            thruster_pos: [r.thruster_x, r.thruster_y, r.thruster_z],
            tank: TankConfig {
                position: [r.tank_x, r.tank_y, r.tank_z],
                drain_position: r
                    .tank_drain_x
                    .map(|dx| [dx, r.tank_drain_y, r.tank_drain_z]),
                tank_contents: r.tank_capacity,
            },
            fuel: FuelConfig {
                position: [r.fuel_x, r.fuel_y, r.fuel_z],
                fuel_section_weight: r.fuel_capacity,
                fuel_section_weight_after_burn: r.fuel_after_burn,
            },
        },
        aero: AeroConfig {
            cp_at_launch: [r.cp_x, r.cp_y, r.cp_z],
            cp_mach_table: PathBuf::from("tables/cp_mach.csv"),
            cd0_alpha_mach_table: PathBuf::from("tables/cd0_alpha_mach.csv"),
            cn_table: PathBuf::from("tables/cn.csv"),
            cs_table: PathBuf::from("tables/cs.csv"),
            roll_damping: r.roll_damping_coefficient,
            pitch_damping: r.pitch_damping_coefficient,
            yaw_damping: r
                .yaw_damping_coefficient
                .unwrap_or(r.pitch_damping_coefficient),
        },
        parachute,
        sim: SimConfig {
            flight_duration: sim.flight_duration,
            time_step: sim.time_step,
            csv_sample_interval: sim.output_rate,
            kml_sample_interval: sim.output_rate * 10,
        },
    };

    let yaml = serde_yaml::to_string(&cfg)?;
    let out_path = output_dir.join("config.yaml");
    fs::write(&out_path, &yaml).with_context(|| format!("writing {}", out_path.display()))?;

    let abs = out_path.canonicalize().unwrap_or_else(|_| out_path.clone());
    Ok(abs)
}

// ─── Private helpers ──────────────────────────────────────────────────────

/// Find a file by searching upward from `base` until found (up to 6 levels).
/// This handles old configs where paths are relative to the project root,
/// not to the config file's directory.
fn find_file(base: &Path, rel: &Path) -> Result<PathBuf> {
    if rel.is_absolute() {
        if rel.exists() {
            return Ok(rel.to_path_buf());
        }
        return Err(anyhow::anyhow!("file not found: {}", rel.display()));
    }
    let mut dir: Option<&Path> = Some(base);
    while let Some(d) = dir {
        let candidate = d.join(rel);
        if candidate.exists() {
            return Ok(candidate);
        }
        dir = d.parent();
    }
    Err(anyhow::anyhow!(
        "file not found: {} (searched upward from {})",
        rel.display(),
        base.display()
    ))
}

fn copy_table(base: &Path, src: &Path, dst_dir: &Path, name: &str) -> Result<()> {
    let src_abs = find_file(base, src)?;
    let dst = dst_dir.join(name);
    fs::copy(&src_abs, &dst)
        .with_context(|| format!("copying {} -> {}", src_abs.display(), dst.display()))?;
    Ok(())
}

fn resolve_wind(
    launch: &OldLaunch,
    base: &Path,
    tables_dir: &Path,
) -> Result<(Option<f64>, Option<f64>, Option<PathBuf>)> {
    if let Some(wt) = &launch.winds_table {
        copy_table(base, wt, tables_dir, "wind_table.csv")?;
        return Ok((None, None, Some(PathBuf::from("tables/wind_table.csv"))));
    }
    match (
        launch.ground_wind_speed.first().copied(),
        launch.ground_wind_dir.first().copied(),
    ) {
        (Some(s), Some(d)) => Ok((Some(s), Some(d), None)),
        _ => Ok((None, None, None)),
    }
}

fn build_parachute_config(
    rocket: &OldRocket,
    sim: &OldSimulation,
    tables_dir: &Path,
) -> Result<Option<ParachuteConfig>> {
    let tv = match &rocket.terminal_velocity {
        Some(v) if !v.is_empty() => v,
        _ => return Ok(None),
    };
    // Flat list of [alt_m, vel_mps] pairs; take velocity of first pair.
    let vel = if tv.len() >= 2 { tv[1] } else { tv[0] };
    let tv_path = tables_dir.join("terminal_velocity.csv");
    {
        let mut f = fs::File::create(&tv_path)?;
        writeln!(f, "t_since_deploy_sec,v_terminal_mps")?;
        writeln!(f, "0.0,{vel}")?;
        writeln!(f, "9999.0,{vel}")?;
    }
    Ok(Some(ParachuteConfig {
        terminal_velocity_table: PathBuf::from("tables/terminal_velocity.csv"),
        deploy_delay_sec: sim.parachute_deploy_delay.unwrap_or(1.0),
    }))
}

fn build_cd0_alpha_mach(
    base: &Path,
    cd0_path: &Path,
    cdmach_path: &Path,
    dst_dir: &Path,
) -> Result<()> {
    let cd0_rows = read_2col_csv(base, cd0_path)?; // (alpha_rad, cd0_base)
    let mach_rows = read_2col_csv(base, cdmach_path)?; // (mach, delta_cd0)

    let dst = dst_dir.join("cd0_alpha_mach.csv");
    let mut f = fs::File::create(&dst)?;

    let mach_header: Vec<String> = mach_rows.iter().map(|(m, _)| format!("{m}")).collect();
    writeln!(f, "alpha_deg\\mach,{}", mach_header.join(","))?;

    for (alpha_rad, cd0_base) in &cd0_rows {
        let alpha_deg = alpha_rad.to_degrees();
        let cds: Vec<String> = mach_rows
            .iter()
            .map(|(_, delta)| format!("{:.6}", cd0_base + delta))
            .collect();
        writeln!(f, "{alpha_deg:.4},{}", cds.join(","))?;
    }
    Ok(())
}

fn read_2col_csv(base: &Path, rel_path: &Path) -> Result<Vec<(f64, f64)>> {
    let abs = find_file(base, rel_path)?;
    let content = fs::read_to_string(&abs).with_context(|| format!("reading {}", abs.display()))?;
    let mut pairs = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let delim = if line.contains(',') { ',' } else { '\t' };
        let mut parts = line.splitn(2, delim);
        let a = parts.next().and_then(|s| s.trim().parse::<f64>().ok());
        let b = parts.next().and_then(|s| s.trim().parse::<f64>().ok());
        if let (Some(a), Some(b)) = (a, b) {
            pairs.push((a, b));
        }
    }
    if pairs.is_empty() {
        return Err(anyhow::anyhow!("no numeric data in {}", abs.display()));
    }
    Ok(pairs)
}
