//! Saving an extension preference: a `Task` that writes `prefs.json` off
//! the update thread and reports back to the extension.

use iced::Task;

use super::points::RegisterError;
use super::{ExtensionId, ExtensionMsg};
use crate::app::Message;

/// The answer to [`save_preference`], sent to the extension as the payload
/// of an [`ExtensionMsg`]: the key it saved and whether the write landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceSaved {
    pub key: &'static str,
    pub result: Result<(), String>,
}

/// Save `value` as the preference `key` of the extension `owner`. The write
/// runs on a blocking worker, in the order the saves were made, and the quit
/// path waits for it; its outcome comes back to `owner` as a
/// [`PreferenceSaved`]. Refused when `key` is not under the owner's
/// `ext.<name>.` prefix.
pub fn save_preference(
    owner: &ExtensionId,
    key: &'static str,
    value: serde_json::Value,
) -> Result<Task<Message>, RegisterError> {
    if !owner.owns(key) {
        return Err(RegisterError::NotOwned(key.to_owned()));
    }
    let owner = owner.clone();
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
