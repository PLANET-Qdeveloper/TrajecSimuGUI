//! JSBSim simulator backend.
//!
//! Lifecycle per run
//! ─────────────────
//! ```text
//! JsbSimSimulator::new()
//!   │
//!   ├── initialize(params)
//!   │     ├── XmlGenerator::render_into()  → writes XML to SimWorkspace
//!   │     ├── fdm.set_root_dir(ws.root())
//!   │     ├── fdm.load_script(ws.script_path())
//!   │     └── fdm.run_ic()
//!   │
//!   └── loop:
//!         step()              → fdm.run()  (one JSBSim dt)
//!         get_state()         → GetPropertyValue × N  ← called each step
//!         set_property(…)     → inject control inputs  (optional)
//!         if step() == false: break
//! ```

mod ffi;
mod state;

use cxx::UniquePtr;
use std::pin::Pin;

use crate::output::SimulationState;
use crate::params::RocketParams;
use crate::workspace::SimWorkspace;
use crate::xml_gen::{XmlContext, XmlGenerator};
use crate::{Result, Simulator, SimulatorError};

use ffi::bridge::FDMWrapper;

pub struct JsbSimSimulator {
    fdm: UniquePtr<FDMWrapper>,
    /// Kept alive to prevent temp-dir deletion during the run.
    _workspace: Option<SimWorkspace>,
    running: bool,
    generator: XmlGenerator,
    /// Launch site position — stored at `initialize` for local-coordinate computation.
    launch_lat_deg: f64,
    launch_lon_deg: f64,
    launch_yaw_deg: f64,
}

// ── Thread safety ───────────────────────────────────────────────────────────
//
// `UniquePtr<FDMWrapper>` wraps one `FGFDMExec`.  Each instance has its own
// property tree and state; there is no shared mutable global between
// instances.  We declare Send/Sync here so that `JsbSimSimulator` can be
// moved to a `tokio::task::spawn_blocking` thread.
//
// If JSBSim's simgear dependency ever introduces shared globals this
// assertion must be revisited.
unsafe impl Send for JsbSimSimulator {}
unsafe impl Sync for JsbSimSimulator {}

impl JsbSimSimulator {
    pub fn new() -> Self {
        Self {
            fdm: ffi::bridge::new_fdm_wrapper(),
            _workspace: None,
            running: false,
            generator: XmlGenerator::new(),
            launch_lat_deg: 0.0,
            launch_lon_deg: 0.0,
            launch_yaw_deg: 0.0,
        }
    }

    fn fdm_mut(&mut self) -> Pin<&mut FDMWrapper> {
        self.fdm.pin_mut()
    }
}

impl Default for JsbSimSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Simulator for JsbSimSimulator {
    fn initialize(&mut self, params: &RocketParams) -> Result<()> {
        self.launch_lat_deg = params.launch_env.latitude;
        self.launch_lon_deg = params.launch_env.longitude;
        self.launch_yaw_deg = params.launch_env.yaw;
        params.validate()?;

        std::env::set_var("JSBSIM_DEBUG", "0");

        // 1. Write XML files to an isolated temp directory.
        let ws = SimWorkspace::new()?;
        let ctx = XmlContext::from(params);
        self.generator.render_into(&ctx, &ws)?;

        // 2. Point JSBSim at the workspace root so that `aircraft/`,
        //    `engine/`, etc. are resolved correctly.
        let root = ws.root().to_str().ok_or_else(|| {
            SimulatorError::InitializationError(
                "workspace path contains non-UTF-8 characters".into(),
            )
        })?;
        self.fdm_mut().set_root_dir(root);

        // 3. Load the runscript.
        let script = ws.script_path();
        let script_str = script.to_str().ok_or_else(|| {
            SimulatorError::InitializationError("script path contains non-UTF-8 characters".into())
        })?;
        if !self.fdm_mut().load_script(script_str) {
            return Err(SimulatorError::JsbSimError(
                "LoadScript returned false — check XML syntax".into(),
            ));
        }

        // 4. Apply initial conditions.
        if !self.fdm_mut().run_ic() {
            return Err(SimulatorError::JsbSimError(
                "RunIC returned false — check liftoff.xml".into(),
            ));
        }

        // 5. Override JSBSim's internal clock if the caller asked for a
        //    non-zero start time (e.g. launch-rail-exit handoff). RunIC
        //    always resets sim-time to 0, so this must come *after* it.
        if params.sim.start_sim_time_sec != 0.0 {
            self.fdm_mut().set_sim_time(params.sim.start_sim_time_sec);
        }

        // 6. Disable jsbsim normal output
        self.fdm_mut().disable_output();

        // 7. Keep the workspace alive for the duration of the run.
        self._workspace = Some(ws);
        self.running = true;
        Ok(())
    }

    /// Advance one JSBSim time step.
    ///
    /// Returns `true` while the simulation is running.
    /// Returns `false` (and marks the simulator as done) when JSBSim
    /// signals termination via the script's `<event>` conditions.
    fn step(&mut self) -> Result<bool> {
        if !self.running {
            return Ok(false);
        }
        let continues = self.fdm_mut().run();
        if !continues {
            self.running = false;
            // Release the workspace (and its temp directory) now.
            self._workspace = None;
        }
        Ok(continues)
    }

    /// Read the current vehicle state.
    ///
    /// Designed to be called after every `step()` call (or every N steps).
    /// All values are converted from JSBSim's fps/lbs/psf system to SI.
    fn get_state(&self) -> Result<SimulationState> {
        Ok(state::extract_state(
            &self.fdm,
            self.launch_lat_deg,
            self.launch_lon_deg,
            self.launch_yaw_deg,
        ))
    }

    /// Inject a JSBSim property value between steps.
    ///
    /// Example (release hold-down early):
    /// ```ignore
    /// sim.set_property("forces/hold-down", 0.0)?;
    /// ```
    fn set_property(&mut self, key: &str, value: f64) -> Result<()> {
        self.fdm_mut().set_property(key, value);
        Ok(())
    }
}
