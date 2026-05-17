//! Pure physics simulation — no file I/O.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use simulator_core::analysis;
use simulator_core::{Phase, RocketParams, SimulationOrchestrator, UnifiedSimulationOutput};

use crate::pipeline::{self, PostProcessor, RunPaths};
use crate::refine_landing;

/// Run the simulation and post-analysis pass. No file I/O.
pub fn simulate(params: &RocketParams) -> Result<UnifiedSimulationOutput> {
    let mut orch = SimulationOrchestrator::new();
    orch.initialize(params)?;

    // Safety cap: 2× the nominal step count plus a generous additive buffer
    // for the parachute descent branch.
    let nominal = (params.sim.flight_duration / params.sim.time_step).max(1.0);
    let max_steps = (nominal * 2.0) as usize + 1024;

    for _ in 0..max_steps {
        if !orch.step()? {
            break;
        }
    }

    if orch.phase() != Phase::Completed {
        bail!(
            "simulation did not complete within {} steps (phase = {:?})",
            max_steps,
            orch.phase()
        );
    }

    let mut output = orch.into_output();
    analysis::analyze(&mut output, params);
    Ok(output)
}

pub struct RunArgs {
    pub out_dir: PathBuf,
    pub no_dem: bool,
    pub no_chart: bool,
    pub csv_interval: usize,
    pub kml_interval: usize,
}

/// Full single-run pipeline: simulate → (optional) DEM refine → write outputs.
pub fn run_single(params: &RocketParams, args: &RunArgs) -> Result<RunPaths> {
    let mut output = simulate(params)?;

    let dem = if args.no_dem {
        None
    } else {
        crate::dem::DemCache::new().ok()
    };
    refine_landing::try_refine(&mut output, dem.as_ref());

    std::fs::create_dir_all(&args.out_dir)
        .with_context(|| format!("creating output dir {}", args.out_dir.display()))?;

    let (csv_int, kml_int) =
        pipeline::normalise_intervals(args.csv_interval, args.kml_interval);
    let ctx = pipeline::RunContext {
        output: &output,
        out_dir: &args.out_dir,
        params,
        csv_interval: csv_int,
        kml_interval: kml_int,
    };

    let optional: Vec<Box<dyn PostProcessor>> = if args.no_chart {
        vec![]
    } else {
        vec![Box::new(pipeline::DrawChartsStep)]
    };
    pipeline::run_pipeline(&ctx, &pipeline::default_mandatory_steps(), &optional)?;

    Ok(RunPaths::from_context(&ctx))
}
