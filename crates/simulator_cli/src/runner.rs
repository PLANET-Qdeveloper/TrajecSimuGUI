//! Drive the `SimulationOrchestrator` end-to-end and write outputs.

use std::cmp::min;
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;

use simulator_core::analysis;

use simulator_core::{
    EventKind, EventStamp, Phase, RocketParams, SimulationOrchestrator, SimulationState,
    UnifiedSimulationOutput,
};

#[derive(Debug, Clone)]
pub struct RunPaths {
    pub mainline: PathBuf,
    pub parachute: PathBuf,
    pub events: PathBuf,
    pub events_csv: PathBuf,
    pub summary: PathBuf,
    pub kml: PathBuf,
}

/// Run the simulation and post-analysis pass. No file I/O.
pub fn simulate(params: &RocketParams) -> Result<UnifiedSimulationOutput> {
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

    let mut output = orch.into_output();
    analysis::analyze(&mut output, params);
    Ok(output)
}

/// Write all output files from a finished (and optionally refined) output.
pub fn write_outputs(
    out: &UnifiedSimulationOutput,
    out_dir: &Path,
    csv_interval: usize,
    kml_interval: usize,
    params: &RocketParams,
) -> Result<RunPaths> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("creating output dir {}", out_dir.display()))?;

    let paths = RunPaths {
        mainline: out_dir.join("mainline.csv"),
        parachute: out_dir.join("parachute.csv"),
        events: out_dir.join("events.json"),
        events_csv: out_dir.join("events.csv"),
        summary: out_dir.join("summary.json"),
        kml: out_dir.join("trajectory.kml"),
    };

    let norm_csv = csv_interval / min(csv_interval, kml_interval);
    let norm_kml = kml_interval / min(csv_interval, kml_interval);

    let time_at_parachute_open = out
        .events
        .iter()
        .find(|e| e.kind == EventKind::ParachuteOpen)
        .map(|e| e.sim_time_sec);
    let index_at_parachute_open = time_at_parachute_open.and_then(|t| {
        out.mainline
            .trajectory
            .row_iter()
            .position(|s| s.time_sec >= t)
    });

    write_trajectory_csv(
        &paths.mainline,
        out.mainline.trajectory.row_iter(),
        out.mainline.trajectory.len(),
        norm_csv,
    )?;
    if let Some(index_at_parachute_open) = index_at_parachute_open {
        write_trajectory_csv(
            &paths.parachute,
            out.mainline
                .trajectory
                .row_iter()
                .take(index_at_parachute_open)
                .chain(out.parachute_branch.trajectory.row_iter()),
            index_at_parachute_open + out.parachute_branch.trajectory.len(),
            norm_csv,
        )?;
    }

    write_events_json(&paths.events, out)?;
    write_events_csv(&paths.events_csv, &out.events)?;
    write_summary_json(&paths.summary, out)?;
    crate::kml_writer::write_trajectory_kml_file(&paths.kml, out, params, norm_kml)?;

    Ok(paths)
}

/// Convenience wrapper: simulate + write outputs (no DEM refinement).
#[allow(dead_code)]
pub fn run(
    params: &RocketParams,
    out_dir: &Path,
    csv_interval: usize,
    kml_interval: usize,
) -> Result<RunPaths> {
    let output = simulate(params)?;
    write_outputs(&output, out_dir, csv_interval, kml_interval, params)
}

/// `i` is the trajectory step index, `len` the trajectory length. Keep
/// rows at multiples of `interval` and always retain the final step so
/// downstream consumers can read the landing state directly.
pub(crate) fn keep_step(i: usize, len: usize, interval: usize) -> bool {
    interval <= 1 || i.is_multiple_of(interval) || i + 1 == len
}

