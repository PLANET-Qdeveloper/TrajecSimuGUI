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
    pub alt_msl_m: f64,
    /// Horizontal distance from the launch site (m).
    pub down_range_m: f64,
    /// Distance along the launch yaw direction (m), positive forward.
    pub local_x_m: f64,
    /// Distance perpendicular to the launch yaw direction (m), positive to the right.
    pub local_y_m: f64,
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
    /// Total angle of attack αt = arctan(√(v²+w²) / |u|) (degrees).
    pub total_aoa_deg: f64,
    /// Atmospheric pressure (Pa).
    pub pressure_pa: f64,
    /// Atmospheric temperature (K).
    pub temperature_k: f64,
    /// Airspeed under 9 m/s lateral gust = √(vtrue² + 9²) (m/s).
    pub gust_airspeed_mps: f64,
    /// AoA under 9 m/s lateral gust added to v-component (degrees).
    pub gust_aoa_deg: f64,
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

// ─── Trajectory (SoA) ─────────────────────────────────────────────────────

/// Column-oriented (SoA) trajectory storage.
///
/// Each field is a `Vec<f64>` of length *N* (one entry per recorded step).
/// Use [`Trajectory::push`] to append, [`Trajectory::get_state`] or
/// [`Trajectory::row_iter`] to reconstruct per-step [`SimulationState`] values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Trajectory {
    pub time_sec: Vec<f64>,
    // ── Position ────────────────────────────────────────────────────────────
    pub lat_deg: Vec<f64>,
    pub lon_deg: Vec<f64>,
    pub alt_msl_m: Vec<f64>,
    pub down_range_m: Vec<f64>,
    pub local_x_m: Vec<f64>,
    pub local_y_m: Vec<f64>,
    // ── Velocity ────────────────────────────────────────────────────────────
    pub u_mps: Vec<f64>,
    pub v_mps: Vec<f64>,
    pub w_mps: Vec<f64>,
    pub true_airspeed_mps: Vec<f64>,
    pub ground_speed_mps: Vec<f64>,
    // ── Attitude ────────────────────────────────────────────────────────────
    pub pitch_deg: Vec<f64>,
    pub roll_deg: Vec<f64>,
    pub yaw_deg: Vec<f64>,
    // ── Angular rates ────────────────────────────────────────────────────────
    pub p_rad_sec: Vec<f64>,
    pub q_rad_sec: Vec<f64>,
    pub r_rad_sec: Vec<f64>,
    // ── Acceleration ─────────────────────────────────────────────────────────
    pub ax_mps2: Vec<f64>,
    pub ay_mps2: Vec<f64>,
    pub az_mps2: Vec<f64>,
    // ── Aerodynamics / atmosphere ────────────────────────────────────────────
    pub alpha_deg: Vec<f64>,
    pub beta_deg: Vec<f64>,
    pub qbar_pa: Vec<f64>,
    pub total_aoa_deg: Vec<f64>,
    pub pressure_pa: Vec<f64>,
    pub temperature_k: Vec<f64>,
    pub gust_airspeed_mps: Vec<f64>,
    pub gust_aoa_deg: Vec<f64>,
    // ── Propulsion ───────────────────────────────────────────────────────────
    pub thrust_n: Vec<f64>,
    pub mach: Vec<f64>,
}

impl Trajectory {
    pub fn len(&self) -> usize {
        self.time_sec.len()
    }

    pub fn is_empty(&self) -> bool {
        self.time_sec.is_empty()
    }

    /// Append one step from a [`SimulationState`].
    pub fn push(&mut self, s: &SimulationState) {
        self.time_sec.push(s.time_sec);
        self.lat_deg.push(s.position.lat_deg);
        self.lon_deg.push(s.position.lon_deg);
        self.alt_msl_m.push(s.position.alt_msl_m);
        self.down_range_m.push(s.position.down_range_m);
        self.local_x_m.push(s.position.local_x_m);
        self.local_y_m.push(s.position.local_y_m);
        self.u_mps.push(s.velocity.u_mps);
        self.v_mps.push(s.velocity.v_mps);
        self.w_mps.push(s.velocity.w_mps);
        self.true_airspeed_mps.push(s.velocity.true_airspeed_mps);
        self.ground_speed_mps.push(s.velocity.ground_speed_mps);
        self.pitch_deg.push(s.attitude.pitch_deg);
        self.roll_deg.push(s.attitude.roll_deg);
        self.yaw_deg.push(s.attitude.yaw_deg);
        self.p_rad_sec.push(s.angular_rates.p_rad_sec);
        self.q_rad_sec.push(s.angular_rates.q_rad_sec);
        self.r_rad_sec.push(s.angular_rates.r_rad_sec);
        self.ax_mps2.push(s.acceleration.ax_mps2);
        self.ay_mps2.push(s.acceleration.ay_mps2);
        self.az_mps2.push(s.acceleration.az_mps2);
        self.alpha_deg.push(s.aero.alpha_deg);
        self.beta_deg.push(s.aero.beta_deg);
        self.qbar_pa.push(s.aero.qbar_pa);
        self.total_aoa_deg.push(s.aero.total_aoa_deg);
        self.pressure_pa.push(s.aero.pressure_pa);
        self.temperature_k.push(s.aero.temperature_k);
        self.gust_airspeed_mps.push(s.aero.gust_airspeed_mps);
        self.gust_aoa_deg.push(s.aero.gust_aoa_deg);
        self.thrust_n.push(s.thrust_n);
        self.mach.push(s.mach);
    }

