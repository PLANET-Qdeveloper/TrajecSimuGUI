//! XML file generator for JSBSim input.
//!
//! Templates are embedded at compile time via `include_str!`, so no
//! file-system access is required at runtime and parallel runs are safe.

mod context;
pub use context::XmlContext;

use std::fs;
use minijinja::{Environment, value::Value};
use crate::{Result, SimulatorError};
use crate::workspace::SimWorkspace;

// ── Embedded templates ──────────────────────────────────────────────────────

const TMPL_SIMULATION: &str =
    include_str!("../../../param-xml-template/pq_simulation.xml.j2");

const TMPL_AIRCRAFT: &str =
    include_str!("../../../param-xml-template/aircraft/PQ_ROCKET/pq_rocket.xml.j2");

const TMPL_LIFTOFF: &str =
    include_str!("../../../param-xml-template/aircraft/PQ_ROCKET/liftoff.xml.j2");

/// Static file copied verbatim (unit-conversion definitions for CSV output).
const UNIT_CONVERSIONS_XML: &str =
    include_str!("../../../param-xml-template/unitconversions.xml");

// ── Generator ──────────────────────────────────────────────────────────────

/// Renders all JSBSim input XML files into a `SimWorkspace`.
///
/// One `XmlGenerator` instance can be shared across threads because
/// `Environment` is immutable after construction.
pub struct XmlGenerator {
    env: Environment<'static>,
}

impl XmlGenerator {
    pub fn new() -> Self {
        let mut env = Environment::new();

        env.add_template("simulation", TMPL_SIMULATION)
            .expect("pq_simulation.xml.j2 is invalid");
        env.add_template("aircraft", TMPL_AIRCRAFT)
            .expect("pq_rocket.xml.j2 is invalid");
        env.add_template("liftoff", TMPL_LIFTOFF)
            .expect("liftoff.xml.j2 is invalid");

        Self { env }
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
        let val = Value::from_serialize(ctx);

        fs::create_dir_all(ws.aircraft_dir())?;

        let render = |name: &str| -> Result<String> {
            self.env
                .get_template(name)
                .and_then(|t| t.render(&val))
                .map_err(|e| SimulatorError::XmlRenderError(e.to_string()))
        };

        fs::write(ws.script_path(),                          render("simulation")?)?;
        fs::write(ws.aircraft_dir().join("pq_rocket.xml"),   render("aircraft")?)?;
        fs::write(ws.aircraft_dir().join("liftoff.xml"),     render("liftoff")?)?;
        fs::write(ws.root().join("unitconversions.xml"),     UNIT_CONVERSIONS_XML)?;

        Ok(())
    }
}

impl Default for XmlGenerator {
    fn default() -> Self {
        Self::new()
    }
}