/// Flat CSV row derived from `SimulationState`.
///
/// The csv crate auto-generates the header from field names, so adding a field
/// here (and to the `From` impl below) is the only change needed when the
/// output schema grows.
#[derive(Serialize)]
struct SimStateCsvRow {
    // ── Time ────────────────────────────────────────────────────────────
    time_sec: f64,
    // ── Position ────────────────────────────────────────────────────────
    lat_deg: f64,
    lon_deg: f64,
    alt_msl_m: f64,
    down_range_m: f64,
    local_x_m: f64,
    local_y_m: f64,
    // ── Velocity ────────────────────────────────────────────────────────
    u_mps: f64,
    v_mps: f64,
    w_mps: f64,
    true_airspeed_mps: f64,
    ground_speed_mps: f64,
    // ── Attitude ────────────────────────────────────────────────────────
    pitch_deg: f64,
    roll_deg: f64,
    yaw_deg: f64,
    // ── Angular rates ────────────────────────────────────────────────────
    p_rad_sec: f64,
    q_rad_sec: f64,
    r_rad_sec: f64,
    // ── Acceleration ─────────────────────────────────────────────────────
    ax_mps2: f64,
    ay_mps2: f64,
    az_mps2: f64,
    // ── Aerodynamics / atmosphere ────────────────────────────────────────
    alpha_deg: f64,
    beta_deg: f64,
    qbar_pa: f64,
    total_aoa_deg: f64,
    pressure_pa: f64,
    temperature_k: f64,
    gust_airspeed_mps: f64,
    gust_aoa_deg: f64,
    // ── Propulsion ───────────────────────────────────────────────────────
    thrust_n: f64,
    mach: f64,
}

impl From<&SimulationState> for SimStateCsvRow {
    fn from(s: &SimulationState) -> Self {
        let p = &s.position;
        let v = &s.velocity;
        let att = &s.attitude;
        let ar = &s.angular_rates;
        let acc = &s.acceleration;
        let aero = &s.aero;
        SimStateCsvRow {
            time_sec: s.time_sec,
            lat_deg: p.lat_deg,
            lon_deg: p.lon_deg,
            alt_msl_m: p.alt_msl_m,
            down_range_m: p.down_range_m,
            local_x_m: p.local_x_m,
            local_y_m: p.local_y_m,
            u_mps: v.u_mps,
            v_mps: v.v_mps,
            w_mps: v.w_mps,
            true_airspeed_mps: v.true_airspeed_mps,
            ground_speed_mps: v.ground_speed_mps,
            pitch_deg: att.pitch_deg,
            roll_deg: att.roll_deg,
            yaw_deg: att.yaw_deg,
            p_rad_sec: ar.p_rad_sec,
            q_rad_sec: ar.q_rad_sec,
            r_rad_sec: ar.r_rad_sec,
            ax_mps2: acc.ax_mps2,
            ay_mps2: acc.ay_mps2,
            az_mps2: acc.az_mps2,
            alpha_deg: aero.alpha_deg,
            beta_deg: aero.beta_deg,
            qbar_pa: aero.qbar_pa,
            total_aoa_deg: aero.total_aoa_deg,
            pressure_pa: aero.pressure_pa,
            temperature_k: aero.temperature_k,
            gust_airspeed_mps: aero.gust_airspeed_mps,
            gust_aoa_deg: aero.gust_aoa_deg,
            thrust_n: s.thrust_n,
            mach: s.mach,
        }
    }
}

fn write_trajectory_csv(
    path: &Path,
    traj: impl Iterator<Item = SimulationState>,
    data_len: usize,
    interval: usize,
) -> Result<()> {
    let f = fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = csv::Writer::from_writer(BufWriter::new(f));
    for (i, s) in traj.enumerate() {
        if !keep_step(i, data_len, interval) {
            continue;
        }
        writer
            .serialize(SimStateCsvRow::from(&s))
            .with_context(|| format!("writing CSV row {i}"))?;
    }
    writer.flush()?;
    Ok(())
}

