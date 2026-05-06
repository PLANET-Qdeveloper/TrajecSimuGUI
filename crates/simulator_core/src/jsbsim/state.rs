//! Extract `SimulationState` from JSBSim properties via `GetPropertyValue`.
//!
//! JSBSim internal units → SI conversion is done here with `uom`,
//! so unit errors are caught at compile time.
//!
//! This function is called every step (or every N steps as configured),
//! so it is kept allocation-free.

use uom::si::f64::{
    Acceleration, Angle, AngularVelocity, Length, Pressure, ThermodynamicTemperature, Velocity,
};
use uom::si::{
    acceleration::foot_per_second_squared, angle::radian, angular_velocity::radian_per_second,
    length::foot, pressure::pound_force_per_square_foot, thermodynamic_temperature::degree_rankine,
    velocity::foot_per_second,
};

use super::ffi::bridge::FDMWrapper;
use crate::output::{
    Acceleration as AccOut, AeroState, AngularRates, Attitude, Position, SimulationState,
    Velocity as VelOut,
};
use crate::simple_simulator::env::latlon_to_local;

/// Read the full vehicle state from JSBSim in one call.
///
/// Called after every `Run()`. Per-output decimation is applied at the
/// writer side via `SimControl::csv_sample_interval` /
/// `kml_sample_interval`, so this hot path always produces full-resolution
/// state.
pub fn extract_state(
    fdm: &FDMWrapper,
    launch_lat_deg: f64,
    launch_lon_deg: f64,
    launch_yaw_deg: f64,
) -> SimulationState {
    // ── Position ────────────────────────────────────────────────────────
    let alt_agl_m = Length::new::<foot>(fdm.get_h_agl_ft()).get::<uom::si::length::meter>();

    // ── Velocity ────────────────────────────────────────────────────────
    let true_airspeed_mps = Velocity::new::<foot_per_second>(fdm.get_vtrue_fps())
        .get::<uom::si::velocity::meter_per_second>();

    let ground_speed_mps = Velocity::new::<foot_per_second>(fdm.get_vg_fps())
        .get::<uom::si::velocity::meter_per_second>();

    // Body-axis velocity components (u/v/w).
    let u_mps = Velocity::new::<foot_per_second>(fdm.get_u_fps())
        .get::<uom::si::velocity::meter_per_second>();

    let v_mps = Velocity::new::<foot_per_second>(fdm.get_v_fps())
        .get::<uom::si::velocity::meter_per_second>();

    let w_mps = Velocity::new::<foot_per_second>(fdm.get_w_fps())
        .get::<uom::si::velocity::meter_per_second>();

    // ── Attitude ────────────────────────────────────────────────────────
    let pitch_deg = Angle::new::<radian>(fdm.get_theta_rad()).get::<uom::si::angle::degree>();

    let roll_deg = Angle::new::<radian>(fdm.get_phi_rad()).get::<uom::si::angle::degree>();

    let yaw_deg = Angle::new::<radian>(fdm.get_psi_rad()).get::<uom::si::angle::degree>();

    // ── Angular rates ───────────────────────────────────────────────────
    let p =
        AngularVelocity::new::<radian_per_second>(fdm.get_p_rad_sec()).get::<radian_per_second>();

    let q =
        AngularVelocity::new::<radian_per_second>(fdm.get_q_rad_sec()).get::<radian_per_second>();

    let r =
        AngularVelocity::new::<radian_per_second>(fdm.get_r_rad_sec()).get::<radian_per_second>();

    // ── Acceleration (body frame) ───────────────────────────────────────
    let ax = Acceleration::new::<foot_per_second_squared>(fdm.get_udot_ft_sec2())
        .get::<uom::si::acceleration::meter_per_second_squared>();

    let ay = Acceleration::new::<foot_per_second_squared>(fdm.get_vdot_ft_sec2())
        .get::<uom::si::acceleration::meter_per_second_squared>();

    let az = Acceleration::new::<foot_per_second_squared>(fdm.get_wdot_ft_sec2())
        .get::<uom::si::acceleration::meter_per_second_squared>();

    // ── Aerodynamics ────────────────────────────────────────────────────
    let alpha_deg = Angle::new::<radian>(fdm.get_alpha_rad()).get::<uom::si::angle::degree>();

    let beta_deg = Angle::new::<radian>(fdm.get_beta_rad()).get::<uom::si::angle::degree>();

    let qbar_pa = Pressure::new::<pound_force_per_square_foot>(fdm.get_qbar_psf())
        .get::<uom::si::pressure::pascal>();

    // Total AoA: αt = arctan(√(v²+w²) / |u|)
    let total_aoa_deg = (v_mps.powi(2) + w_mps.powi(2))
        .sqrt()
        .atan2(u_mps.abs())
        .to_degrees();

    // Atmosphere
    let pressure_pa = Pressure::new::<pound_force_per_square_foot>(fdm.get_pressure_psf())
        .get::<uom::si::pressure::pascal>();

    let temperature_k =
        ThermodynamicTemperature::new::<degree_rankine>(fdm.get_temperature_rankine())
            .get::<uom::si::thermodynamic_temperature::kelvin>();

    // Gust (fixed 9 m/s in the pitch-axis / v direction)
    const V_GUST_MPS: f64 = 9.0;
    let gust_airspeed_mps = (true_airspeed_mps.powi(2) + V_GUST_MPS.powi(2)).sqrt();
    let gust_aoa_deg = ((v_mps + V_GUST_MPS).powi(2) + w_mps.powi(2))
        .sqrt()
        .atan2(u_mps.abs())
        .to_degrees();

    // ── Launch-local position ────────────────────────────────────────────
    let lat_deg = fdm.get_lat_gc_deg();
    let lon_deg = fdm.get_lon_gc_deg();
    let (down_range_m, local_x_m, local_y_m) = latlon_to_local(
        lat_deg,
        lon_deg,
        launch_lat_deg,
        launch_lon_deg,
        launch_yaw_deg,
    );

    // ── Thrust ──────────────────────────────────────────────────────────
    let thrust_n = fdm.get_thrust_magnitude_lbf() * 4.448_221_6; // lbf → N

    SimulationState {
        time_sec: fdm.get_sim_time_sec(),
        position: Position {
            lat_deg,
            lon_deg,
            alt_agl_m,
            down_range_m,
            local_x_m,
            local_y_m,
        },
        velocity: VelOut {
            true_airspeed_mps,
            ground_speed_mps,
            u_mps,
            v_mps,
            w_mps,
        },
        attitude: Attitude {
            pitch_deg,
            roll_deg,
            yaw_deg,
        },
        angular_rates: AngularRates {
            p_rad_sec: p,
            q_rad_sec: q,
            r_rad_sec: r,
        },
        acceleration: AccOut {
            ax_mps2: ax,
            ay_mps2: ay,
            az_mps2: az,
        },
        aero: AeroState {
            alpha_deg,
            beta_deg,
            qbar_pa,
            total_aoa_deg,
            pressure_pa,
            temperature_k,
            gust_airspeed_mps,
            gust_aoa_deg,
        },
        thrust_n,
        mach: fdm.get_mach(),
    }
}
