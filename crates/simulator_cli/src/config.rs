//! User-facing YAML config schema.
//!
//! Directly deserialised from `config.yaml`. Table paths are stored as
//! `PathBuf` and resolved against the config file's parent directory
//! by [`Config::load`].

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

fn default_rail_length() -> f64 {
    5.0
}

fn default_roll() -> f64 {
    0.0
}

fn default_wind_power_exponent() -> f64 { 0.166666666 }

fn default_csv_sample_interval() -> u32 {
    1
}

fn default_kml_sample_interval() -> u32 {
    10
}

fn default_deploy_delay() -> f64 {
    1.0
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub launch: LaunchConfig,
    pub body: BodyConfig,
    pub engine: EngineConfig,
    pub aero: AeroConfig,
    #[serde(default)]
    pub parachute: Option<ParachuteConfig>,
    pub sim: SimConfig,
}

#[derive(Debug, Deserialize)]
pub struct LaunchConfig {
    pub latitude: f64,
    pub longitude: f64,
    pub elevation: f64,
    pub launcher_height: f64,
    #[serde(default = "default_rail_length")]
    pub rail_length: f64,
    pub pitch: f64,
    #[serde(default = "default_roll")]
    pub roll: f64,
    pub yaw: f64,
    #[serde(default)]
    pub wind_speed_mps: Option<f64>,
    #[serde(default)]
    pub wind_reference_alt: Option<f64>,
    #[serde(default="default_wind_power_exponent")]
    pub wind_power_exponent: f64,
    #[serde(default)]
    pub wind_direction_deg: Option<f64>,
    #[serde(default)]
    pub wind_table: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct BodyConfig {
    pub diameter: f64,
    /// Airframe dry mass with fuel section installed, before tank fill [kg].
    pub dry_mass_with_fuel_section: f64,
    pub cg: [f64; 3],
    /// `[Ixx, Iyy, Izz, Ixy, Ixz, Iyz]`.
    pub inertia: [f64; 6],
}

#[derive(Debug, Deserialize)]
pub struct EngineConfig {
    pub thrust_table: PathBuf,
    pub thruster_pos: [f64; 3],
    pub tank: TankConfig,
    pub fuel: FuelConfig,
}

#[derive(Debug, Deserialize)]
pub struct TankConfig {
    pub position: [f64; 3],
    #[serde(default)]
    pub drain_position: Option<[f64; 3]>,
    /// Tank contents mass [kg].
    pub tank_contents: f64,
}

#[derive(Debug, Deserialize)]
pub struct FuelConfig {
    pub position: [f64; 3],
    /// Fuel section mass before burn [kg].
    pub fuel_section_weight: f64,
    /// Fuel section mass after burn [kg].
    pub fuel_section_weight_after_burn: f64,
}

#[derive(Debug, Deserialize)]
pub struct AeroConfig {
    pub cp_at_launch: [f64; 3],
    pub cp_mach_table: PathBuf,
    pub cd0_alpha_mach_table: PathBuf,
    pub cn_table: PathBuf,
    pub cs_table: PathBuf,
    pub roll_damping: f64,
    pub pitch_damping: f64,
    pub yaw_damping: f64,
}

#[derive(Debug, Deserialize)]
pub struct ParachuteConfig {
    pub terminal_velocity_table: PathBuf,
    #[serde(default = "default_deploy_delay")]
    pub deploy_delay_sec: f64,
}

#[derive(Debug, Deserialize)]
pub struct SimConfig {
    pub flight_duration: f64,
    pub time_step: f64,
    /// CSV writer decimation. Default `1` (every step).
    #[serde(default = "default_csv_sample_interval")]
    pub csv_sample_interval: u32,
    /// KML writer decimation. Default `10` (one point per ten steps).
    #[serde(default = "default_kml_sample_interval")]
    pub kml_sample_interval: u32,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("reading config {}", path.display()))?;
        let mut cfg: Config = serde_yaml::from_str(&raw)
            .with_context(|| format!("parsing YAML config {}", path.display()))?;
        let base = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));


        cfg.resolve_paths(&base);
        Ok(cfg)
    }

    fn resolve_paths(&mut self, base: &Path) {
        let fix = |p: &mut PathBuf| {
            if !p.is_absolute() {
                *p = base.join(&*p);
            }
        };
        fix(&mut self.engine.thrust_table);
        fix(&mut self.aero.cp_mach_table);
        fix(&mut self.aero.cd0_alpha_mach_table);
        fix(&mut self.aero.cn_table);
        fix(&mut self.aero.cs_table);
        if let Some(p) = self.parachute.as_mut() {
            fix(&mut p.terminal_velocity_table);
        }
        if let Some(w) = self.launch.wind_table.as_mut() {
            fix(w);
        }
    }
}
