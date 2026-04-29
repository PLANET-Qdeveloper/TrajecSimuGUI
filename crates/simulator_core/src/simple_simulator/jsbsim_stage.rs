//! JSBSim stage adapter.
//!
//! Event detection on top of `JsbSimSimulator`:
//! - `EngineBurnout`: derived **statically** from `engine.thrust_table`
//!   (no property read); fires the first step whose JSBSim sim-time
//!   crosses the burnout instant.
//! - `Apogee`: detected from the vehicle's own state by tracking the
//!   peak AGL altitude; fires the first step the rocket starts
//!   descending past a small float-noise margin.
//! - `Landed`: terrain-aware termination via `TerrainModel`.
//!
//! Time continuity across the rail → JSBSim handoff is handled inside
//! `JsbSimSimulator::initialize` via `FGFDMExec::Setsim_time`, so this
//! stage reports JSBSim's internal time verbatim.

use crate::jsbsim::JsbSimSimulator;
use crate::orchestrator::Phase;
use crate::progress::EventKind;
use crate::simple_simulator::env;
use crate::simple_simulator::{StageRunner, StageStepInput, StageStepOutput};
use crate::{Result, RocketParams, Simulator};

/// JSBSim stage adapter.
pub struct JsbSimStage {
    sim: JsbSimSimulator,
    terrain_terminated: bool,

    /// Burnout instant inferred from `thrust_table`. `None` for empty tables.
    ///
    /// The thrust table in the engine XML is indexed by JSBSim's
    /// `simulation/sim-time-sec`, so this is compared directly against
    /// the simulator's internal time — which the orchestrator seeds to
    /// the rail-exit time via `SimControl::start_sim_time_sec`.
    burnout_time_sec: Option<f64>,
    burnout_emitted: bool,

    apogee: ApogeeTracker,
}

impl JsbSimStage {
    pub fn new() -> Self {
        Self {
            sim: JsbSimSimulator::new(),
            terrain_terminated: false,
            burnout_time_sec: None,
            burnout_emitted: false,
            apogee: ApogeeTracker::default(),
        }
    }

}

impl Default for JsbSimStage {
    fn default() -> Self {
        Self::new()
    }
}

impl StageRunner for JsbSimStage {
    fn initialize(&mut self, params: &RocketParams) -> Result<()> {
        self.terrain_terminated = false;
        self.burnout_time_sec = burnout_time_from_thrust_table(&params.engine.thrust_table);
        self.burnout_emitted = false;
        self.apogee.reset();
        self.sim.initialize(params)
    }

    fn step(&mut self, params: &RocketParams, _input: StageStepInput) -> Result<StageStepOutput> {
        let running = self.sim.step()?;
        let state = self.sim.get_state()?;

        let mut events = Vec::new();
        let mut completed = !running;

        if !self.burnout_emitted {
            if let Some(t_burn) = self.burnout_time_sec {
                if state.time_sec >= t_burn {
                    self.burnout_emitted = true;
                    events.push(EventKind::EngineBurnout);
                }
            }
        }

        if self.apogee.observe(state.position.alt_agl_m) {
            events.push(EventKind::Apogee);
        }

        if !self.terrain_terminated
            && env::hit_terrain(
                params,
                state.position.lat_deg,
                state.position.lon_deg,
                state.position.alt_agl_m,
            )
        {
            self.sim.set_property("simulation/terminate", 1.0)?;
            self.terrain_terminated = true;
            completed = true;
            events.push(EventKind::Landed);
        }

        Ok(StageStepOutput {
            state,
            events,
            completed
        })
    }
}

// ─── Pure helpers (testable without JSBSim) ────────────────────────────────

/// Burnout time (seconds, JSBSim reference) inferred from the thrust curve.
///
/// Definition: the first tabulated time at which thrust drops to 0 *after*
/// at least one sample with thrust > 0. If the curve never reaches 0 in
/// the table, the last sample time is used (consistent with the convention
/// that thrust is treated as 0 past the end of the table).
///
/// Returns `None` only if the table is empty.
fn burnout_time_from_thrust_table(table: &[[f64; 2]]) -> Option<f64> {
    if table.is_empty() {
        return None;
    }
    let mut saw_nonzero = false;
    for &[t, thrust] in table {
        if thrust > 0.0 {
            saw_nonzero = true;
        } else if saw_nonzero {
            return Some(t);
        }
    }
    table.last().map(|&[t, _]| t)
}

