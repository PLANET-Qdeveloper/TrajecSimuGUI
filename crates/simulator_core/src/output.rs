//! Simulation output data structures.
//!
//! All values are in SI units (m, kg, N, Pa, deg, m/s, m/s², rad/s)
//! regardless of which backend produced them.

use serde::{Deserialize, Serialize};

// ─── Per-step state ────────────────────────────────────────────────────────

/// Geographic position of the vehicle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Position {
    /// Geocentric latitude (degrees north).
    pub lat_deg: f64,
    /// Longitude (degrees east).
    pub lon_deg: f64,
    /// Altitude above ground level (m).
    pub alt_agl_m: f64,
}

/// Velocity in the wind / body frame.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Velocity {
    /// True airspeed magnitude (m/s).
    pub true_airspeed_mps: f64,
    /// Ground-track speed magnitude (m/s).
    pub ground_speed_mps: f64,
    /// Body-axis forward velocity along the nose (m/s).
    /// Maps to JSBSim `velocities/u-fps`.
    pub u_mps: f64,
    /// Body-axis lateral (right-wing) velocity (m/s).
    /// Maps to JSBSim `velocities/v-fps`.
    pub v_mps: f64,
    /// Body-axis downward velocity (m/s).
    /// Maps to JSBSim `velocities/w-fps`.
    pub w_mps: f64,
}

/// Euler attitude angles.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Attitude {
    /// Pitch (θ), degrees. 90 = vertical.
    pub pitch_deg: f64,
    /// Roll (φ), degrees.
    pub roll_deg: f64,
    /// Yaw / heading (ψ), degrees.
    pub yaw_deg: f64,
}

/// Body-axis angular rates (rad/s).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AngularRates {
    /// Roll rate p.
    pub p_rad_sec: f64,
    /// Pitch rate q.
    pub q_rad_sec: f64,
    /// Yaw rate r.
    pub r_rad_sec: f64,
}

/// Body-axis linear accelerations (m/s²).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Acceleration {
    pub ax_mps2: f64,
    pub ay_mps2: f64,
    pub az_mps2: f64,
}

/// Aerodynamic state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AeroState {
    /// Angle of attack (degrees).
    pub alpha_deg: f64,
    /// Sideslip angle (degrees).
    pub beta_deg: f64,
    /// Dynamic pressure (Pa).
    pub qbar_pa: f64,
}

/// Complete vehicle state at one time step.
///
/// Filled by `Simulator::get_state()` after each `step()` call.
/// JSBSim backend: populated via `GetPropertyValue`.
/// Custom backend: populated from integrated state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SimulationState {
    /// Simulation time (s).
    pub time_sec: f64,
    pub position: Position,
    pub velocity: Velocity,
    pub attitude: Attitude,
    pub angular_rates: AngularRates,
    pub acceleration: Acceleration,
    pub aero: AeroState,
    /// Current thrust (N).
    pub thrust_n: f64,
    /// Mach number.
    pub mach: f64,
}

// ─── Trajectory history ────────────────────────────────────────────────────

/// Accumulated trajectory and statistics from a completed simulation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationOutput {
    /// State recorded at each output step.
    pub trajectory: Vec<SimulationState>,
    /// Peak altitude AGL (m).
    pub max_altitude_m: f64,
    /// Maximum speed (m/s).
    pub max_speed_mps: f64,
    /// Total flight duration (s).
    pub flight_time_sec: f64,
}

impl SimulationOutput {
    pub fn new() -> Self {
        Self {
            trajectory: Vec::new(),
            max_altitude_m: 0.0,
            max_speed_mps: 0.0,
            flight_time_sec: 0.0,
        }
    }

    /// Push a state and update running statistics.
    pub fn push(&mut self, state: SimulationState) {
        self.max_altitude_m = self.max_altitude_m.max(state.position.alt_agl_m);
        self.max_speed_mps = self.max_speed_mps.max(state.velocity.true_airspeed_mps);
        self.flight_time_sec = state.time_sec;
        self.trajectory.push(state);
    }
}

impl Default for SimulationOutput {
    fn default() -> Self {
        Self::new()
    }
}
