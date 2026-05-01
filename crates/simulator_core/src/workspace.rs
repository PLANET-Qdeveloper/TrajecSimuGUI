//! Temporary working directory for one simulation run.
//!
//! Each `SimWorkspace` gets its own isolated directory under the system
//! temp folder.  JSBSim reads aircraft/script XML from here and writes
//! CSV output here.  Multiple workspaces can coexist safely (parallel runs).
//!
//! The directory is deleted automatically when `SimWorkspace` is dropped.

use crate::error::Result;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;

pub struct SimWorkspace {
    dir: TempDir,
    /// Unique ID, useful for logging parallel runs.
    pub id: Uuid,
}

impl SimWorkspace {
    /// Create a fresh workspace under the OS temp directory.
    pub fn new() -> Result<Self> {
        let dir = tempfile::Builder::new().prefix("trajec-").tempdir()?;
        Ok(Self {
            dir,
            id: Uuid::new_v4(),
        })
    }

    /// Root directory (JSBSim `SetRootDir` target).
    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Path for the main runscript XML.
    pub fn script_path(&self) -> PathBuf {
        self.dir.path().join("pq_simulation.xml")
    }

    /// Directory for the aircraft FDM XML files.
    ///
    /// JSBSim looks for aircraft in `<root>/aircraft/<AircraftName>/`.
    pub fn aircraft_dir(&self) -> PathBuf {
        self.dir.path().join("aircraft/PQ_ROCKET")
    }

    /// Expected location of JSBSim's CSV output (for debugging).
    pub fn csv_output_path(&self) -> PathBuf {
        self.dir.path().join("pq_rocket_output_raw.csv")
    }
}
