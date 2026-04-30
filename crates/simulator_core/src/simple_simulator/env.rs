//! Shared physical-environment helpers used by every stage runner.
//!
//! These were originally private to `LaunchRailStage` but are needed by
//! `ParachuteStage` too, so they live here as `pub(crate)` primitives.


pub(crate) const EARTH_RADIUS_M: f64 = 6_378_137.0;
pub(crate) const G0_MPS2: f64 = 9.806_65;

/// Dot product for 3-element arrays.
pub(crate) fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Wind velocity vector in ENU (m/s), from a meteorological
/// `[alt_msl_m, speed_mps, from_direction_deg]` table.
///
/// Meteorological convention: `from_direction_deg` is the direction
/// the wind is coming *from* (0° = from north, 90° = from east).
/// Hence wind flows *towards* the opposite azimuth — the east and
/// north components are negated accordingly.
///
/// Altitudes outside the table range are clamped to the nearest row.
pub(crate) fn wind_enu_at_alt(winds_table: &[[f64; 3]], alt_msl_m: f64) -> [f64; 3] {
    if winds_table.is_empty() {
        return [0.0, 0.0, 0.0];
    }
    let mut sorted: Vec<[f64; 3]> = winds_table.to_vec();
    sorted.sort_by(|a, b| a[0].total_cmp(&b[0]));

    let (speed, dir_deg) = if alt_msl_m <= sorted[0][0] {
        (sorted[0][1], sorted[0][2])
    } else if alt_msl_m >= sorted[sorted.len() - 1][0] {
        let last = sorted.last().unwrap();
        (last[1], last[2])
    } else {
        let mut s = 0.0;
        let mut d = 0.0;
        for w in sorted.windows(2) {
            let (a, b) = (w[0], w[1]);
            if alt_msl_m >= a[0] && alt_msl_m <= b[0] {
                let t = (alt_msl_m - a[0]) / (b[0] - a[0]);
                s = a[1] + t * (b[1] - a[1]);
                d = a[2] + t * (b[2] - a[2]);
                break;
            }
        }
        (s, d)
    };

    let theta = dir_deg.to_radians();
    [-speed * theta.sin(), -speed * theta.cos(), 0.0]
}

/// Advance a geodetic position by a small ENU offset using the spherical-Earth
/// approximation (adequate for the per-step displacements the simulator
/// produces at rocket speeds).
pub(crate) fn advance_latlon_by_enu(
    lat_deg: f64,
    lon_deg: f64,
    east_m: f64,
    north_m: f64,
) -> (f64, f64) {
    let dlat_deg = (north_m / EARTH_RADIUS_M).to_degrees();
    let lon_scale = lat_deg.to_radians().cos().abs().max(1e-6);
    let dlon_deg = (east_m / (EARTH_RADIUS_M * lon_scale)).to_degrees();
    (lat_deg + dlat_deg, lon_deg + dlon_deg)
}