//! Saving an extension preference: a `Task` that writes `prefs.json` off
//! the update thread and reports back to the extension.

use iced::Task;

use super::ExtensionMsg;
use super::points::{PreferenceValues, RegisterError};
use crate::app::Message;

/// The answer to [`PreferenceValues::save`], sent to the extension as the
/// payload of an [`ExtensionMsg`]: the key it saved and whether the write
/// landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceSaved {
    pub key: &'static str,
    pub result: Result<(), String>,
}

impl PreferenceValues {
    /// Save `value` as the preference `key` of the extension these values
    /// belong to. The write runs on a blocking worker, in the order the
    /// saves were made, and the quit path waits for it; its outcome comes
    /// back to the extension as a [`PreferenceSaved`]. Refused when `key` is
    /// not under the extension's `ext.<name>.` prefix, is not an extension
    /// id, or is not a key the extension added with `Preferences::add`.
    pub fn save(
        &self,
        key: &str,
        value: serde_json::Value,
    ) -> Result<Task<Message>, RegisterError> {
        if !self.owner().owns(key) {
            return Err(RegisterError::NotOwned(key.to_owned()));
        }
        if !signex_commands::is_extension_id(key) {
            return Err(RegisterError::InvalidId(key.to_owned()));
        }
        let Some(key) = self.added_key(key) else {
            return Err(RegisterError::NotRegistered(key.to_owned()));
        };
        let owner = self.owner().clone();
        Ok(crate::fonts::background_pref_merge(
            crate::fonts::EXTENSION_PREFS_KEY,
            move || crate::fonts::write_extension_pref(key, value),
            move |result| {
                Message::Extension(ExtensionMsg::new(
                    owner.clone(),
                    PreferenceSaved { key, result },
                ))
            },
        ))
    }
}
