//! Landing-area sweep: power-law wind × N speeds × M directions, rayon-parallel.
//!
//! Each condition runs simulate → (optional) DEM refine → write files, all
//! within one rayon closure. The full `UnifiedSimulationOutput` is dropped as
//! soon as files are written, keeping peak memory bounded by (thread count × 1
//! simulation output) — the same as the original single-phase implementation.
//!
//! Output layout:
//!   <out_dir>/spd{speed:.1}_dir{dir:03.0}/
//!     mainline.csv  parachute.csv  events.json  summary.json  trajectory.kml

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use rayon::prelude::*;

use simulator_core::RocketParams;

use crate::assemble::power_law_winds_table;
use crate::config::Config;
use crate::dem::DemCache;
use crate::refine_landing;
use crate::runner;

// ---------------------------------------------------------------------------
// Public configuration struct
// ---------------------------------------------------------------------------

pub struct LandingAreaArgs {
    pub out_dir: PathBuf,
    /// Number of equally-spaced compass directions (e.g. 8 → N/NE/E/…).
    pub directions: u32,
    /// Maximum wind speed [m/s].
    pub speed_max: f64,
    /// Number of speed steps including 0 (e.g. 9 → 0,1,…,8 m/s).
    pub speed_steps: u32,
    /// Override rayon thread count (None = all available cores).
    pub jobs: Option<usize>,
    pub csv_interval: usize,
    pub kml_interval: usize,
    /// Skip GSI DEM landing-point refinement.
    pub no_dem: bool,
}

// ---------------------------------------------------------------------------
// Condition grid
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Condition {
    speed_mps: f64,
    dir_deg: f64,
}

impl Condition {
    /// Subdirectory name, e.g. `spd3.0_dir045`.
    fn dir_name(self) -> String {
        format!("spd{:.1}_dir{:03.0}", self.speed_mps, self.dir_deg)
    }
}

fn make_conditions(directions: u32, speed_max: f64, speed_steps: u32) -> Vec<Condition> {
    let dirs: Vec<f64> = (0..directions)
        .map(|i| 360.0 * i as f64 / directions as f64)
        .collect();

    let speeds: Vec<f64> = if speed_steps <= 1 {
        vec![0.0]
    } else {
        let step = speed_max / (speed_steps - 1) as f64;
        (0..speed_steps)
            .map(|i| (i as f64 * step).min(speed_max))
            .collect()
    };

    dirs.iter()
        .flat_map(|&dir| speeds.iter().map(move |&spd| Condition { speed_mps: spd, dir_deg: dir }))
        .collect()
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(base_cfg: &Config, base_params: &RocketParams, args: &LandingAreaArgs) -> Result<()> {
    if let Some(n) = args.jobs {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global();
    }

    let conditions = make_conditions(args.directions, args.speed_max, args.speed_steps);
    let total = conditions.len();
    eprintln!(
        "landing-area: {} conditions ({} directions × {} speeds)",
        total, args.directions, args.speed_steps
    );

    let h_ref = base_cfg
        .launch
        .wind_reference_alt
        .unwrap_or(base_cfg.launch.elevation)
        .max(10.0);
    let alpha = base_cfg.launch.wind_power_exponent;
    let h_launch = base_params.launch_env.elevation;

    // Initialise DEM cache once; shared across all rayon workers via reference.
    // DemCache is Send + Sync (Mutex interior), so this is safe.
    let dem: Option<DemCache> = if args.no_dem {
        None
    } else {
        match DemCache::new() {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("warn: DEM cache init failed, skipping refinement: {e:#}");
                None
            }
        }
    };
    let dem_ref = dem.as_ref();

    let completed = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    conditions.par_iter().for_each(|cond| {
        let out_dir = args.out_dir.join(cond.dir_name());

        let mut params = base_params.clone();
        params.launch_env.winds_table =
            power_law_winds_table(cond.speed_mps, cond.dir_deg, h_ref, alpha).into();

        // Simulate → (optional) refine → write → drop output.
        // All within this closure: peak memory = thread_count × one output.
        let result = (|| -> Result<()> {
            let mut output = runner::simulate(&params)?;

            if let Some(dem) = dem_ref {
                if let Err(e) = refine_landing::refine_one(&mut output, h_launch, dem) {
                    eprintln!("warn: DEM refine {}: {e:#}", cond.dir_name());
                }
            }

            runner::write_outputs(
                &output,
                &out_dir,
                args.csv_interval,
                args.kml_interval,
                base_params,
            )?;
            // `output` is dropped here.
            Ok(())
        })();

        match result {
            Ok(_) => {
                let n = completed.fetch_add(1, Ordering::Relaxed) + 1;
                eprintln!("[{n:>3}/{total}] OK  {}", cond.dir_name());
            }
            Err(e) => {
                failed.fetch_add(1, Ordering::Relaxed);
                eprintln!("FAIL {}: {e:#}", cond.dir_name());
            }
        }
    });

    let n_ok = completed.load(Ordering::Relaxed);
    let n_fail = failed.load(Ordering::Relaxed);
    eprintln!("landing-area done: {n_ok} OK, {n_fail} failed");

    if n_fail > 0 {
        anyhow::bail!("{n_fail} simulation(s) failed");
    }
    Ok(())
}