/// Flat CSV row for a single event.
/// State fields are `Option<f64>` — empty cells when the event carries no state.
#[derive(Serialize)]
struct EventCsvRow {
    kind: String,
    source: String,
    sim_time_sec: f64,
    // State fields (mirrors SimStateCsvRow but optional)
    state_time_sec: Option<f64>,
    lat_deg: Option<f64>,
    lon_deg: Option<f64>,
    alt_msl_m: Option<f64>,
    down_range_m: Option<f64>,
    local_x_m: Option<f64>,
    local_y_m: Option<f64>,
    u_mps: Option<f64>,
    v_mps: Option<f64>,
    w_mps: Option<f64>,
    true_airspeed_mps: Option<f64>,
    ground_speed_mps: Option<f64>,
    pitch_deg: Option<f64>,
    roll_deg: Option<f64>,
    yaw_deg: Option<f64>,
    p_rad_sec: Option<f64>,
    q_rad_sec: Option<f64>,
    r_rad_sec: Option<f64>,
    ax_mps2: Option<f64>,
    ay_mps2: Option<f64>,
    az_mps2: Option<f64>,
    alpha_deg: Option<f64>,
    beta_deg: Option<f64>,
    qbar_pa: Option<f64>,
    total_aoa_deg: Option<f64>,
    pressure_pa: Option<f64>,
    temperature_k: Option<f64>,
    gust_airspeed_mps: Option<f64>,
    gust_aoa_deg: Option<f64>,
    thrust_n: Option<f64>,
    mach: Option<f64>,
}

impl From<&EventStamp> for EventCsvRow {
    fn from(e: &EventStamp) -> Self {
        let s = e.state.as_ref();
        let p = s.map(|s| &s.position);
        let v = s.map(|s| &s.velocity);
        let att = s.map(|s| &s.attitude);
        let ar = s.map(|s| &s.angular_rates);
        let acc = s.map(|s| &s.acceleration);
        let aero = s.map(|s| &s.aero);
        EventCsvRow {
            kind: e.kind.to_string(),
            source: format!("{:?}", e.source),
            sim_time_sec: e.sim_time_sec,
            state_time_sec: s.map(|s| s.time_sec),
            lat_deg: p.map(|p| p.lat_deg),
            lon_deg: p.map(|p| p.lon_deg),
            alt_msl_m: p.map(|p| p.alt_msl_m),
            down_range_m: p.map(|p| p.down_range_m),
            local_x_m: p.map(|p| p.local_x_m),
            local_y_m: p.map(|p| p.local_y_m),
            u_mps: v.map(|v| v.u_mps),
            v_mps: v.map(|v| v.v_mps),
            w_mps: v.map(|v| v.w_mps),
            true_airspeed_mps: v.map(|v| v.true_airspeed_mps),
            ground_speed_mps: v.map(|v| v.ground_speed_mps),
            pitch_deg: att.map(|a| a.pitch_deg),
            roll_deg: att.map(|a| a.roll_deg),
            yaw_deg: att.map(|a| a.yaw_deg),
            p_rad_sec: ar.map(|a| a.p_rad_sec),
            q_rad_sec: ar.map(|a| a.q_rad_sec),
            r_rad_sec: ar.map(|a| a.r_rad_sec),
            ax_mps2: acc.map(|a| a.ax_mps2),
            ay_mps2: acc.map(|a| a.ay_mps2),
            az_mps2: acc.map(|a| a.az_mps2),
            alpha_deg: aero.map(|a| a.alpha_deg),
            beta_deg: aero.map(|a| a.beta_deg),
            qbar_pa: aero.map(|a| a.qbar_pa),
            total_aoa_deg: aero.map(|a| a.total_aoa_deg),
            pressure_pa: aero.map(|a| a.pressure_pa),
            temperature_k: aero.map(|a| a.temperature_k),
            gust_airspeed_mps: aero.map(|a| a.gust_airspeed_mps),
            gust_aoa_deg: aero.map(|a| a.gust_aoa_deg),
            thrust_n: s.map(|s| s.thrust_n),
            mach: s.map(|s| s.mach),
        }
    }
}

fn write_events_csv(path: &Path, events: &[EventStamp]) -> Result<()> {
    let f = fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = csv::Writer::from_writer(BufWriter::new(f));
    for e in events {
        writer
            .serialize(EventCsvRow::from(e))
            .with_context(|| format!("writing event CSV row: {:?}", e.kind))?;
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
    alt_msl_m: f64,
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
            .last_state()
            .map(|s| LandingPoint {
                lat_deg: s.position.lat_deg,
                lon_deg: s.position.lon_deg,
                alt_msl_m: s.position.alt_msl_m,
                source: "parachute",
            })
    } else {
        out.mainline.trajectory.last_state().map(|s| LandingPoint {
            lat_deg: s.position.lat_deg,
            lon_deg: s.position.lon_deg,
            alt_msl_m: s.position.alt_msl_m,
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
