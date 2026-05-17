//! Post-processing pipeline: trait, context, and built-in output steps.
//!
//! To add a new output format, implement [`PostProcessor`] and include it in
//! [`default_mandatory_steps`] (or pass it as an optional step to [`run_pipeline`]).

use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use simulator_core::{
    EventKind, EventStamp, RocketParams, SimulationState, UnifiedSimulationOutput,
};

// ── Context ──────────────────────────────────────────────────────────────────

/// Shared, immutable context passed to every [`PostProcessor`] step.
///
/// Created once per run after DEM refinement completes. Intervals are
/// already normalised via [`normalise_intervals`].
pub struct RunContext<'a> {
    pub output: &'a UnifiedSimulationOutput,
    pub out_dir: &'a Path,
    pub params: &'a RocketParams,
    /// Normalised CSV decimation interval (≥ 1).
    pub csv_interval: usize,
    /// Normalised KML decimation interval (≥ 1).
    pub kml_interval: usize,
}

/// Paths of the files written by the standard mandatory steps.
#[derive(Debug, Clone)]
pub struct RunPaths {
    pub mainline: PathBuf,
    pub parachute: PathBuf,
    pub events: PathBuf,
    pub events_csv: PathBuf,
    pub summary: PathBuf,
    pub kml: PathBuf,
}

impl RunPaths {
    pub fn from_context(ctx: &RunContext<'_>) -> Self {
        RunPaths {
            mainline: ctx.out_dir.join("mainline.csv"),
            parachute: ctx.out_dir.join("parachute.csv"),
            events: ctx.out_dir.join("events.json"),
            events_csv: ctx.out_dir.join("events.csv"),
            summary: ctx.out_dir.join("summary.json"),
            kml: ctx.out_dir.join("trajectory.kml"),
        }
    }
}

/// Normalise a (csv, kml) interval pair so that both are relative to the
/// smaller of the two, matching the behaviour of the original `write_outputs`.
pub fn normalise_intervals(csv: usize, kml: usize) -> (usize, usize) {
    let m = csv.min(kml).max(1);
    (csv / m, kml / m)
}

// ── Trait ────────────────────────────────────────────────────────────────────

/// A single post-processing step executed after simulation (and optional DEM
/// refinement). Implementations write files or produce other side-effects.
/// They must not mutate the simulation output.
pub trait PostProcessor: Send + Sync {
    fn name(&self) -> &str;
    fn run(&self, ctx: &RunContext<'_>) -> Result<()>;
}

// ── Pipeline runner ───────────────────────────────────────────────────────────

/// Run `mandatory` steps (abort on error), then `optional` steps (warn and continue).
pub fn run_pipeline(
    ctx: &RunContext<'_>,
    mandatory: &[Box<dyn PostProcessor>],
    optional: &[Box<dyn PostProcessor>],
) -> Result<()> {
    for step in mandatory {
        step.run(ctx)
            .with_context(|| format!("pipeline step '{}' failed", step.name()))?;
    }
    for step in optional {
        if let Err(e) = step.run(ctx) {
            eprintln!("warn: optional step '{}' failed: {e:#}", step.name());
        }
    }
    Ok(())
}

/// Standard five mandatory output steps.
pub fn default_mandatory_steps() -> Vec<Box<dyn PostProcessor>> {
    vec![
        Box::new(WriteTrajectoryStep),
        Box::new(WriteEventsJsonStep),
        Box::new(WriteEventsCsvStep),
        Box::new(WriteSummaryJsonStep),
        Box::new(WriteKmlStep),
    ]
}

// ── Implementations ───────────────────────────────────────────────────────────

/// Writes `mainline.csv` and (if a parachute branch exists) `parachute.csv`.
pub struct WriteTrajectoryStep;

impl PostProcessor for WriteTrajectoryStep {
    fn name(&self) -> &str {
        "write trajectory CSV"
    }

    fn run(&self, ctx: &RunContext<'_>) -> Result<()> {
        let out = ctx.output;
        let paths = RunPaths::from_context(ctx);

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
            ctx.csv_interval,
        )?;

