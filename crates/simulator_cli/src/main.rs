use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod assemble;
mod config;
mod csv_loader;
mod runner;

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
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Run { config, out_dir } => {
            let cfg = config::Config::load(&config)?;
            let params = assemble::assemble(&cfg)?;
            let paths = runner::run(&params, &out_dir)?;
            eprintln!("wrote {}", paths.summary.display());
            eprintln!("       {}", paths.mainline.display());
            eprintln!("       {}", paths.parachute.display());
            eprintln!("       {}", paths.events.display());
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
    }

    Ok(())
}
