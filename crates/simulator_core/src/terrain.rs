//! Terrain models used by the orchestrator's terrain-aware landing termination.
//!
//! The simulator queries `Terrain::altitude_m(lat, lon)` once per ballistic /
//! parachute step. Implementations are expected to be cheap and side-effect
//! free; large lookup data should be loaded once and held inside the impl
//! (typically behind an `Arc<[…]>`) so that cloning the surrounding
//! `Arc<dyn Terrain>` stays a refcount bump.

use std::fmt::Debug;
use std::sync::Arc;

/// Terrain altitude lookup at a geodetic point.
///
/// Returned altitude is in metres, **mean sea level (MSL)**. The simulator's
/// landed-event check compares this against the rocket's current AGL altitude
/// plus the launch-pad elevation, so `altitude_m` should give MSL height
/// regardless of the underlying data source.
pub trait Terrain: Send + Sync + Debug {
    /// Terrain MSL altitude at `(lat_deg, lon_deg)` (m).
    ///
    /// Implementations should clamp at their data range — out-of-range
    /// queries must return a finite value (typically the nearest cell's
    /// altitude) rather than NaN or panicking.
    fn altitude_m(&self, lat_deg: f64, lon_deg: f64) -> f64;
}

/// Constant-altitude terrain. Useful for sea-level pads and tests.
#[derive(Debug, Clone)]
pub struct FlatTerrain {
    /// MSL altitude returned for every (lat, lon) query (m).
    pub altitude_m: f64,
}

impl FlatTerrain {
    pub fn new(altitude_m: f64) -> Self {
        Self { altitude_m }
    }
}

impl Terrain for FlatTerrain {
    fn altitude_m(&self, _lat_deg: f64, _lon_deg: f64) -> f64 {
        self.altitude_m
    }
}

/// Equally-spaced lat × lon raster heightmap with bilinear interpolation.
///
/// Heights are stored row-major: `heights_m[row * n_cols + col]` where row
/// indexes latitude (`origin_lat_deg + row * d_lat_deg`) and column indexes
/// longitude. Out-of-range queries clamp to the nearest edge cell.
#[derive(Debug, Clone)]
pub struct RasterTerrain {
    /// Latitude of the first (row=0) sample (degrees).
    pub origin_lat_deg: f64,
    /// Longitude of the first (col=0) sample (degrees).
    pub origin_lon_deg: f64,
    /// Latitude step between adjacent rows (degrees, must be > 0).
    pub d_lat_deg: f64,
    /// Longitude step between adjacent columns (degrees, must be > 0).
    pub d_lon_deg: f64,
    /// Row count (latitude axis), `>= 1`.
    pub n_rows: usize,
    /// Column count (longitude axis), `>= 1`.
    pub n_cols: usize,
    /// Row-major heights (m, MSL). `len() == n_rows * n_cols`.
    pub heights_m: Arc<[f64]>,
}

impl Terrain for RasterTerrain {
    fn altitude_m(&self, lat_deg: f64, lon_deg: f64) -> f64 {
        if self.n_rows == 0 || self.n_cols == 0 {
            return 0.0;
        }
        let r = (lat_deg - self.origin_lat_deg) / self.d_lat_deg;
        let c = (lon_deg - self.origin_lon_deg) / self.d_lon_deg;

        let r_max = (self.n_rows - 1) as f64;
        let c_max = (self.n_cols - 1) as f64;
        let r_clamped = r.clamp(0.0, r_max);
        let c_clamped = c.clamp(0.0, c_max);

        let r0 = r_clamped.floor() as usize;
        let c0 = c_clamped.floor() as usize;
        let r1 = (r0 + 1).min(self.n_rows - 1);
        let c1 = (c0 + 1).min(self.n_cols - 1);
        let tr = r_clamped - r0 as f64;
        let tc = c_clamped - c0 as f64;

        let h00 = self.heights_m[r0 * self.n_cols + c0];
        let h01 = self.heights_m[r0 * self.n_cols + c1];
        let h10 = self.heights_m[r1 * self.n_cols + c0];
        let h11 = self.heights_m[r1 * self.n_cols + c1];

        let h0 = h00 * (1.0 - tc) + h01 * tc;
        let h1 = h10 * (1.0 - tc) + h11 * tc;
        h0 * (1.0 - tr) + h1 * tr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_terrain_returns_constant() {
        let t = FlatTerrain::new(123.5);
        assert_eq!(t.altitude_m(35.0, 139.0), 123.5);
        assert_eq!(t.altitude_m(-89.9, 180.0), 123.5);
    }

    fn raster_2x2(h00: f64, h01: f64, h10: f64, h11: f64) -> RasterTerrain {
        RasterTerrain {
            origin_lat_deg: 35.0,
            origin_lon_deg: 139.0,
            d_lat_deg: 0.1,
            d_lon_deg: 0.1,
            n_rows: 2,
            n_cols: 2,
            heights_m: Arc::from(vec![h00, h01, h10, h11]),
        }
    }

    #[test]
    fn raster_terrain_bilinear_in_cell() {
        // Linear ramp 0 → 100 along lon at row 0, 0 → 100 along lat at col 0.
        let t = raster_2x2(0.0, 100.0, 100.0, 200.0);
        // Corner samples land exactly on grid points.
        assert!((t.altitude_m(35.0, 139.0) - 0.0).abs() < 1e-9);
        assert!((t.altitude_m(35.0, 139.1) - 100.0).abs() < 1e-9);
        assert!((t.altitude_m(35.1, 139.0) - 100.0).abs() < 1e-9);
        assert!((t.altitude_m(35.1, 139.1) - 200.0).abs() < 1e-9);
        // Centre of cell: average of all four = 100.
        assert!((t.altitude_m(35.05, 139.05) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn raster_terrain_clamps_outside() {
        let t = raster_2x2(10.0, 20.0, 30.0, 40.0);
        // Far below origin clamps to (0,0).
        assert!((t.altitude_m(0.0, 0.0) - 10.0).abs() < 1e-9);
        // Far above end clamps to (n_rows-1, n_cols-1).
        assert!((t.altitude_m(89.0, 179.0) - 40.0).abs() < 1e-9);
    }
}
