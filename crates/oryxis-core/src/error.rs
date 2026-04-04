use thiserror::Error;

#[derive(Debug, Error)]
pub enum OryxisError {
    #[error("SSH error: {0}")]
    Ssh(String),

    #[error("Vault error: {0}")]
    Vault(String),

    #[error("Vault is locked")]
    VaultLocked,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Sync error: {0}")]
    Sync(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
