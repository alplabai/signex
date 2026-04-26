//! Stub for keyring credential storage. Filled by Task 4.
//!
//! Spec (WS-C): wraps the `keyring` crate with service name
//! `signex-distributor-<provider>`, used by Mouser/DigiKey to store
//! API keys / refresh tokens.

#[derive(Debug, thiserror::Error)]
pub enum KeyringError {
    #[error("keyring backend unavailable: {0}")]
    Backend(String),
    #[error("entry not found")]
    NotFound,
}

pub struct KeyringStore;
