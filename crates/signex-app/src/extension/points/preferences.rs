//! Preferences an extension adds. Each key sits under its owner's
//! `ext.<name>.` prefix, so it never collides with a built-in preference.
//!
//! The values live in `prefs.json`, in its `extensions` object, under their
//! full keys (`fonts::extension_prefs`). At launch each extension receives
//! its values once, through `Extension::preferences_loaded`; it saves one
//! of the keys it added with `PreferenceValues::save`, which writes off the
//! update thread.

use std::collections::BTreeMap;

use signex_i18n::MessageId;

use super::{RegisterError, claim};
use crate::extension::ExtensionId;

/// The preferences one extension adds: each key with its label, its help
/// and its default value.
#[derive(Debug)]
#[non_exhaustive]
pub struct Preferences {
    owner: ExtensionId,
    entries: Vec<PreferenceEntry>,
    pub(super) errors: Vec<RegisterError>,
}

/// One preference as the extension added it.
#[derive(Debug, Clone, PartialEq)]
struct PreferenceEntry {
    key: &'static str,
    texts: PreferenceTexts,
    default: serde_json::Value,
}

/// What a preference's row on the extension's Preferences page says: its
/// label and its help, both messages of the extension's own `.ftl` files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreferenceTexts {
    pub label: MessageId,
    pub help: Option<MessageId>,
}

/// One extension's preference values: the stored value of each key it
/// added, else that key's default. Only the application makes one, for the
/// extension that added the keys; the extension keeps it to save a value
/// later (`PreferenceValues::save`).
#[derive(Debug, Clone, PartialEq)]
pub struct PreferenceValues {
    owner: ExtensionId,
    values: BTreeMap<&'static str, serde_json::Value>,
}

impl PreferenceValues {
    /// The value of `key`; `None` for a key the extension did not add.
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.values.get(key)
    }

    /// Is there no value at all?
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The extension the values belong to.
    pub fn owner(&self) -> &ExtensionId {
        &self.owner
    }

    /// The key `key` as the extension added it; `None` for one it did not.
    pub(crate) fn added_key(&self, key: &str) -> Option<&'static str> {
        self.values.get_key_value(key).map(|(added, _)| *added)
    }

    /// Each key with its value, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &serde_json::Value)> {
        self.values.iter().map(|(key, value)| (*key, value))
    }

    /// Record `value` for the added key `key` (a save that landed); a key the
    /// extension did not add is left out.
    pub(crate) fn set(&mut self, key: &str, value: serde_json::Value) {
        if let Some(slot) = self.values.get_mut(key) {
            *slot = value;
        }
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
    /// until a value is saved. Its row on the extension's Preferences page
    /// reads `label`, with `help` under it; both are the owner's own
    /// messages (`ext-<name>-…`, see `Messages`). Refused, and recorded so
    /// install refuses the set, when the key is not under the owner's
    /// prefix or was added already, or when `label` or `help` is not one of
    /// the owner's message ids (`RegisterError::ForeignMessage`).
    pub fn add(
        &mut self,
        key: &'static str,
        label: MessageId,
        help: Option<MessageId>,
        default: serde_json::Value,
    ) -> Result<(), RegisterError> {
        let taken = self.keys().any(|taken| taken == key);
        claim(&self.owner, key, taken, &mut self.errors)?;
        let prefix = super::Messages::prefix_of(&self.owner);
        if let Some(foreign) = std::iter::once(label)
            .chain(help)
            .find(|id| !id.as_str().starts_with(&prefix))
        {
            let error = RegisterError::ForeignMessage(foreign.as_str().to_owned());
            self.errors.push(error.clone());
            return Err(error);
        }
        self.entries.push(PreferenceEntry {
            key,
            texts: PreferenceTexts { label, help },
            default,
        });
        Ok(())
    }

    /// The label and help of the added key `key`.
    pub fn texts(&self, key: &str) -> Option<PreferenceTexts> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.texts)
    }

    /// Were no preferences added?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The keys, in the order they were added.
    pub fn keys(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(|entry| entry.key)
    }

    /// The value of each key from `stored` (the `extensions` object of
    /// `prefs.json`), else its default. A stored key the extension did not
    /// add is not read.
    pub(crate) fn values_from(
        &self,
        stored: &BTreeMap<String, serde_json::Value>,
    ) -> PreferenceValues {
        PreferenceValues {
            owner: self.owner.clone(),
            values: self
                .entries
                .iter()
                .map(|entry| {
                    let value = stored.get(entry.key).unwrap_or(&entry.default);
                    (entry.key, value.clone())
                })
                .collect(),
        }
    }
}
