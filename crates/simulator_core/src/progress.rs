//! Canonical simulation event model for composite orchestration.

use serde::{Deserialize, Serialize};

use crate::output::SimulationState;

/// Canonical one-shot events used by composite simulation orchestration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventKind {
    Start,
    LaunchClear,
    EngineBurnout,
    Apogee,
    ParachuteOpen,
    Landed,
    ParachuteLanded,
    // Derived events emitted by `analysis::analyze` after the simulation
    // completes. Each fires once at the trajectory step where the metric
    // peaks; the `state` payload on `EventStamp` carries the full
    // SimulationState snapshot at that instant.
    /// Peak `aero.qbar_pa` over the mainline trajectory.
    MaxQ,
    /// Peak `acceleration.ax_mps2` (signed maximum, body axial).
    MaxAxialAcceleration,
    /// Peak `thrust_n` over the mainline trajectory.
    MaxThrust,
    /// Peak `velocity.true_airspeed_mps` over the mainline trajectory.
    MaxAirspeed,
    /// Peak `aero.qbar_pa * aero.alpha_deg` over the mainline trajectory.
    MaxDynamicPressureAlpha,
    /// Peak `sqrt(ay² + az²)` magnitude (body lateral).
    MaxLateralAcceleration,
    /// Peak `sqrt(p² + q² + r²)` magnitude (body angular rate).
    MaxAngularRate,
}

impl std::fmt::Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start => write!(f, "start"),
            Self::LaunchClear => write!(f, "launch_clear"),
            Self::EngineBurnout => write!(f, "engine_burnout"),
            Self::Apogee => write!(f, "apogee"),
            Self::ParachuteOpen => write!(f, "parachute_open"),
            Self::Landed => write!(f, "landed"),
            Self::ParachuteLanded => write!(f, "parachute_landed"),
            Self::MaxQ => write!(f, "max_q"),
            Self::MaxAxialAcceleration => write!(f, "max_axial_acceleration"),
            Self::MaxThrust => write!(f, "max_thrust"),
            Self::MaxAirspeed => write!(f, "max_airspeed"),
            Self::MaxDynamicPressureAlpha => write!(f, "max_dynamic_pressure_alpha"),
            Self::MaxLateralAcceleration => write!(f, "max_lateral_acceleration"),
            Self::MaxAngularRate => write!(f, "max_angular_rate"),
        }
    }
}

/// Event source for tracing phase ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSource {
    Orchestrator,
    LaunchRail,
    JsbSim,
    Parachute,
    /// Post-simulation analysis pass (`crate::analysis`).
    Analysis,
    External,
}

/// Timestamped event record emitted into unified external output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStamp {
    pub kind: EventKind,
    pub sim_time_sec: f64,
    pub source: EventSource,
    /// Full vehicle state snapshot at the instant the event fired. `None`
    /// for events that have no meaningful per-step state (e.g. the
    /// pre-physics `Start` marker).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<SimulationState>,
}

/// One-shot branch trigger specification: "fire `delay_sec` after `origin` event."
///
/// Pure configuration; the "has fired" latch is held by whichever phase-runner
/// or orchestrator owns the trigger, since that state is run-local and should
/// not be serialized into user-provided parameter files.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DelayedBranchTrigger {
    /// Event that starts the countdown.
    pub origin: EventKind,
    /// Delay from origin event before the trigger fires [s].
    pub delay_sec: f64,
}