        if let Some(split) = index_at_parachute_open {
            write_trajectory_csv(
                &paths.parachute,
                out.mainline
                    .trajectory
                    .row_iter()
                    .take(split)
                    .chain(out.parachute_branch.trajectory.row_iter()),
                split + out.parachute_branch.trajectory.len(),
                ctx.csv_interval,
            )?;
        }

        Ok(())
    }
}

/// Writes `events.json`.
pub struct WriteEventsJsonStep;

impl PostProcessor for WriteEventsJsonStep {
    fn name(&self) -> &str {
        "write events JSON"
    }

    fn run(&self, ctx: &RunContext<'_>) -> Result<()> {
        let path = ctx.out_dir.join("events.json");
        let f = fs::File::create(&path).with_context(|| format!("creating {}", path.display()))?;
        serde_json::to_writer_pretty(BufWriter::new(f), &ctx.output.events)?;
        Ok(())
    }
}

/// Writes `events.csv`.
pub struct WriteEventsCsvStep;

impl PostProcessor for WriteEventsCsvStep {
    fn name(&self) -> &str {
        "write events CSV"
    }

    fn run(&self, ctx: &RunContext<'_>) -> Result<()> {
        write_events_csv(&ctx.out_dir.join("events.csv"), &ctx.output.events)
    }
}

/// Writes `summary.json`.
pub struct WriteSummaryJsonStep;

impl PostProcessor for WriteSummaryJsonStep {
    fn name(&self) -> &str {
        "write summary JSON"
    }

    fn run(&self, ctx: &RunContext<'_>) -> Result<()> {
        write_summary_json(&ctx.out_dir.join("summary.json"), ctx.output)
    }
}

/// Writes `trajectory.kml`.
pub struct WriteKmlStep;

impl PostProcessor for WriteKmlStep {
    fn name(&self) -> &str {
        "write trajectory KML"
    }

    fn run(&self, ctx: &RunContext<'_>) -> Result<()> {
        crate::kml_writer::write_trajectory_kml_file(
            &ctx.out_dir.join("trajectory.kml"),
            ctx.output,
            ctx.params,
            ctx.kml_interval,
        )
    }
}

/// Draws BMP chart plots (optional — warnings only on failure).
pub struct DrawChartsStep;

impl PostProcessor for DrawChartsStep {
    fn name(&self) -> &str {
        "draw charts"
    }

    fn run(&self, ctx: &RunContext<'_>) -> Result<()> {
        crate::chart::draw_result_plot(ctx.out_dir, ctx.output).map_err(|e| anyhow::anyhow!("{e}"))
    }
}

// ── Private helpers (moved from runner.rs) ────────────────────────────────────

/// Keep trajectory step `i` based on interval decimation. Always retains the
/// final step so the landing state is readable directly from the last row.
pub(crate) fn keep_step(i: usize, len: usize, interval: usize) -> bool {
    interval <= 1 || i.is_multiple_of(interval) || i + 1 == len
}

#[derive(Serialize)]
struct SimStateCsvRow {
    time_sec: f64,
    lat_deg: f64,
    lon_deg: f64,
    alt_msl_m: f64,
    down_range_m: f64,
    local_x_m: f64,
    local_y_m: f64,
    u_mps: f64,
    v_mps: f64,
    w_mps: f64,
    true_airspeed_mps: f64,
    ground_speed_mps: f64,
    pitch_deg: f64,
    roll_deg: f64,
    yaw_deg: f64,
    p_rad_sec: f64,
    q_rad_sec: f64,
    r_rad_sec: f64,
    ax_mps2: f64,
    ay_mps2: f64,
    az_mps2: f64,
    alpha_deg: f64,
    beta_deg: f64,
    qbar_pa: f64,
    total_aoa_deg: f64,
    pressure_pa: f64,
    temperature_k: f64,
    gust_airspeed_mps: f64,
    gust_aoa_deg: f64,
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

#[derive(Serialize)]
struct EventCsvRow {
    kind: String,
    source: String,
    sim_time_sec: f64,
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

    let flight_time_sec = out
        .mainline
        .flight_time_sec
        .max(out.parachute_branch.flight_time_sec);

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
    serde_json::to_writer_pretty(BufWriter::new(f), &summary)?;
    Ok(())
}