    pub fn truncate(&mut self, len: usize) {
        self.time_sec.truncate(len);
        self.lat_deg.truncate(len);
        self.lon_deg.truncate(len);
        self.alt_msl_m.truncate(len);
        self.down_range_m.truncate(len);
        self.local_x_m.truncate(len);
        self.local_y_m.truncate(len);
        self.u_mps.truncate(len);
        self.v_mps.truncate(len);
        self.w_mps.truncate(len);
        self.true_airspeed_mps.truncate(len);
        self.ground_speed_mps.truncate(len);
        self.pitch_deg.truncate(len);
        self.roll_deg.truncate(len);
        self.yaw_deg.truncate(len);
        self.p_rad_sec.truncate(len);
        self.q_rad_sec.truncate(len);
        self.r_rad_sec.truncate(len);
        self.ax_mps2.truncate(len);
        self.ay_mps2.truncate(len);
        self.az_mps2.truncate(len);
        self.alpha_deg.truncate(len);
        self.beta_deg.truncate(len);
        self.qbar_pa.truncate(len);
        self.total_aoa_deg.truncate(len);
        self.pressure_pa.truncate(len);
        self.temperature_k.truncate(len);
        self.gust_airspeed_mps.truncate(len);
        self.gust_aoa_deg.truncate(len);
        self.thrust_n.truncate(len);
        self.mach.truncate(len);
    }

    /// Reconstruct a [`SimulationState`] from row index `i`.
    pub fn get_state(&self, i: usize) -> SimulationState {
        SimulationState {
            time_sec: self.time_sec[i],
            position: Position {
                lat_deg: self.lat_deg[i],
                lon_deg: self.lon_deg[i],
                alt_msl_m: self.alt_msl_m[i],
                down_range_m: self.down_range_m[i],
                local_x_m: self.local_x_m[i],
                local_y_m: self.local_y_m[i],
            },
            velocity: Velocity {
                u_mps: self.u_mps[i],
                v_mps: self.v_mps[i],
                w_mps: self.w_mps[i],
                true_airspeed_mps: self.true_airspeed_mps[i],
                ground_speed_mps: self.ground_speed_mps[i],
            },
            attitude: Attitude {
                pitch_deg: self.pitch_deg[i],
                roll_deg: self.roll_deg[i],
                yaw_deg: self.yaw_deg[i],
            },
            angular_rates: AngularRates {
                p_rad_sec: self.p_rad_sec[i],
                q_rad_sec: self.q_rad_sec[i],
                r_rad_sec: self.r_rad_sec[i],
            },
            acceleration: Acceleration {
                ax_mps2: self.ax_mps2[i],
                ay_mps2: self.ay_mps2[i],
                az_mps2: self.az_mps2[i],
            },
            aero: AeroState {
                alpha_deg: self.alpha_deg[i],
                beta_deg: self.beta_deg[i],
                qbar_pa: self.qbar_pa[i],
                total_aoa_deg: self.total_aoa_deg[i],
                pressure_pa: self.pressure_pa[i],
                temperature_k: self.temperature_k[i],
                gust_airspeed_mps: self.gust_airspeed_mps[i],
                gust_aoa_deg: self.gust_aoa_deg[i],
            },
            thrust_n: self.thrust_n[i],
            mach: self.mach[i],
        }
    }

    /// Return the last row as a [`SimulationState`], or `None` if empty.
    pub fn last_state(&self) -> Option<SimulationState> {
        if self.is_empty() {
            None
        } else {
            Some(self.get_state(self.len() - 1))
        }
    }

    /// Iterate over all rows, yielding reconstructed [`SimulationState`] values.
    pub fn row_iter(&self) -> impl Iterator<Item = SimulationState> + '_ {
        (0..self.len()).map(|i| self.get_state(i))
    }
}

// ─── Trajectory history ────────────────────────────────────────────────────

/// Accumulated trajectory and statistics from a completed simulation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationOutput {
    /// State recorded at each output step (SoA layout).
    pub trajectory: Trajectory,
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
            trajectory: Trajectory::default(),
            max_altitude_m: 0.0,
            max_speed_mps: 0.0,
            flight_time_sec: 0.0,
        }
    }

    /// Push a state and update running statistics.
    pub fn push(&mut self, state: SimulationState) {
        self.max_altitude_m = self.max_altitude_m.max(state.position.alt_msl_m);
        self.max_speed_mps = self.max_speed_mps.max(state.velocity.true_airspeed_mps);
        self.flight_time_sec = state.time_sec;
        self.trajectory.push(&state);
    }
}

impl Default for SimulationOutput {
    fn default() -> Self {
        Self::new()
    }
}
