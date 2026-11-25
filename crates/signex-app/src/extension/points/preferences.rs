//! Preferences an extension adds. Each key sits under its owner's
//! `ext.<name>.` prefix, so it never collides with a built-in preference.
//!
//! The values live in `prefs.json`, in its `extensions` object, under their
//! full keys (`fonts::extension_prefs`). At launch each extension receives
//! its values once, through `Extension::preferences_loaded`; it saves one
//! with [`crate::extension::save_preference`], which writes off the update
//! thread.

use std::collections::BTreeMap;

use super::{RegisterError, claim};
use crate::extension::ExtensionId;

/// The preferences one extension adds: each key with its default value.
#[derive(Debug)]
#[non_exhaustive]
pub struct Preferences {
    owner: ExtensionId,
    entries: Vec<(&'static str, serde_json::Value)>,
    pub(super) errors: Vec<RegisterError>,
}

/// One extension's preference values: the stored value of each key it
/// added, else that key's default.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreferenceValues(BTreeMap<&'static str, serde_json::Value>);

impl PreferenceValues {
    /// The value of `key`; `None` for a key the extension did not add.
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.0.get(key)
    }

    /// Is there no value at all?
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Preferences {
    pub(super) fn new(owner: ExtensionId) -> Self {
        Self {
            owner,
            entries: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Add the preference `key` (`ext.<name>.<local>`), read as `default`
    /// until a value is saved. Refused, and recorded so install refuses the
    /// set, when the key is not under the owner's prefix or was added
    /// already.
    pub fn add(
        &mut self,
        key: &'static str,
        default: serde_json::Value,
    ) -> Result<(), RegisterError> {
        let taken = self.keys().any(|taken| taken == key);
        claim(&self.owner, key, taken, &mut self.errors)?;
        self.entries.push((key, default));
        Ok(())
    }

    /// Were no preferences added?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The keys, in the order they were added.
    pub fn keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(|(key, _)| *key)
    }

    /// The value of each key from `stored` (the `extensions` object of
    /// `prefs.json`), else its default. A stored key the extension did not
    /// add is not read.
    pub(crate) fn values_from(
        &self,
        stored: &BTreeMap<String, serde_json::Value>,
    ) -> PreferenceValues {
        PreferenceValues(
            self.entries
                .iter()
                .map(|(key, default)| (*key, stored.get(*key).unwrap_or(default).clone()))
                .collect(),
        )
    }
}
