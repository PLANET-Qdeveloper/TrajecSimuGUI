//! XML file generator for JSBSim input.
//!
//! Templates are embedded at compile time via askama, so no
//! file-system access is required at runtime and parallel runs are safe.

mod context;
pub use context::XmlContext;

use crate::workspace::SimWorkspace;
use crate::{Result, SimulatorError};
use askama::Template;
use std::fs;

// ── Static file ────────────────────────────────────────────────────────────

/// Unit-conversion definitions for CSV output — copied verbatim.
const UNIT_CONVERSIONS_XML: &str = include_str!("../../../param-xml-template/unitconversions.xml");

// ── Template structs ───────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pq_simulation.xml.j2", escape = "none")]
struct SimulationTmpl<'a> {
    ctx: &'a XmlContext,
}

#[derive(Template)]
#[template(path = "aircraft/PQ_ROCKET/pq_rocket.xml.j2", escape = "none")]
struct AircraftTmpl<'a> {
    ctx: &'a XmlContext,
}

#[derive(Template)]
#[template(path = "aircraft/PQ_ROCKET/liftoff.xml.j2", escape = "none")]
struct LiftoffTmpl<'a> {
    ctx: &'a XmlContext,
}

// ── Generator ──────────────────────────────────────────────────────────────

/// Renders all JSBSim input XML files into a `SimWorkspace`.
///
/// Stateless — templates are resolved at compile time by askama.
pub struct XmlGenerator;

impl XmlGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Render all templates and write files into `ws`.
    ///
    /// Directory layout produced:
    /// ```text
    /// <ws.root>/
    ///   pq_simulation.xml
    ///   unitconversions.xml
    ///   aircraft/PQ_ROCKET/
    ///     pq_rocket.xml
    ///     liftoff.xml
    /// ```
    pub fn render_into(&self, ctx: &XmlContext, ws: &SimWorkspace) -> Result<()> {
        fs::create_dir_all(ws.aircraft_dir())?;

        let render_err = |e: askama::Error| SimulatorError::XmlRenderError(e.to_string());

        let simulation = SimulationTmpl { ctx }.render().map_err(render_err)?;
        let aircraft = AircraftTmpl { ctx }.render().map_err(render_err)?;
        let liftoff = LiftoffTmpl { ctx }.render().map_err(render_err)?;

        fs::write(ws.script_path(), simulation)?;
        // JSBSim's `<use aircraft="PQ_ROCKET"/>` loads `PQ_ROCKET.xml` (case-
        // sensitive match to the aircraft name) on filesystems that don't
        // fold case, so write with the uppercase name rather than `pq_rocket`.
        fs::write(ws.aircraft_dir().join("PQ_ROCKET.xml"), aircraft)?;
        fs::write(ws.aircraft_dir().join("liftoff.xml"), liftoff)?;
        fs::write(ws.root().join("unitconversions.xml"), UNIT_CONVERSIONS_XML)?;

        Ok(())
    }
}

impl Default for XmlGenerator {
    fn default() -> Self {
        Self::new()
    }
}
