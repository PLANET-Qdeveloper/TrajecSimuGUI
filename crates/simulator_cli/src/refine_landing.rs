//! Landing point refinement using GSI DEM elevation tiles.
//!
//! The simulator's `alt_agl_m` is height above the **launch site elevation**
//! (flat-terrain JSBSim assumption). When the rocket lands elsewhere, the
//! actual terrain may be higher or lower. This module computes the true
//! terrain crossing for both the ballistic (Landed) and parachute
//! (ParachuteLanded) trajectories independently, then overwrites the
//! corresponding event state with the interpolated position.

use anyhow::Result;

use simulator_core::output::{Position, SimulationState, Trajectory, Velocity};
use simulator_core::progress::EventKind;
use simulator_core::UnifiedSimulationOutput;

use crate::dem::DemCache;


/// Refine the Landed and ParachuteLanded events in `output` using DEM terrain
/// data. Modifies the matching event states in-place.
pub fn refine_one(
    output: &mut UnifiedSimulationOutput,
    dem: &DemCache,
) -> Result<()> {
    if !output.mainline.trajectory.is_empty() {
        if let Some(state) =
            find_terrain_crossing(&output.mainline.trajectory, dem)?
        {
            update_event(&mut output.events, EventKind::Landed, state);
        }
    }

    if !output.parachute_branch.trajectory.is_empty() {
        if let Some(state) =
            find_terrain_crossing(&output.parachute_branch.trajectory, dem)?
        {
            update_event(&mut output.events, EventKind::ParachuteLanded, state);
        }
    }

    Ok(())
}

// ── Internal ─────────────────────────────────────────────────────────────────

/// Walk backwards through `traj` to find where the rocket crosses the actual
/// terrain. Returns an interpolated `SimulationState` at the exact crossing,
/// or `None` if no suitable crossing is found (e.g. no DEM data available).
fn find_terrain_crossing(
    traj: &Trajectory,
    dem: &DemCache,
) -> Result<Option<SimulationState>> {
    if traj.len() < 2 {
        return Ok(None);
    }

    let len = traj.len();
    let mut idx_above: Option<usize> = None;

    for i in (0..len - 1).rev() {
        let s = traj.get_state(i);
        match compute_true_agl(&s, dem)? {
            Some(agl) if agl > 0.0 => {
                idx_above = Some(i);
                break;
            }
            _ => {}
        }
    }

    let idx_above = match idx_above {
        Some(i) => i,
        None => return Ok(None),
    };

    let idx_below = idx_above + 1;
    if idx_below >= len {
        return Ok(None);
    }

    let a = traj.get_state(idx_above);
    let b = traj.get_state(idx_below);

    let agl_a = match compute_true_agl(&a, dem)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let agl_b = match compute_true_agl(&b, dem)? {
        Some(v) => v,
        None => return Ok(None),
    };

    if agl_a <= 0.0 || agl_b >= 0.0 {
        return Ok(Some(b));
    }

    let t = agl_a / (agl_a - agl_b);
    Ok(Some(interpolate_state(&a, &b, t)))
}

fn compute_true_agl(
    s: &SimulationState,
    dem: &DemCache,
) -> Result<Option<f64>> {
    match dem.get_elevation(s.position.lat_deg, s.position.lon_deg)? {
        Some(h_terrain) => Ok(Some(s.position.alt_agl_m - (h_terrain))),
        None => Ok(None),
    }
}

fn interpolate_state(a: &SimulationState, b: &SimulationState, t: f64) -> SimulationState {
    let lerp = |va: f64, vb: f64| va + t * (vb - va);
    SimulationState {
        time_sec: lerp(a.time_sec, b.time_sec),
        position: Position {
            lat_deg: lerp(a.position.lat_deg, b.position.lat_deg),
            lon_deg: lerp(a.position.lon_deg, b.position.lon_deg),
            alt_agl_m: lerp(a.position.alt_agl_m, b.position.alt_agl_m),
            down_range_m: lerp(a.position.down_range_m, b.position.down_range_m),
            local_x_m: lerp(a.position.local_x_m, b.position.local_x_m),
            local_y_m: lerp(a.position.local_y_m, b.position.local_y_m),
        },
        velocity: Velocity {
            true_airspeed_mps: lerp(a.velocity.true_airspeed_mps, b.velocity.true_airspeed_mps),
            ground_speed_mps: lerp(a.velocity.ground_speed_mps, b.velocity.ground_speed_mps),
            u_mps: lerp(a.velocity.u_mps, b.velocity.u_mps),
            v_mps: lerp(a.velocity.v_mps, b.velocity.v_mps),
            w_mps: lerp(a.velocity.w_mps, b.velocity.w_mps),
        },
        attitude: a.attitude.clone(),
        angular_rates: a.angular_rates.clone(),
        acceleration: a.acceleration.clone(),
        aero: a.aero.clone(),
        thrust_n: lerp(a.thrust_n, b.thrust_n),
        mach: lerp(a.mach, b.mach),
    }
}

fn update_event(
    events: &mut [simulator_core::progress::EventStamp],
    kind: EventKind,
    state: SimulationState,
) {
    if let Some(ev) = events.iter_mut().find(|e| e.kind == kind) {
        ev.state = Some(state);
    }
}
