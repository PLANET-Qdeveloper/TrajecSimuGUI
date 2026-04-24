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

private:
    std::unique_ptr<JSBSim::FGFDMExec> fdm_;
};

/// Factory function (required by cxx for heap-allocated opaque types).
std::unique_ptr<FDMWrapper> new_fdm_wrapper();
