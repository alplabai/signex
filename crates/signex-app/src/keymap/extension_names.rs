//! What the keyboard-shortcut surfaces and the palette show for an
//! extension's command (#2495): the extension's name and the command's own
//! label. A binding to an extension command that no installed extension
//! adds stays in its profile (`docs/DECISIONS.md`, 2026-11-25) and shows as
//! unavailable.

use std::collections::BTreeMap;

use signex_i18n::{I18n, MessageId, shortcut_ids};

use super::AppCommandId;

/// One installed extension command, as the keymap surfaces name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionCommandName {
    /// The extension's name.
    pub extension: String,
    /// The command's label message.
    pub label: MessageId,
    /// The command's catalog category.
    pub category: &'static str,
}

/// The installed extensions' commands, by id: plain data the UI state keeps
/// so the keymap rows can be built without the extensions themselves.
/// Empty when no extension is installed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtensionCommandNames(BTreeMap<AppCommandId, ExtensionCommandName>);

impl ExtensionCommandNames {
    /// Name `command` as the installed extension `name` added it.
    pub fn insert(&mut self, command: AppCommandId, name: ExtensionCommandName) {
        self.0.insert(command, name);
    }

    /// The installed command `command`, if one is.
    pub fn get(&self, command: &AppCommandId) -> Option<&ExtensionCommandName> {
        self.0.get(command)
    }

    /// The installed commands' ids, in id order.
    pub fn ids(&self) -> impl Iterator<Item = &AppCommandId> {
        self.0.keys()
    }

    /// Is no extension command installed?
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// What a surface calls the extension command `command`: "<extension>:
    /// <label>" when an installed extension added it, else the id marked
    /// unavailable. `None` for a built-in command.
    pub fn label_in(&self, i18n: &I18n, command: &AppCommandId) -> Option<String> {
        if !command.is_extension() {
            return None;
        }
        Some(match self.get(command) {
            Some(name) => i18n.message_plain(
                shortcut_ids::EXTENSION_COMMAND_LABEL,
                &[
                    ("extension", name.extension.as_str().into()),
                    ("command", i18n.message_plain(name.label, &[]).into()),
                ],
            ),
            None => i18n.message_plain(
                shortcut_ids::EXTENSION_COMMAND_UNAVAILABLE,
                &[("command", command.as_str().into())],
            ),
        })
    }
}
