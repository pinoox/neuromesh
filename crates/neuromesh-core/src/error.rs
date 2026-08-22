use thiserror::Error;

#[derive(Error, Debug)]
pub enum NeuroMeshError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite_error::DatabaseError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Project not found: {0}")]
    ProjectNotFound(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Provider error ({provider}): {message}")]
    Provider { provider: String, message: String },

    #[error("Local AI inference error: {0}")]
    LocalAi(String),

    #[error("Parser error: {0}")]
    Parser(String),

    #[error("Quality Gate rejection: {0}")]
    QualityGateRejection(String),

    #[error("Context expansion failed: {0}")]
    ExpansionFailed(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub mod rusqlite_error {
    use thiserror::Error;

    #[derive(Error, Debug)]
    #[error("{0}")]
    pub struct DatabaseError(pub String);
}

pub type Result<T> = std::result::Result<T, NeuroMeshError>;
