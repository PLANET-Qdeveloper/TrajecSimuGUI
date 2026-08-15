//! Pressure -> altitude conversion for live telemetry display.
//!
//! The wire protocol does not document the pressure sensor's unit or a
//! reference sea-level pressure (`ref/NSE2026_PROTOCOL.md`: "気圧、単位は
//! 現行コードで明文化されていない"), so callers pass both in explicitly
//! (configured per serial port slot in the UI) rather than this module
//! guessing. Only the ISA troposphere (<11 km) formula is implemented,
//! which is sufficient for sounding-rocket flight altitudes.

/// ISA troposphere pressure altitude, in meters, given a station pressure
/// and a reference sea-level pressure (same unit for both).
///
/// The lapse rate and sea-level temperature come from
/// `simulator_core::standard_atmosphere`, which also backs the simulator's
/// forward (altitude -> pressure) model — sharing the constants keeps the
/// live-telemetry altitude readout and the simulated trajectory from
/// silently drifting apart if the ISA constants are ever revised.
pub fn pressure_to_altitude_m(pressure: f64, sea_level_pressure: f64) -> f64 {
    use simulator_core::standard_atmosphere::{
        TROPOSPHERE_LAPSE_RATE_K_PER_M, TROPOSPHERE_SEA_LEVEL_TEMP_K,
    };
    const GAS_EXPONENT: f64 = 1.0 / 5.255; // R*L/(g*M) inverse, standard barometric exponent

    if pressure <= 0.0 || sea_level_pressure <= 0.0 {
        return f64::NAN;
    }
    let ratio = pressure / sea_level_pressure;
    (TROPOSPHERE_SEA_LEVEL_TEMP_K / TROPOSPHERE_LAPSE_RATE_K_PER_M)
        * (1.0 - ratio.powf(GAS_EXPONENT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sea_level_pressure_gives_zero_altitude() {
        let alt = pressure_to_altitude_m(101325.0, 101325.0);
        assert!(alt.abs() < 1e-6);
    }

    #[test]
    fn known_standard_atmosphere_point() {
        // ISA: 1000 m altitude corresponds to ~89874.6 Pa at sea-level 101325 Pa.
        let alt = pressure_to_altitude_m(89_874.6, 101_325.0);
        assert!((alt - 1000.0).abs() < 1.0, "alt = {alt}");
    }

    #[test]
    fn invalid_pressure_yields_nan() {
        assert!(pressure_to_altitude_m(0.0, 101325.0).is_nan());
    }
}
