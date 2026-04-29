use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::progress::DelayedBranchTrigger;

/// Parachute descent parameters.
///
/// The parachute stage is modelled as a 3-D point mass under gravity and
/// drag. Drag is specified implicitly through a time-varying terminal
/// velocity `v_term(t)` — the effective drag coefficient is recovered
/// from the equilibrium relation `a_drag = (g / v_term²) · |v_rel| · v_rel`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParachuteParams {
    /// `[[t_since_deploy_sec, v_terminal_mps], ...]`.
    ///
    /// Magnitude (positive), downward. The time axis origin is the
    /// parachute-deployment instant, not absolute sim-time, so staged
    /// openings (drogue → main) can be expressed independently of
    /// launch or apogee times.
    ///
    /// Empty → parachute disabled. The orchestrator treats this as
    /// "no parachute branch" and terminates after ballistic flight ends.
    #[serde(default = "empty_arc_table", with = "crate::arc_serde::slice")]
    pub terminal_velocity_table: Arc<[[f64; 2]]>,

    /// Event-driven deployment trigger. Deployment fires `delay_sec`
    /// after the origin event is observed (typically `Apogee`).
    ///
    /// `None` disables parachute deployment even if the terminal-velocity
    /// table is populated.
    #[serde(default)]
    pub deploy_trigger: Option<DelayedBranchTrigger>,

    /// Fractional tolerance against the current terminal velocity for
    /// declaring the vehicle "settled" on the chute. Once every velocity
    /// component is within `tol_frac · v_term(t)` of its steady-state
    /// target, the integrator switches to the analytic steady-state mode.
    #[serde(default = "default_settle_tol_frac")]
    pub settle_tol_frac: f64,

    /// Number of consecutive steps the settle criterion must hold before
    /// switching modes. Provides hysteresis against transient oscillation
    /// around the terminal-velocity envelope.
    #[serde(default = "default_settle_hold_steps")]
    pub settle_hold_steps: u32,
}

fn default_settle_tol_frac() -> f64 {
    0.05
}

fn default_settle_hold_steps() -> u32 {
    5
}

fn empty_arc_table() -> Arc<[[f64; 2]]> {
    Arc::from(Vec::<[f64; 2]>::new())
}

impl Default for ParachuteParams {
    fn default() -> Self {
        Self {
            terminal_velocity_table: empty_arc_table(),
            deploy_trigger: None,
            settle_tol_frac: default_settle_tol_frac(),
            settle_hold_steps: default_settle_hold_steps(),
        }
    }
}
