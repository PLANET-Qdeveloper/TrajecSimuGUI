//! # simulator_core
//!
//! Core simulation engine for TrajecSimuGUI.
//!
//! ## Backends
//! - [`JsbSimSimulator`] — full 6-DOF simulation via JSBSim C++ (cxx binding)
//! - [`CustomSimulator`] — lightweight ballistic model for quick sanity checks
//!
//! ## Typical usage
//! ```rust,ignore
//! use simulator_core::{Simulator, JsbSimSimulator};
//! use simulator_core::output::SimulationOutput;
//!
//! let mut sim = JsbSimSimulator::new();
//! sim.initialize(&params)?;
//!
//! let mut output = SimulationOutput::new();
//! while sim.step()? {
//!     let state = sim.get_state()?;
//!     output.push(state);
//!
//!     // Optional real-time control injection:
//!     // sim.set_property("forces/hold-down", 0.0)?;
//! }
//! ```

pub mod analysis;
pub mod arc_serde;
pub mod error;
pub mod jsbsim;
pub mod orchestrator;
pub mod output;
pub mod params;
pub mod progress;
pub mod simple_simulator;
pub mod workspace;
pub mod xml_gen;

pub use error::{Result, SimulatorError};
pub use jsbsim::JsbSimSimulator;
pub use orchestrator::{Phase, SimulationOrchestrator, UnifiedSimulationOutput};
pub use output::{SimulationState, Trajectory};
pub use params::RocketParams;
pub use progress::{EventKind, EventSource, EventStamp};

use output::SimulationState as State;
use params::RocketParams as Params;

/// Common interface implemented by all simulator backends.
///
/// # Step-based design
///
/// The step loop maps directly to JSBSim's `FGFDMExec::Run()`:
///
/// ```text
/// initialize(params)   → LoadScript + RunIC
/// loop:
///   step()             → Run()           // one JSBSim dt
///   get_state()        → GetPropertyValue × N  ← call every step (or N steps)
///   set_property(…)    → SetPropertyValue      ← optional control injection
///   if !step(): break
/// ```
///
/// `CustomSimulator` follows the same interface with integrated ballistic
/// physics, allowing backends to be swapped without changing the loop.
pub trait Simulator: Send + Sync {
    /// Load parameters and prepare the simulator for stepping.
    fn initialize(&mut self, params: &Params) -> Result<()>;

    /// Advance one simulation time step.
    ///
    /// Returns `true` while the simulation is running.
    /// Returns `false` when the simulation has ended (landed, apogee
    /// with terminate, or `flight_duration` exceeded).
    fn step(&mut self) -> Result<bool>;

    /// Read the current vehicle state.
    ///
    /// Intended to be called after each `step()`. Per-output decimation
    /// is applied at write-time using `SimControl::csv_sample_interval`
    /// and `SimControl::kml_sample_interval`, so the simulator itself
    /// always produces the full-resolution trajectory in memory.
    /// JSBSim backend: reads via `GetPropertyValue`, converts to SI.
    /// Custom backend: returns integrated state directly.
    fn get_state(&self) -> Result<State>;

    /// Inject a property value into the simulator between steps.
    ///
    /// JSBSim backend: calls `SetPropertyValue(key, value)`.
    /// Custom backend: no-op (override if needed).
    ///
    /// Useful for real-time control inputs such as releasing hold-down,
    /// triggering parachute deployment, or adjusting wind.
    fn set_property(&mut self, _key: &str, _value: f64) -> Result<()> {
        Ok(())
    }
}

fn assert_send_sync<T: Send + Sync>() {}

const _: () = {
    let _ = assert_send_sync::<JsbSimSimulator>;
};
