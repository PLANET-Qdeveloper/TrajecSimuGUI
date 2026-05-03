//! Drive the `SimulationOrchestrator` end-to-end and write outputs.

use std::cmp::min;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;

use simulator_core::analysis;
use simulator_core::{
    EventKind, Phase, RocketParams, SimulationOrchestrator, SimulationState,
    UnifiedSimulationOutput,
};

#[derive(Debug, Clone)]
pub struct RunPaths {
    pub mainline: PathBuf,
    pub parachute: PathBuf,
    pub events: PathBuf,
    pub summary: PathBuf,
    pub kml: PathBuf,
}

pub fn run(
    params: &RocketParams,
    out_dir: &Path,
    csv_interval: usize,
    kml_interval: usize,
) -> Result<RunPaths> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("creating output dir {}", out_dir.display()))?;

    let mut orch = SimulationOrchestrator::new();
    orch.initialize(params)?;

    // Safety cap: 2× the nominal step count plus a generous additive
    // buffer for the parachute descent branch, which shares the same
    // time_step but may run for much longer than `flight_duration` in
    // contrived cases.
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

    // Run post-simulation analysis: per-step diagnostics + derived events
    // (MaxQ, MaxAxialAcceleration, MaxLateralAcceleration, MaxAngularRate).
    let mut output = orch.into_output();
    analysis::analyze(&mut output, params);
    let out = &output;

    let paths = RunPaths {
        mainline: out_dir.join("mainline.csv"),
        parachute: out_dir.join("parachute.csv"),
        events: out_dir.join("events.json"),
        summary: out_dir.join("summary.json"),
        kml: out_dir.join("trajectory.kml"),
    };
    
    let csv_interval = csv_interval / min(csv_interval, kml_interval);
    let kml_interval = kml_interval / min(csv_interval, kml_interval);
    

    write_trajectory_csv(&paths.mainline, &out.mainline.trajectory, csv_interval)?;
    write_trajectory_csv(
        &paths.parachute,
        &out.parachute_branch.trajectory,
        csv_interval,
    )?;
    write_events_json(&paths.events, out)?;
    write_summary_json(&paths.summary, out)?;
    crate::kml_writer::write_trajectory_kml(&paths.kml, out, params, kml_interval)?;

    Ok(paths)
}

/// `i` is the trajectory step index, `len` the trajectory length. Keep
/// rows at multiples of `interval` and always retain the final step so
/// downstream consumers can read the landing state directly.
fn keep_step(i: usize, len: usize, interval: usize) -> bool {
    interval <= 1 || i.is_multiple_of(interval) || i + 1 == len
}

const CSV_HEADER: &str = "\
time_sec,\
lat_deg,lon_deg,alt_agl_m,\
u_mps,v_mps,w_mps,true_airspeed_mps,ground_speed_mps,\
pitch_deg,roll_deg,yaw_deg,\
p_rad_sec,q_rad_sec,r_rad_sec,\
ax_mps2,ay_mps2,az_mps2,\
alpha_deg,beta_deg,qbar_pa,\
thrust_n,mach";

fn write_trajectory_csv(path: &Path, traj: &[SimulationState], interval: usize) -> Result<()> {
    let f = fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = BufWriter::new(f);
    writeln!(writer, "{CSV_HEADER}")?;
    let len = traj.len();
    for (i, s) in traj.iter().enumerate() {
        if !keep_step(i, len, interval) {
            continue;
        }
        writeln!(
            writer,
            "{:.9},{:.9},{:.9},{:.6},\
             {:.6},{:.6},{:.6},{:.6},{:.6},\
             {:.6},{:.6},{:.6},\
             {:.6},{:.6},{:.6},\
             {:.6},{:.6},{:.6},\
             {:.6},{:.6},{:.6},\
             {:.6},{:.6}",
            s.time_sec,
            s.position.lat_deg,
            s.position.lon_deg,
            s.position.alt_agl_m,
            s.velocity.u_mps,
            s.velocity.v_mps,
            s.velocity.w_mps,
            s.velocity.true_airspeed_mps,
            s.velocity.ground_speed_mps,
            s.attitude.pitch_deg,
            s.attitude.roll_deg,
            s.attitude.yaw_deg,
            s.angular_rates.p_rad_sec,
            s.angular_rates.q_rad_sec,
            s.angular_rates.r_rad_sec,
            s.acceleration.ax_mps2,
            s.acceleration.ay_mps2,
            s.acceleration.az_mps2,
            s.aero.alpha_deg,
            s.aero.beta_deg,
            s.aero.qbar_pa,
            s.thrust_n,
            s.mach,
        )?;
    }
    writer.flush()?;
    Ok(())
}

fn write_events_json(path: &Path, out: &UnifiedSimulationOutput) -> Result<()> {
    let f = fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let writer = BufWriter::new(f);
    serde_json::to_writer_pretty(writer, &out.events)?;
    Ok(())
}

#[derive(Serialize)]
struct Summary<'a> {
    apogee_m: f64,
    max_speed_mps: f64,
    flight_time_sec: f64,
    landing: Option<LandingPoint>,
    events_count: usize,
    phase_final: &'a str,
}

#[derive(Serialize)]
struct LandingPoint {
    lat_deg: f64,
    lon_deg: f64,
    alt_agl_m: f64,
    source: &'static str,
}

fn write_summary_json(path: &Path, out: &UnifiedSimulationOutput) -> Result<()> {
    let apogee_m = out.mainline.max_altitude_m;
    let max_speed_mps = out
        .mainline
        .max_speed_mps
        .max(out.parachute_branch.max_speed_mps);

    let mainline_end_t = out.mainline.flight_time_sec;
    let parachute_end_t = out.parachute_branch.flight_time_sec;
    let flight_time_sec = mainline_end_t.max(parachute_end_t);

    // Prefer a parachute landing if present, else fall back to the last
    // mainline state.
    let has_parachute_landed = out
        .events
        .iter()
        .any(|e| e.kind == EventKind::ParachuteLanded);
    let landing = if has_parachute_landed {
        out.parachute_branch
            .trajectory
            .last()
            .map(|s| LandingPoint {
                lat_deg: s.position.lat_deg,
                lon_deg: s.position.lon_deg,
                alt_agl_m: s.position.alt_agl_m,
                source: "parachute",
            })
    } else {
        out.mainline.trajectory.last().map(|s| LandingPoint {
            lat_deg: s.position.lat_deg,
            lon_deg: s.position.lon_deg,
            alt_agl_m: s.position.alt_agl_m,
            source: "ballistic",
        })
    };

    let summary = Summary {
        apogee_m,
        max_speed_mps,
        flight_time_sec,
        landing,
        events_count: out.events.len(),
        phase_final: "Completed",
    };

    let f = fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let writer = BufWriter::new(f);
    serde_json::to_writer_pretty(writer, &summary)?;
    Ok(())
}
