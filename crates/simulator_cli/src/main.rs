use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod assemble;
mod config;
mod csv_loader;
mod dem;
mod kml_writer;
mod landing_area;
mod refine_landing;
mod runner;
mod summary_writer;
mod chart;

#[derive(Parser, Debug)]
#[command(name = "simulator_cli", about = "TrajecSimuGUI core driver")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Run a full simulation and write mainline/parachute/events/summary.
    Run {
        #[arg(short, long)]
        config: PathBuf,
        #[arg(long, default_value = "out")]
        out_dir: PathBuf,
        /// Disable GSI DEM landing-point refinement.
        #[arg(long)]
        no_dem: bool,
    },
    /// Parse + assemble + validate only. No simulation step.
    Validate {
        #[arg(short, long)]
        config: PathBuf,
    },
    /// Print assembled RocketParams as pretty JSON.
    Inspect {
        #[arg(short, long)]
        config: PathBuf,
    },
    /// Sweep wind speed × direction with power-law profile (rayon-parallel).
    ///
    /// Outputs one subdirectory per condition under <out-dir>:
    ///   <out-dir>/spd{speed:.1}_dir{dir:03.0}/
    LandingArea {
        #[arg(short, long)]
        config: PathBuf,
        /// Root output directory.
        #[arg(long, default_value = "landing_area")]
        out_dir: PathBuf,
        /// Number of equally-spaced compass directions.
        #[arg(long, default_value = "8")]
        directions: u32,
        /// Maximum wind speed [m/s].
        #[arg(long, default_value = "8.0")]
        speed_max: f64,
        /// Number of speed steps (0 to speed-max inclusive).
        #[arg(long, default_value = "9")]
        speed_steps: u32,
        /// Maximum parallel jobs (default: all available cores).
        #[arg(long)]
        jobs: Option<usize>,
        /// Disable GSI DEM landing-point refinement.
        #[arg(long)]
        no_dem: bool,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    std::env::set_var("JSBSIM_DEBUG", "0");
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Run { config, out_dir, no_dem } => {
            let cfg = config::Config::load(&config)?;
            let params = assemble::assemble(&cfg)?;

            let mut output = runner::simulate(&params)?;

            if !no_dem {
                match dem::DemCache::new() {
                    Ok(cache) => {
                        if let Err(e) = refine_landing::refine_one(
                            &mut output,
                            &cache,
                        ) {
                            eprintln!("warn: DEM refinement failed, using original landing: {e:#}");
                        }
                    }
                    Err(e) => eprintln!("warn: DEM cache init failed: {e:#}"),
                }
            }

            let paths = runner::write_outputs(
                &output,
                &out_dir,
                cfg.sim.csv_sample_interval as usize,
                cfg.sim.kml_sample_interval as usize,
                &params,
            )?;
            eprintln!("wrote {}", paths.summary.display());
            eprintln!("       {}", paths.mainline.display());
            eprintln!("       {}", paths.parachute.display());
            eprintln!("       {}", paths.events.display());
            eprintln!("       {}", paths.kml.display());
        }
        Cmd::Validate { config } => {
            let cfg = config::Config::load(&config)?;
            let _params = assemble::assemble(&cfg)?;
            eprintln!("config OK: {}", config.display());
        }
        Cmd::Inspect { config } => {
            let cfg = config::Config::load(&config)?;
            let params = assemble::assemble(&cfg)?;
            println!("{}", serde_json::to_string_pretty(&params)?);
        }
        Cmd::LandingArea {
            config,
            out_dir,
            directions,
            speed_max,
            speed_steps,
            jobs,
            no_dem,
        } => {
            let cfg = config::Config::load(&config)?;
            let params = assemble::assemble(&cfg)?;
            let args = landing_area::LandingAreaArgs {
                out_dir,
                directions,
                speed_max,
                speed_steps,
                jobs,
                csv_interval: cfg.sim.csv_sample_interval as usize,
                kml_interval: cfg.sim.kml_sample_interval as usize,
                no_dem,
            };
            landing_area::run(&cfg, &params, &args)?;
        }
    }

    Ok(())
}
