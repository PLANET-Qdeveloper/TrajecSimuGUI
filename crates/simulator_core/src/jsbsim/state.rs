//! Extract `SimulationState` from JSBSim properties via `GetPropertyValue`.
//!
//! JSBSim internal units → SI conversion is done here with `uom`,
//! so unit errors are caught at compile time.
//!
//! This function is called every step (or every N steps as configured),
//! so it is kept allocation-free.

use uom::si::f64::{
    Acceleration, Angle, AngularVelocity, Length, Pressure, Velocity,
};
use uom::si::{
    acceleration::foot_per_second_squared,
    angle::radian,
    angular_velocity::radian_per_second,
    length::foot,
    pressure::pound_force_per_square_foot,
    velocity::foot_per_second,
};

use crate::output::{
    Acceleration as AccOut, AeroState, AngularRates, Attitude, Position,
    SimulationState, Velocity as VelOut,
};
use super::ffi::ffi::FDMWrapper;

/// Read the full vehicle state from JSBSim in one call.
///
/// Called after every `Run()` (or every `state_sample_interval` steps).
pub fn extract_state(fdm: &FDMWrapper) -> SimulationState {
    // ── Position ────────────────────────────────────────────────────────
    let alt_agl_m = Length::new::<foot>(
        fdm.get_property("position/h-agl-ft"),
    )
    .get::<uom::si::length::meter>();

    // ── Velocity ────────────────────────────────────────────────────────
    let true_airspeed_mps = Velocity::new::<foot_per_second>(
        fdm.get_property("velocities/vtrue-fps"),
    )
    .get::<uom::si::velocity::meter_per_second>();

    let ground_speed_mps = Velocity::new::<foot_per_second>(
        fdm.get_property("velocities/vg-fps"),
    )
    .get::<uom::si::velocity::meter_per_second>();

    // ── Attitude ────────────────────────────────────────────────────────
    let pitch_deg = Angle::new::<radian>(
        fdm.get_property("attitude/theta-rad"),
    )
    .get::<uom::si::angle::degree>();

    let roll_deg = Angle::new::<radian>(
        fdm.get_property("attitude/phi-rad"),
    )
    .get::<uom::si::angle::degree>();

    let yaw_deg = Angle::new::<radian>(
        fdm.get_property("attitude/psi-rad"),
    )
    .get::<uom::si::angle::degree>();

    // ── Angular rates ───────────────────────────────────────────────────
    let p = AngularVelocity::new::<radian_per_second>(
        fdm.get_property("velocities/p-rad_sec"),
    )
    .get::<radian_per_second>();

    let q = AngularVelocity::new::<radian_per_second>(
        fdm.get_property("velocities/q-rad_sec"),
    )
    .get::<radian_per_second>();

    let r = AngularVelocity::new::<radian_per_second>(
        fdm.get_property("velocities/r-rad_sec"),
    )
    .get::<radian_per_second>();

    // ── Acceleration (body frame) ───────────────────────────────────────
    let ax = Acceleration::new::<foot_per_second_squared>(
        fdm.get_property("accelerations/udot-ft_sec2"),
    )
    .get::<uom::si::acceleration::meter_per_second_squared>();

    let ay = Acceleration::new::<foot_per_second_squared>(
        fdm.get_property("accelerations/vdot-ft_sec2"),
    )
    .get::<uom::si::acceleration::meter_per_second_squared>();

    let az = Acceleration::new::<foot_per_second_squared>(
        fdm.get_property("accelerations/wdot-ft_sec2"),
    )
    .get::<uom::si::acceleration::meter_per_second_squared>();

    // ── Aerodynamics ────────────────────────────────────────────────────
    let alpha_deg = Angle::new::<radian>(
        fdm.get_property("aero/alpha-rad"),
    )
    .get::<uom::si::angle::degree>();

    let beta_deg = Angle::new::<radian>(
        fdm.get_property("aero/beta-rad"),
    )
    .get::<uom::si::angle::degree>();

    let qbar_pa = Pressure::new::<pound_force_per_square_foot>(
        fdm.get_property("aero/qbar-psf"),
    )
    .get::<uom::si::pressure::pascal>();

    // ── Thrust ──────────────────────────────────────────────────────────
    // JSBSim stores thrust magnitude in lbf.
    let thrust_n = fdm.get_property("external_reactions/thrust/magnitude")
        * 4.448_221_6; // lbf → N

    SimulationState {
        time_sec: fdm.get_property("simulation/sim-time-sec"),
        position: Position {
            lat_deg: fdm.get_property("position/lat-gc-deg"),
            lon_deg: fdm.get_property("position/long-gc-deg"),
            alt_agl_m,
        },
        velocity: VelOut { true_airspeed_mps, ground_speed_mps },
        attitude: Attitude { pitch_deg, roll_deg, yaw_deg },
        angular_rates: AngularRates {
            p_rad_sec: p,
            q_rad_sec: q,
            r_rad_sec: r,
        },
        acceleration: AccOut { ax_mps2: ax, ay_mps2: ay, az_mps2: az },
        aero: AeroState { alpha_deg, beta_deg, qbar_pa },
        thrust_n,
        mach: fdm.get_property("velocities/mach"),
    }
}
