#include "jsbsim_bridge.h"

// cxx-generated header (produced from ffi.rs at build time)
#include "simulator_core/src/jsbsim/ffi.rs.h"

#include <FGFDMExec.h>
#include <FGJSBBase.h>

#include "models/FGAccelerations.h"
#include "models/FGAtmosphere.h"
#include "models/FGAuxiliary.h"
#include "models/FGPropulsion.h"
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

void FDMWrapper::disable_output()
{
    fdm_->SetDebugLevel(0);  // Suppress JSBSim's verbose stdout by default.
    fdm_->DisableOutput();
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

// ---------------------------------------------------------------------------
// Direct state accessors
// ---------------------------------------------------------------------------

double FDMWrapper::get_sim_time_sec() const {
    return fdm_->GetSimTime();
}

// ---- Position --------------------------------------------------------------

double FDMWrapper::get_lat_gc_deg() const {
    return fdm_->GetPropagate()->GetLatitudeDeg();
}

double FDMWrapper::get_lon_gc_deg() const {
    return fdm_->GetPropagate()->GetLongitudeDeg();
}

double FDMWrapper::get_h_agl_ft() const {
    return fdm_->GetPropagate()->GetDistanceAGL();
}

// ---- Velocity --------------------------------------------------------------

double FDMWrapper::get_vtrue_fps() const {
    return fdm_->GetAuxiliary()->GetVtrueFPS();
}

double FDMWrapper::get_vg_fps() const {
    return fdm_->GetAuxiliary()->GetVground();
}

double FDMWrapper::get_u_fps() const {
    return fdm_->GetAuxiliary()->GetAeroUVW(JSBSim::FGJSBBase::eU);
}

double FDMWrapper::get_v_fps() const {
    return fdm_->GetAuxiliary()->GetAeroUVW(JSBSim::FGJSBBase::eV);
}

double FDMWrapper::get_w_fps() const {
    return fdm_->GetAuxiliary()->GetAeroUVW(JSBSim::FGJSBBase::eW);
}

// ---- Attitude --------------------------------------------------------------

double FDMWrapper::get_phi_rad() const {
    return fdm_->GetPropagate()->GetEuler(JSBSim::FGJSBBase::ePhi);
}

double FDMWrapper::get_theta_rad() const {
    return fdm_->GetPropagate()->GetEuler(JSBSim::FGJSBBase::eTht);
}

double FDMWrapper::get_psi_rad() const {
    return fdm_->GetPropagate()->GetEuler(JSBSim::FGJSBBase::ePsi);
}

// ---- Angular rates ---------------------------------------------------------

double FDMWrapper::get_p_rad_sec() const {
    return fdm_->GetPropagate()->GetPQR(JSBSim::FGJSBBase::eP);
}

double FDMWrapper::get_q_rad_sec() const {
    return fdm_->GetPropagate()->GetPQR(JSBSim::FGJSBBase::eQ);
}

double FDMWrapper::get_r_rad_sec() const {
    return fdm_->GetPropagate()->GetPQR(JSBSim::FGJSBBase::eR);
}

// ---- Acceleration ----------------------------------------------------------

double FDMWrapper::get_udot_ft_sec2() const {
    return fdm_->GetAccelerations()->GetUVWdot(JSBSim::FGJSBBase::eU);
}

double FDMWrapper::get_vdot_ft_sec2() const {
    return fdm_->GetAccelerations()->GetUVWdot(JSBSim::FGJSBBase::eV);
}

double FDMWrapper::get_wdot_ft_sec2() const {
    return fdm_->GetAccelerations()->GetUVWdot(JSBSim::FGJSBBase::eW);
}

// ---- Aerodynamics ----------------------------------------------------------

double FDMWrapper::get_alpha_rad() const {
    return fdm_->GetAuxiliary()->Getalpha();
}

double FDMWrapper::get_beta_rad() const {
    return fdm_->GetAuxiliary()->Getbeta();
}

double FDMWrapper::get_qbar_psf() const {
    return fdm_->GetAuxiliary()->Getqbar();
}

double FDMWrapper::get_mach() const {
    return fdm_->GetAuxiliary()->GetMach();
}

// ---- Propulsion ------------------------------------------------------------

double FDMWrapper::get_thrust_magnitude_lbf() const {
    // Get total propulsion thrust force magnitude
    // FGPropulsion provides the total thrust forces in body axes
    auto thrust_forces = fdm_->GetPropulsion()->GetForces();
    return thrust_forces.Magnitude();
}

double FDMWrapper::get_pressure_psf() const {
    return fdm_->GetAtmosphere()->GetPressure();
}

double FDMWrapper::get_temperature_rankine() const {
    return fdm_->GetAtmosphere()->GetTemperature();
}

std::unique_ptr<FDMWrapper> new_fdm_wrapper() {
    return std::make_unique<FDMWrapper>();
}
