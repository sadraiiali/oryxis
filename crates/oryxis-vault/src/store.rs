use thiserror::Error;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("Vault is locked")]
    Locked,

    #[error("Invalid master password")]
    InvalidPassword,

    #[error("Database error: {0}")]
    Database(String),

    #[error("Crypto error: {0}")]
    Crypto(String),
}

/// Vault store — manages encrypted SQLite database.
/// Full implementation will use SQLCipher + argon2id.
pub struct VaultStore {
    locked: bool,
}

impl VaultStore {
    pub fn new() -> Self {
        Self { locked: true }
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Placeholder: will derive key with argon2id and open SQLCipher DB
    pub fn unlock(&mut self, _master_password: &str) -> Result<(), VaultError> {
        tracing::info!("Vault: unlock placeholder");
        self.locked = false;
        Ok(())
    }

    pub fn lock(&mut self) {
        self.locked = true;
    }
}
