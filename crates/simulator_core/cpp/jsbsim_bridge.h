#pragma once

#include <memory>
#include "rust/cxx.h"

// Forward declaration to avoid pulling all JSBSim headers into the bridge
namespace JSBSim {
class FGFDMExec;
}

/// Thin C++ wrapper around FGFDMExec, exposed to Rust via cxx.
///
/// Each instance owns one FGFDMExec. Multiple instances can run on
/// separate threads (no shared mutable global state per-instance).
class FDMWrapper {
public:
    FDMWrapper();
    ~FDMWrapper();

    // Prevent copy (FGFDMExec is not copyable)
    FDMWrapper(const FDMWrapper&) = delete;
    FDMWrapper& operator=(const FDMWrapper&) = delete;

    /// Set JSBSim root directory (aircraft/, engine/, etc. are relative to this).
    void set_root_dir(rust::Str path);

    /// Load a runscript XML file. Returns true on success.
    bool load_script(rust::Str path);

    /// Apply initial conditions (call once after load_script).
    bool run_ic();

    /// Advance simulation by one dt step.
    /// Returns true while running, false when the simulation has ended.
    bool run();

    void disable_output();
    /// Read a named JSBSim property.
    /// Values are in JSBSim's internal units (fps, lbs, psf, rad …).
    double get_property(rust::Str name) const;

    /// Write a named JSBSim property.
    /// Use for real-time control injection between steps.
    void set_property(rust::Str name, double value);

    /// Override JSBSim's internal simulation time (`FGFDMExec::Setsim_time`).
    /// `run_ic` resets sim-time to 0; call this *after* `run_ic` to start
    /// integration from a non-zero time (e.g. a launch-rail-exit handoff).
    void set_sim_time(double value);

    // -----------------------------------------------------------------------
    // Direct state accessors — bypass FGPropertyManager string lookup.
    // All values are in JSBSim internal units (ft, fps, rad, psf, lbf …).
    // -----------------------------------------------------------------------

    // Simulation time
    double get_sim_time_sec() const;

    // Position  (FGPropagate)
    double get_lat_gc_deg() const;
    double get_lon_gc_deg() const;
    double get_h_agl_ft()   const;

    // Velocity  (FGAuxiliary / FGPropagate)
    double get_vtrue_fps()  const;
    double get_vg_fps()     const;
    double get_u_fps()      const;
    double get_v_fps()      const;
    double get_w_fps()      const;

    // Attitude  (FGPropagate)
    double get_phi_rad()    const;
    double get_theta_rad()  const;
    double get_psi_rad()    const;

    // Angular rates  (FGPropagate)
    double get_p_rad_sec()  const;
    double get_q_rad_sec()  const;
    double get_r_rad_sec()  const;

    // Acceleration body-frame  (FGAccelerations)
    double get_udot_ft_sec2() const;
    double get_vdot_ft_sec2() const;
    double get_wdot_ft_sec2() const;

    // Aerodynamics  (FGAuxiliary)
    double get_alpha_rad()  const;
    double get_beta_rad()   const;
    double get_qbar_psf()   const;
    double get_mach()       const;

    // Propulsion  (FGExternalReactions)
    double get_thrust_magnitude_lbf() const;

private:
    std::unique_ptr<JSBSim::FGFDMExec> fdm_;
};

/// Factory function (required by cxx for heap-allocated opaque types).
std::unique_ptr<FDMWrapper> new_fdm_wrapper();
