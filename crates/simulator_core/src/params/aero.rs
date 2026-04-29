use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// 2D drag table in JSBSim tableData layout.
///
/// Format:
/// - `mach_keys`: column keys
/// - `rows`: each row is `[alpha_key, cd(mach_1), cd(mach_2), ...]`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cd0AlphaMachTable {
    #[serde(with = "crate::arc_serde::slice")]
    pub mach_keys: Arc<[f64]>,
    #[serde(with = "crate::arc_serde::slice")]
    pub rows: Arc<[Vec<f64>]>,
}

/// Aerodynamic model parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AeroParams {
    /// CP x-position at launch (before any Mach variation) [x, y, z] (m).
    /// Used as the static AERORP location in `<metrics>`.
    pub cp_at_launch: [f64; 3],

    /// CP x-position vs Mach: `[[mach, cp_x_m], …]`.
    #[serde(with = "crate::arc_serde::slice")]
    pub cp_mach_table: Arc<[[f64; 2]]>,

    // ── Drag ──────────────────────────────────────────────────────────────
    /// Axial drag coefficient table (alpha × Mach).
    pub cd0_alpha_mach_table: Cd0AlphaMachTable,

    // ── Lift / Normal (already integrated at input stage) ────────────────
    /// Integrated CN-alpha slope vs Mach: `[[mach, CNα_total], …]`.
    #[serde(with = "crate::arc_serde::slice")]
    pub cn_table: Arc<[[f64; 2]]>,

    // ── Side (already integrated at input stage) ─────────────────────────
    /// Integrated CS-beta slope vs Mach: `[[mach, CSβ_total], …]`.
    #[serde(with = "crate::arc_serde::slice")]
    pub cs_table: Arc<[[f64; 2]]>,

    // ── Damping ───────────────────────────────────────────────────────────
    /// Roll damping Clp.
    pub roll_damping_coefficient: f64,
    /// Pitch damping Cmq.
    pub pitch_damping_coefficient: f64,
    /// Yaw damping Cnr.
    pub yaw_damping_coefficient: f64,
}
