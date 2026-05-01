//! Extract `SimulationState` from JSBSim properties via `GetPropertyValue`.
//!
//! JSBSim internal units → SI conversion is done here with `uom`,
//! so unit errors are caught at compile time.
//!
//! This function is called every step (or every N steps as configured),
//! so it is kept allocation-free.

use uom::si::f64::{Acceleration, Angle, AngularVelocity, Length, Pressure, Velocity};
use uom::si::{
    acceleration::foot_per_second_squared, angle::radian, angular_velocity::radian_per_second,
    length::foot, pressure::pound_force_per_square_foot, velocity::foot_per_second,
};

use super::ffi::bridge::FDMWrapper;
use crate::output::{
    Acceleration as AccOut, AeroState, AngularRates, Attitude, Position, SimulationState,
    Velocity as VelOut,
};

/// Read the full vehicle state from JSBSim in one call.
///
/// Called after every `Run()`. Per-output decimation is applied at the
/// writer side via `SimControl::csv_sample_interval` /
/// `kml_sample_interval`, so this hot path always produces full-resolution
/// state.
pub fn extract_state(fdm: &FDMWrapper) -> SimulationState {
    // ── Position ────────────────────────────────────────────────────────
    let alt_agl_m =
        Length::new::<foot>(fdm.get_h_agl_ft()).get::<uom::si::length::meter>();

    // ── Velocity ────────────────────────────────────────────────────────
    let true_airspeed_mps =
        Velocity::new::<foot_per_second>(fdm.get_vtrue_fps())
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
    let pitch_deg = Angle::new::<radian>(fdm.get_theta_rad())
        .get::<uom::si::angle::degree>();

    let roll_deg =
        Angle::new::<radian>(fdm.get_phi_rad()).get::<uom::si::angle::degree>();

    let yaw_deg =
        Angle::new::<radian>(fdm.get_psi_rad()).get::<uom::si::angle::degree>();

    // ── Angular rates ───────────────────────────────────────────────────
    let p = AngularVelocity::new::<radian_per_second>(fdm.get_p_rad_sec())
        .get::<radian_per_second>();

    let q = AngularVelocity::new::<radian_per_second>(fdm.get_q_rad_sec())
        .get::<radian_per_second>();

    let r = AngularVelocity::new::<radian_per_second>(fdm.get_r_rad_sec())
        .get::<radian_per_second>();

    // ── Acceleration (body frame) ───────────────────────────────────────
    let ax = Acceleration::new::<foot_per_second_squared>(fdm.get_udot_ft_sec2())
        .get::<uom::si::acceleration::meter_per_second_squared>();

    let ay = Acceleration::new::<foot_per_second_squared>(fdm.get_vdot_ft_sec2())
        .get::<uom::si::acceleration::meter_per_second_squared>();

    let az = Acceleration::new::<foot_per_second_squared>(fdm.get_wdot_ft_sec2())
        .get::<uom::si::acceleration::meter_per_second_squared>();

    // ── Aerodynamics ────────────────────────────────────────────────────
    let alpha_deg =
        Angle::new::<radian>(fdm.get_alpha_rad()).get::<uom::si::angle::degree>();

    let beta_deg =
        Angle::new::<radian>(fdm.get_beta_rad()).get::<uom::si::angle::degree>();

    let qbar_pa = Pressure::new::<pound_force_per_square_foot>(fdm.get_qbar_psf())
        .get::<uom::si::pressure::pascal>();

    // ── Thrust ──────────────────────────────────────────────────────────
    let thrust_n = fdm.get_thrust_magnitude_lbf() * 4.448_221_6; // lbf → N

    SimulationState {
        time_sec: fdm.get_sim_time_sec(),
        position: Position {
            lat_deg: fdm.get_lat_gc_deg(),
            lon_deg: fdm.get_lon_gc_deg(),
            alt_agl_m,
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
        },
        thrust_n,
        mach: fdm.get_mach(),
    }
}
