//! cxx bridge: Rust ↔ C++ (FDMWrapper / FGFDMExec).

#[cxx::bridge]
pub mod bridge {
    unsafe extern "C++" {
        include!("jsbsim_bridge.h");

        /// Opaque wrapper around `JSBSim::FGFDMExec`.
        type FDMWrapper;

        /// Allocate a new `FDMWrapper` on the heap.
        fn new_fdm_wrapper() -> UniquePtr<FDMWrapper>;

        /// Set JSBSim root directory.
        /// Aircraft, engine, and systems directories are resolved relative
        /// to this path.
        fn set_root_dir(self: Pin<&mut FDMWrapper>, path: &str);

        /// Load a runscript XML file.  Returns `true` on success.
        fn load_script(self: Pin<&mut FDMWrapper>, path: &str) -> bool;

        /// Apply initial conditions.  Call once after `load_script`.
        fn run_ic(self: Pin<&mut FDMWrapper>) -> bool;

        /// Advance simulation by one time step.
        /// Returns `true` while running, `false` when the script signals
        /// termination (landed, apogee, or `<run end=…>` reached).
        fn run(self: Pin<&mut FDMWrapper>) -> bool;

        /// Read a named JSBSim property (JSBSim internal units).
        fn get_property(self: &FDMWrapper, name: &str) -> f64;

        /// Write a named JSBSim property.
        /// Use between `run()` calls to inject real-time control inputs.
        fn set_property(self: Pin<&mut FDMWrapper>, name: &str, value: f64);

        /// Override JSBSim's internal simulation time (`FGFDMExec::Setsim_time`).
        /// `run_ic` resets sim-time to 0; call this *after* `run_ic` to start
        /// integration from a non-zero time (e.g. a launch-rail-exit handoff).
        fn set_sim_time(self: Pin<&mut FDMWrapper>, value: f64);
    }
}
