#include "jsbsim_bridge.h"

// cxx-generated header (produced from ffi.rs at build time)
#include "simulator_core/src/jsbsim/ffi.rs.h"

#include <FGFDMExec.h>

// SGPath is declared in FGFDMExec.h transitively via FGJSBBase.h
// It lives in the global namespace in JSBSim's bundled simgear.

FDMWrapper::FDMWrapper()
    : fdm_(std::make_unique<JSBSim::FGFDMExec>())
{
    // Suppress JSBSim's verbose stdout by default.
    // Remove this line to re-enable JSBSim console output during development.
    fdm_->SetDebugLevel(0);
}

FDMWrapper::~FDMWrapper() = default;

void FDMWrapper::set_root_dir(rust::Str path) {
    fdm_->SetRootDir(SGPath(std::string(path)));
}

bool FDMWrapper::load_script(rust::Str path) {
    return fdm_->LoadScript(SGPath(std::string(path)));
}

bool FDMWrapper::run_ic() {
    return fdm_->RunIC();
}

bool FDMWrapper::run() {
    return fdm_->Run();
}

double FDMWrapper::get_property(rust::Str name) const {
    return fdm_->GetPropertyValue(std::string(name));
}

void FDMWrapper::set_property(rust::Str name, double value) {
    fdm_->SetPropertyValue(std::string(name), value);
}

std::unique_ptr<FDMWrapper> new_fdm_wrapper() {
    return std::make_unique<FDMWrapper>();
}
