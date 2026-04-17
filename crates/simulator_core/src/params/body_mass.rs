use serde::{Deserialize, Serialize};

/// Integrated airframe + mass/inertia parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyMassParams {
    /// Body diameter (m).
    pub diameter: f64,

    /// Total launch mass including all propellant (kg).
    /// JSBSim `emptywt` = `total_mass - fuel_contents`.
    pub total_mass: f64,

    /// Centre of gravity from nose tip, body-axis: [x, y, z] (m).
    pub cg: [f64; 3],

    /// Inertia tensor components (kg·m²): [Ixx, Iyy, Izz, Ixy, Ixz, Iyz].
    pub inertia: [f64; 6],
}
