use oryxis_core::models::Connection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SshError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Authentication failed")]
    AuthFailed,

    #[error("Channel error: {0}")]
    Channel(String),

    #[error("Russh error: {0}")]
    Russh(String),
}

/// SSH engine — manages connections and channels.
/// Full implementation will wrap russh sessions.
pub struct SshEngine;

impl SshEngine {
    pub fn new() -> Self {
        Self
    }

    /// Placeholder: will connect using the full pipeline (direct/jump/proxy)
    pub async fn connect(&self, _connection: &Connection) -> Result<(), SshError> {
        tracing::info!("SSH engine: connect placeholder");
        Ok(())
    }
}
