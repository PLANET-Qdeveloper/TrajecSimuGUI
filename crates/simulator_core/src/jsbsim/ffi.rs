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


        fn disable_output(self: Pin<&mut FDMWrapper>);

        /// Read a named JSBSim property (JSBSim internal units).
        #[allow(unused)]
        fn get_property(self: &FDMWrapper, name: &str) -> f64;

        /// Write a named JSBSim property.
        /// Use between `run()` calls to inject real-time control inputs.
        fn set_property(self: Pin<&mut FDMWrapper>, name: &str, value: f64);

        /// Override JSBSim's internal simulation time (`FGFDMExec::Setsim_time`).
        /// `run_ic` resets sim-time to 0; call this *after* `run_ic` to start
        /// integration from a non-zero time (e.g. a launch-rail-exit handoff).
        fn set_sim_time(self: Pin<&mut FDMWrapper>, value: f64);

        // -------------------------------------------------------------------
        // Direct state accessors (bypass FGPropertyManager string lookup).
        // All values in JSBSim internal units (ft, fps, rad, psf, lbf …).
        // -------------------------------------------------------------------

        fn get_sim_time_sec(self: &FDMWrapper) -> f64;

        // Position
        fn get_lat_gc_deg(self: &FDMWrapper) -> f64;
        fn get_lon_gc_deg(self: &FDMWrapper) -> f64;
        fn get_h_agl_ft(self: &FDMWrapper) -> f64;

        // Velocity
        fn get_vtrue_fps(self: &FDMWrapper) -> f64;
        fn get_vg_fps(self: &FDMWrapper) -> f64;
        fn get_u_fps(self: &FDMWrapper) -> f64;
        fn get_v_fps(self: &FDMWrapper) -> f64;
        fn get_w_fps(self: &FDMWrapper) -> f64;

        // Attitude
        fn get_phi_rad(self: &FDMWrapper) -> f64;
        fn get_theta_rad(self: &FDMWrapper) -> f64;
        fn get_psi_rad(self: &FDMWrapper) -> f64;

        // Angular rates
        fn get_p_rad_sec(self: &FDMWrapper) -> f64;
        fn get_q_rad_sec(self: &FDMWrapper) -> f64;
        fn get_r_rad_sec(self: &FDMWrapper) -> f64;

        // Acceleration (body frame)
        fn get_udot_ft_sec2(self: &FDMWrapper) -> f64;
        fn get_vdot_ft_sec2(self: &FDMWrapper) -> f64;
        fn get_wdot_ft_sec2(self: &FDMWrapper) -> f64;

        // Aerodynamics
        fn get_alpha_rad(self: &FDMWrapper) -> f64;
        fn get_beta_rad(self: &FDMWrapper) -> f64;
        fn get_qbar_psf(self: &FDMWrapper) -> f64;
        fn get_mach(self: &FDMWrapper) -> f64;

        // Propulsion
        fn get_thrust_magnitude_lbf(self: &FDMWrapper) -> f64;
    }
}