/// Peak-altitude apogee detector.
///
/// Fires `observe(...)==true` exactly once, on the first step where the
/// altitude has dropped past a small descent margin below its peak.
/// The `peak_min_m` guard avoids false detection at liftoff when the
/// rocket is still on or near the pad.
#[derive(Debug, Clone, Default)]
struct ApogeeTracker {
    peak_alt_m: f64,
    emitted: bool,
}

impl ApogeeTracker {
    /// Altitude the rocket must have cleared before apogee detection arms.
    const PEAK_MIN_M: f64 = 1.0;
    /// Descent threshold below the peak before apogee fires.
    const DESCENT_MARGIN_M: f64 = 0.1;

    fn observe(&mut self, alt_agl_m: f64) -> bool {
        if alt_agl_m > self.peak_alt_m {
            self.peak_alt_m = alt_agl_m;
            return false;
        }
        if self.emitted {
            return false;
        }
        if self.peak_alt_m > Self::PEAK_MIN_M
            && alt_agl_m < self.peak_alt_m - Self::DESCENT_MARGIN_M
        {
            self.emitted = true;
            return true;
        }
        false
    }

    fn reset(&mut self) {
        self.peak_alt_m = 0.0;
        self.emitted = false;
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // --- burnout_time_from_thrust_table --------------------------------------

    #[test]
    fn burnout_empty_table_is_none() {
        assert_eq!(burnout_time_from_thrust_table(&[]), None);
    }

    #[test]
    fn burnout_at_first_zero_after_nonzero() {
        let table = [[0.0, 1000.0], [5.0, 0.0], [10.0, 0.0]];
        assert_eq!(burnout_time_from_thrust_table(&table), Some(5.0));
    }

    #[test]
    fn burnout_uses_last_time_when_never_zero() {
        // Thrust stays positive across the table; by convention the
        // interpolator treats thrust as 0 past the end → burnout ≈ last time.
        let table = [[0.0, 1000.0], [5.0, 500.0]];
        assert_eq!(burnout_time_from_thrust_table(&table), Some(5.0));
    }

    #[test]
    fn burnout_respects_ignition_delay() {
        // Zero thrust before ignition must not be treated as burnout.
        let table = [[0.0, 0.0], [2.0, 1000.0], [5.0, 0.0]];
        assert_eq!(burnout_time_from_thrust_table(&table), Some(5.0));
    }

    // --- ApogeeTracker -------------------------------------------------------

    #[test]
    fn apogee_never_fires_while_monotonically_rising() {
        let mut t = ApogeeTracker::default();
        for h in [0.5, 1.0, 5.0, 100.0, 1000.0] {
            assert!(!t.observe(h));
        }
        assert!(!t.emitted);
    }

    #[test]
    fn apogee_fires_once_on_first_descent_past_margin() {
        let mut t = ApogeeTracker::default();
        // Climb.
        for h in [0.5, 2.0, 10.0, 50.0, 100.0] {
            assert!(!t.observe(h));
        }
        // Peak reached at 100. First descent past 0.1 m margin fires.
        assert!(t.observe(99.5));
        // Subsequent descending samples do not re-emit.
        assert!(!t.observe(90.0));
        assert!(!t.observe(0.0));
    }

    #[test]
    fn apogee_ignores_noise_within_margin() {
        let mut t = ApogeeTracker::default();
        // Climb past the arming threshold.
        assert!(!t.observe(10.0));
        // Tiny dip within the descent margin → no apogee yet.
        assert!(!t.observe(10.0 - 0.05));
        // Continues to climb.
        assert!(!t.observe(20.0));
        // Real descent.
        assert!(t.observe(19.5));
    }

    #[test]
    fn apogee_does_not_arm_below_peak_min() {
        let mut t = ApogeeTracker::default();
        // Peak stays below 1 m (e.g. rocket still on pad).
        assert!(!t.observe(0.5));
        assert!(!t.observe(0.3));
        assert!(!t.observe(0.0));
        assert!(!t.emitted);
    }

    #[test]
    fn apogee_reset_clears_state() {
        let mut t = ApogeeTracker::default();
        for h in [0.0, 10.0, 5.0] {
            let _ = t.observe(h);
        }
        assert!(t.emitted);
        t.reset();
        assert_eq!(t.peak_alt_m, 0.0);
        assert!(!t.emitted);
    }
}
