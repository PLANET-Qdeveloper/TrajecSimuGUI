//! Simulation progress tracking and event system
//!
//! Provides event-driven progress reporting with snapshots and milestones.
//! Events flow through tokio channels for async distribution to CLI/GUI.

use serde::{Deserialize, Serialize};
use std::time::Instant;
use crate::output::SimulationState;

/// Milestone event kinds during rocket flight
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MilestoneKind {
    /// Motor ignition
    Ignition,
    /// Vehicle liftoff
    Liftoff,
    /// Motor burnout
    BurnOut,
    /// Maximum altitude reached
    Apogee,
    /// Parachute deployment
    ParachuteDeployed,
    /// Landing/impact
    Landing,
}

impl std::fmt::Display for MilestoneKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ignition => write!(f, "Ignition"),
            Self::Liftoff => write!(f, "Liftoff"),
            Self::BurnOut => write!(f, "Burnout"),
            Self::Apogee => write!(f, "Apogee"),
            Self::ParachuteDeployed => write!(f, "Parachute Deployed"),
            Self::Landing => write!(f, "Landing"),
        }
    }
}

/// Snapshot of progress at a given moment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressSnapshot {
    /// Simulation time (s)
    pub sim_time: f64,
    
    /// Progress ratio: [0.0, 1.0]
    pub progress_ratio: f64,
    
    /// Current altitude (m)
    pub altitude: f64,
    
    /// Downrange distance (m)
    pub downrange: f64,
    
    /// Current velocity magnitude (m/s)
    pub velocity_mag: f64,
    
    /// Condensed state for visualization
    pub state_summary: StateSnapshot,
    
    /// Wall clock elapsed time (s)
    pub wall_time_elapsed: f64,
}

/// Minimal state snapshot for progress display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub position: [f64; 3],      // [x, y, z]
    pub velocity: [f64; 3],      // [vx, vy, vz]
    pub pitch: f64,              // degrees
    pub roll: f64,               // degrees
    pub yaw: f64,                // degrees
    pub pitch_rate: f64,         // deg/s
    pub roll_rate: f64,          // deg/s
    pub yaw_rate: f64,           // deg/s
    pub accel_mag: f64,          // m/s²
}

impl StateSnapshot {
    /// Build from full SimulationState
    pub fn from_state(state: &SimulationState) -> Self {
        let accel_mag = (state.acceleration.ax_mps2.powi(2)
            + state.acceleration.ay_mps2.powi(2)
            + state.acceleration.az_mps2.powi(2))
            .sqrt();

        Self {
            position: [
                state.position.lat_deg,
                state.position.lon_deg,
                state.position.alt_agl_m,
            ],
            velocity: [
                state.velocity.true_airspeed_mps,
                state.velocity.ground_speed_mps,
                0.0,
            ],
            pitch: state.attitude.pitch_deg,
            roll: state.attitude.roll_deg,
            yaw: state.attitude.yaw_deg,
            pitch_rate: state.angular_rates.q_rad_sec,
            roll_rate: state.angular_rates.p_rad_sec,
            yaw_rate: state.angular_rates.r_rad_sec,
            accel_mag,
        }
    }
}

/// Milestone event with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub kind: MilestoneKind,
    pub sim_time: f64,
    pub altitude: f64,
    pub velocity_mag: f64,
}

/// Summary of completed simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationSummary {
    pub total_time: f64,
    pub max_altitude: f64,
    pub max_downrange: f64,
    pub max_velocity: f64,
    pub milestone_count: usize,
    pub wall_time_elapsed: f64,
}

/// Simulation event for distribution to listeners
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimulationEvent {
    /// Simulation started
    Started {
        max_time: f64,
        max_altitude_expected: f64,
    },
    
    /// Regular progress update (500ms or configurable interval)
    Progress(ProgressSnapshot),
    
    /// Significant milestone reached
    Milestone(Milestone),
    
    /// Simulation completed successfully
    Completed(SimulationSummary),
    
    /// Simulation failed with error
    Failed {
        message: String,
        sim_time: f64,
    },
    
    /// User-initiated cancellation
    Cancelled {
        sim_time: f64,
    },
}

impl SimulationEvent {
    pub fn is_terminal(&self) -> bool {
        matches!(self, SimulationEvent::Completed(_) | SimulationEvent::Failed { .. } | SimulationEvent::Cancelled { .. })
    }
}

/// Event broadcaster (wrapper around tokio::sync::broadcast)
pub struct EventBroadcaster {
    tx: tokio::sync::broadcast::Sender<SimulationEvent>,
    start_wall_time: Instant,
}

impl EventBroadcaster {
    /// Create broadcaster with capacity
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(capacity);
        Self {
            tx,
            start_wall_time: Instant::now(),
        }
    }
    
    /// Subscribe to events
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<SimulationEvent> {
        self.tx.subscribe()
    }
    
    /// Emit event
    pub fn emit(&self, event: SimulationEvent) -> Result<usize, tokio::sync::broadcast::error::SendError<SimulationEvent>> {
        self.tx.send(event)
    }
    
    /// Get elapsed wall time
    pub fn wall_time_elapsed(&self) -> f64 {
        self.start_wall_time.elapsed().as_secs_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_milestone_display() {
        assert_eq!(MilestoneKind::Ignition.to_string(), "Ignition");
        assert_eq!(MilestoneKind::Apogee.to_string(), "Apogee");
    }
    
    #[test]
    fn test_event_is_terminal() {
        let summary = SimulationSummary {
            total_time: 100.0,
            max_altitude: 1000.0,
            max_downrange: 500.0,
            max_velocity: 100.0,
            milestone_count: 3,
            wall_time_elapsed: 10.0,
        };
        assert!(SimulationEvent::Completed(summary).is_terminal());
        
        let progress = ProgressSnapshot {
            sim_time: 10.0,
            progress_ratio: 0.1,
            altitude: 100.0,
            downrange: 50.0,
            velocity_mag: 50.0,
            state_summary: StateSnapshot {
                position: [0.0, 0.0, 100.0],
                velocity: [0.0, 0.0, 50.0],
                pitch: 45.0,
                roll: 0.0,
                yaw: 0.0,
                pitch_rate: 0.0,
                roll_rate: 0.0,
                yaw_rate: 0.0,
                accel_mag: 9.81,
            },
            wall_time_elapsed: 1.0,
        };
        assert!(!SimulationEvent::Progress(progress).is_terminal());
    }
}
