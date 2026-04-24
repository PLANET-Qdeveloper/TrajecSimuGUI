//! Canonical simulation event model for composite orchestration.

use serde::{Deserialize, Serialize};

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
    External,
}

/// Timestamped event record emitted into unified external output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventStamp {
    pub kind: EventKind,
    pub sim_time_sec: f64,
    pub source: EventSource,
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
