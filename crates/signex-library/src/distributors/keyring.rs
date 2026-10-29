//! OS keyring credential storage for distributor adapters.
//!
//! - Service name format: `signex-distributor-<provider>`
//! - Used by Mouser (API key) and DigiKey (OAuth refresh token).
//! - Backends (see this crate's `Cargo.toml`): Windows Credential Manager,
//!   macOS Keychain, Linux Secret Service over D-Bus (pure-Rust zbus client).
//!   There is deliberately NO plaintext or in-memory fallback: when the OS
//!   has no usable keychain every operation returns `KeyringError::Backend`
//!   and callers must show it to the user.

use ::keyring::Entry;

const SERVICE_PREFIX: &str = "signex-distributor-";

/// Actionable remediation appended to every backend failure shown to the user.
#[cfg(target_os = "linux")]
const KEYCHAIN_HINT: &str = "Signex stores API credentials in the Secret Service keychain; \
     start GNOME Keyring or KeePassXC (Secret Service integration) on the D-Bus session bus and unlock it.";
#[cfg(not(target_os = "linux"))]
const KEYCHAIN_HINT: &str = "Signex stores API credentials in the OS keychain; \
     check that it is available and unlocked.";

#[derive(Debug, thiserror::Error)]
pub enum KeyringError {
    #[error("OS keychain unavailable ({0}). {KEYCHAIN_HINT}")]
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
    /// MD-17: returns `Result` because `keyring::Entry::new` can fail on
    /// platforms without a daemon (Linux Docker without dbus / libsecret,
    /// minimal Wayland setups, headless CI). The previous `expect()`
    /// panicked the calling thread — typically the iced UI thread on
    /// app startup — which is unrecoverable. Callers now propagate the
    /// error to the user (e.g. "Distributor unavailable: install
    /// libsecret-tools or run with `--no-keyring`").
    pub fn for_provider(provider: &str, username: &str) -> Result<Self, KeyringError> {
        let service = format!("{SERVICE_PREFIX}{provider}");
        let entry = Entry::new(&service, username).map_err(KeyringError::from)?;
        Ok(Self {
            service,
            username: username.to_string(),
            entry,
        })
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

    /// #976 guard that runs on every CI platform and needs no keychain:
    /// keyring 3.x silently falls back to its in-memory mock (persistence
    /// `EntryOnly`) when no backend feature is enabled for the target. The
    /// real Windows / macOS / Secret Service backends persist until deleted.
    /// Building the credential builder does no IO.
    #[test]
    fn default_backend_is_a_persistent_os_keychain_not_the_mock() {
        let persistence = ::keyring::default::default_credential_builder().persistence();
        assert!(
            matches!(
                persistence,
                ::keyring::credential::CredentialPersistence::UntilDelete
            ),
            "keyring default backend is not a persistent OS keychain (the in-memory mock \
             reports EntryOnly); a backend feature is missing for this target in \
             signex-library/Cargo.toml"
        );
    }

    #[test]
    fn backend_error_names_reason_and_remedy() {
        let msg = KeyringError::Backend("no D-Bus session".into()).to_string();
        assert!(msg.contains("OS keychain unavailable"), "{msg}");
        assert!(msg.contains("no D-Bus session"), "{msg}");
        assert!(msg.contains(KEYCHAIN_HINT), "{msg}");
    }

    #[test]
    fn service_prefix_is_stable() {
        assert_eq!(SERVICE_PREFIX, "signex-distributor-");
    }

    #[test]
    fn for_provider_builds_expected_service_name() {
        // CI environments without a keyring daemon return `Err` —
        // skip rather than fail to keep the test useful on macOS /
        // Windows where the daemon is always present.
        let Ok(s) = KeyringStore::for_provider("mouser", "default") else {
            return;
        };
        assert_eq!(s.service_name(), "signex-distributor-mouser");
        assert_eq!(s.username(), "default");
    }
}
