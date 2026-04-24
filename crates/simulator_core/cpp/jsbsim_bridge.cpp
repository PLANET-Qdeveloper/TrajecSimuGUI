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
    // FGFDMExec's own `LoadModel` consults the `AircraftPath` / `EnginePath`
    // / `SystemsPath` members directly (without re-applying `GetFullPath`),
    // so setting only `RootDir` would leave the ctor defaults (`"aircraft"`,
    // etc.) as relative strings and JSBSim would fail to open the config
    // file. Mirror the layout used by the stock `JSBSim` CLI so aircraft
    // XML is resolved under `<root>/aircraft/...`.
    fdm_->SetRootDir(SGPath(std::string(path)));
    fdm_->SetAircraftPath(SGPath("aircraft"));
    fdm_->SetEnginePath(SGPath("engine"));
    fdm_->SetSystemsPath(SGPath("systems"));
    fdm_->SetOutputPath(SGPath("."));
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

void FDMWrapper::set_sim_time(double value) {
    fdm_->Setsim_time(value);
}

std::unique_ptr<FDMWrapper> new_fdm_wrapper() {
    return std::make_unique<FDMWrapper>();
}
