//! OS keyring credential storage for distributor adapters.
//!
//! Spec (WS-C):
//! - Service name format: `signex-distributor-<provider>`
//! - Used by Mouser (API key) and DigiKey (OAuth refresh token).
//! - Tests gated by platform: Windows Credential Manager works; Linux/macOS
//!   CI runners may lack a backend → callers must handle
//!   `KeyringError::Backend` gracefully.

use ::keyring::Entry;

const SERVICE_PREFIX: &str = "signex-distributor-";

#[derive(Debug, thiserror::Error)]
pub enum KeyringError {
    #[error("keyring backend unavailable: {0}")]
    Backend(String),
    #[error("entry not found")]
    NotFound,
}

impl From<::keyring::Error> for KeyringError {
    fn from(e: ::keyring::Error) -> Self {
        match e {
            ::keyring::Error::NoEntry => KeyringError::NotFound,
            other => KeyringError::Backend(other.to_string()),
        }
    }
}

/// Wrapper around a single keyring entry, scoped to one distributor provider.
///
/// One `KeyringStore` instance maps to one underlying OS keychain item. The
/// service name follows the spec: `signex-distributor-<provider>` (e.g.
/// `signex-distributor-digikey`). The username slot lets callers separate
/// e.g. an OAuth access token from a refresh token (`"access"`/`"refresh"`).
#[derive(Debug)]
pub struct KeyringStore {
    service: String,
    username: String,
    entry: Entry,
}

impl KeyringStore {
    /// Create a store for `provider` with the given `username` slot.
    ///
    /// # Panics
    /// `keyring::Entry::new` only fails if the platform refuses any entry —
    /// in that case `set_secret` / `get_secret` will surface the real error
    /// later. We construct lazily and unwrap here so the API is infallible
    /// at construction.
    pub fn for_provider(provider: &str, username: &str) -> Self {
        let service = format!("{SERVICE_PREFIX}{provider}");
        let entry = Entry::new(&service, username)
            .expect("keyring::Entry::new is infallible on supported platforms");
        Self {
            service,
            username: username.to_string(),
            entry,
        }
    }

    /// Service name as registered with the OS keychain.
    pub fn service_name(&self) -> &str {
        &self.service
    }

    /// Username slot.
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Persist the secret. Overwrites any existing value.
    pub fn set_secret(&self, secret: &str) -> Result<(), KeyringError> {
        self.entry.set_password(secret).map_err(KeyringError::from)
    }

    /// Read the stored secret. Returns `KeyringError::NotFound` if absent.
    pub fn get_secret(&self) -> Result<String, KeyringError> {
        self.entry.get_password().map_err(KeyringError::from)
    }

    /// Delete the entry. Idempotent: deleting an absent entry is `Ok`.
    pub fn delete(&self) -> Result<(), KeyringError> {
        match self.entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(::keyring::Error::NoEntry) => Ok(()),
            Err(other) => Err(KeyringError::Backend(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_prefix_is_stable() {
        assert_eq!(SERVICE_PREFIX, "signex-distributor-");
    }

    #[test]
    fn for_provider_builds_expected_service_name() {
        let s = KeyringStore::for_provider("mouser", "default");
        assert_eq!(s.service_name(), "signex-distributor-mouser");
        assert_eq!(s.username(), "default");
    }
}
