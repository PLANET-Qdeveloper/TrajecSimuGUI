use thiserror::Error;

#[derive(Error, Debug)]
pub enum SimulatorError {
    #[error("Initialization error: {0}")]
    InitializationError(String),

    #[error("Simulation step error: {0}")]
    StepError(String),

    /// JSBSim returned false or reported an internal error.
    #[error("JSBSim error: {0}")]
    JsbSimError(String),

    /// Jinja2 template rendering failed.
    #[error("XML render error: {0}")]
    XmlRenderError(String),

    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SimulatorError>;
