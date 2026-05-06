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

impl Cd0AlphaMachTable {
    pub fn get_value(&self, alpha: f64, mach: f64) -> f64 {
        let mach_keys = &self.mach_keys;
        let rows = &self.rows;

        // 1. マッハ方向のインデックスを探す (x軸)
        let m_idx = self.find_lower_index(mach_keys, mach);
        let m0 = mach_keys[m_idx];
        let m1 = mach_keys[m_idx + 1];

        // 2. AoA方向のインデックスを探す (y軸)
        // 各行の最初の要素がAlphaキーであることを前提とする
        let a_keys: Vec<f64> = rows.iter().map(|r| r[0]).collect();
        let a_idx = self.find_lower_index(&a_keys, alpha);
        let a0 = a_keys[a_idx];
        let a1 = a_keys[a_idx + 1];

        // 3. 四隅の値を取得 (row[0]はalphaキーなので、machのインデックスは +1 する)
        let q11 = rows[a_idx][m_idx + 1];     // (a0, m0)
        let q21 = rows[a_idx][m_idx + 2];     // (a0, m1)
        let q12 = rows[a_idx + 1][m_idx + 1]; // (a1, m0)
        let q22 = rows[a_idx + 1][m_idx + 2]; // (a1, m1)

        // 4. 二線形補間
        let r1 = self.lerp(m0, m1, q11, q21, mach);
        let r2 = self.lerp(m0, m1, q12, q22, mach);

        self.lerp(a0, a1, r1, r2, alpha)
    }

    // 範囲外をクランプしつつ、左側のインデックスを返す補助関数
    fn find_lower_index(&self, keys: &[f64], val: f64) -> usize {
        if val <= keys[0] { return 0; }
        if val >= keys[keys.len() - 1] { return keys.len() - 2; }
        keys.windows(2)
            .position(|w| val >= w[0] && val <= w[1])
            .unwrap_or(0)
    }

    // 線形補間
    fn lerp(&self, x0: f64, x1: f64, y0: f64, y1: f64, x: f64) -> f64 {
        y0 + (x - x0) * (y1 - y0) / (x1 - x0)
    }
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
