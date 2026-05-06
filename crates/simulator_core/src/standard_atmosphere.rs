//! ISA 1976 standard atmosphere (dry air) in SI units.
//!
//! Inputs are geometric altitude above sea level in meters.
//! Outputs are temperature (K), pressure (Pa), and density (kg/m^3).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtmosphereSample {
    pub temperature_k: f64,
    pub pressure_pa: f64,
    pub density_kg_m3: f64,
    pub geopotential_alt_m: f64,
    pub sound_speed: f64,
}

const EARTH_RADIUS_M: f64 = 6_356_766.0; // ISA 1976 reference radius
const G0_MPS2: f64 = 9.806_65;
const R_DRY_AIR: f64 = 287.052_87;

const SPECIFIC_HEAT_RATIO: f64 = 1.4;

const SEA_LEVEL_PRESSURE_PA: f64 = 101_325.0;

const MAX_GEOMETRIC_ALT_M: f64 = 86_000.0;

const LAYER_BASE_GEO_H_M: [f64; 8] = [
    0.0, 11_000.0, 20_000.0, 32_000.0, 47_000.0, 51_000.0, 71_000.0, 84_852.0,
];

const LAYER_BASE_TEMP_K: [f64; 8] = [
    288.15, 216.65, 216.65, 228.65, 270.65, 270.65, 214.65, 186.946,
];

const LAYER_LAPSE_K_PER_M: [f64; 7] = [-0.0065, 0.0, 0.0010, 0.0028, 0.0, -0.0028, -0.0020];

/// Sample the ISA 1976 standard atmosphere at a geometric altitude.
///
/// Altitudes below sea level are clamped to 0 m. Altitudes above 86 km are
/// clamped to 86 km, which is the ISA validity range for this layer model.
pub fn sample_atmosphere(alt_m: f64) -> AtmosphereSample {
    let alt_m = alt_m.max(0.0).min(MAX_GEOMETRIC_ALT_M);
    let geopot_m = geopotential_altitude(alt_m);

    let layer = layer_index(geopot_m);
    let (hb, tb, lb, pb) = layer_base_state(layer);

    let (temperature_k, pressure_pa) = temperature_pressure_at(geopot_m, hb, tb, lb, pb);
    let density_kg_m3 = pressure_pa / (R_DRY_AIR * temperature_k);

    let sound_speed = (temperature_k * R_DRY_AIR * SPECIFIC_HEAT_RATIO).sqrt();

    AtmosphereSample {
        temperature_k,
        pressure_pa,
        density_kg_m3,
        geopotential_alt_m: geopot_m,
        sound_speed,
    }
}

#[inline]
fn geopotential_altitude(geometric_alt_m: f64) -> f64 {
    (EARTH_RADIUS_M * geometric_alt_m) / (EARTH_RADIUS_M + geometric_alt_m)
}

#[inline]
fn layer_index(geopot_m: f64) -> usize {
    for i in 0..(LAYER_BASE_GEO_H_M.len() - 1) {
        if geopot_m < LAYER_BASE_GEO_H_M[i + 1] {
            return i;
        }
    }
    LAYER_BASE_GEO_H_M.len() - 2
}

#[inline]
fn layer_base_state(layer: usize) -> (f64, f64, f64, f64) {
    let mut pb = SEA_LEVEL_PRESSURE_PA;

    for i in 0..layer {
        let hb = LAYER_BASE_GEO_H_M[i];
        let tb = LAYER_BASE_TEMP_K[i];
        let lb = LAYER_LAPSE_K_PER_M[i];
        let h_next = LAYER_BASE_GEO_H_M[i + 1];
        pb = pressure_at(h_next, hb, tb, lb, pb);
    }

    let hb = LAYER_BASE_GEO_H_M[layer];
    let tb = LAYER_BASE_TEMP_K[layer];
    let lb = LAYER_LAPSE_K_PER_M[layer];

    (hb, tb, lb, pb)
}

#[inline]
fn temperature_pressure_at(h: f64, hb: f64, tb: f64, lb: f64, pb: f64) -> (f64, f64) {
    if lb.abs() < f64::EPSILON {
        let temperature_k = tb;
        let pressure_pa = pb * (-G0_MPS2 * (h - hb) / (R_DRY_AIR * tb)).exp();
        (temperature_k, pressure_pa)
    } else {
        let temperature_k = tb + lb * (h - hb);
        let exponent = G0_MPS2 / (R_DRY_AIR * lb);
        let pressure_pa = pb * (tb / temperature_k).powf(exponent);
        (temperature_k, pressure_pa)
    }
}

#[inline]
fn pressure_at(h: f64, hb: f64, tb: f64, lb: f64, pb: f64) -> f64 {
    temperature_pressure_at(h, hb, tb, lb, pb).1
}

#[cfg(test)]
mod tests {
    use super::{sample_atmosphere, AtmosphereSample};

    fn assert_close(label: &str, value: f64, expected: f64, tol: f64) {
        let delta = (value - expected).abs();
        assert!(
            delta <= tol,
            "{label} expected {expected}, got {value} (delta {delta})"
        );
    }

    fn assert_sample(sample: AtmosphereSample, t: f64, p: f64, rho: f64) {
        assert_close("temperature", sample.temperature_k, t, 1e-2);
        assert_close("pressure", sample.pressure_pa, p, 3.0);
        assert_close("density", sample.density_kg_m3, rho, 5e-4);
    }

    fn geometric_altitude_from_geopotential(geopotential_alt_m: f64) -> f64 {
        (super::EARTH_RADIUS_M * geopotential_alt_m) / (super::EARTH_RADIUS_M - geopotential_alt_m)
    }

    #[test]
    fn sea_level() {
        let sample = sample_atmosphere(0.0);
        assert_sample(sample, 288.15, 101_325.0, 1.225);
    }

    #[test]
    fn eleven_km() {
        let alt_m = geometric_altitude_from_geopotential(11_000.0);
        let sample = sample_atmosphere(alt_m);
        assert_sample(sample, 216.65, 22_632.1, 0.36391);
    }

    #[test]
    fn twenty_km() {
        let alt_m = geometric_altitude_from_geopotential(20_000.0);
        let sample = sample_atmosphere(alt_m);
        assert_sample(sample, 216.65, 5_474.89, 0.08803);
    }

    #[test]
    fn thirty_two_km() {
        let alt_m = geometric_altitude_from_geopotential(32_000.0);
        let sample = sample_atmosphere(alt_m);
        assert_sample(sample, 228.65, 868.02, 0.01322);
    }
}
