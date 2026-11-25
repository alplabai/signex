//! The extension points: what one extension adds to the application.
//!
//! [`Extension::register`](super::Extension::register) fills an
//! [`ExtensionPoints`] once, when the extension is installed. Each point has a
//! file of its own with its entry type and its `add` method, and is listed in
//! `docs/EXTENSION_API.md`, "Extension points". Every point refuses with one
//! type, [`RegisterError`].
//!
//! Boundaries (ADR-0006): [`CatalogCommands`] is what the application core
//! sees, so it names no iced type. The other points are GUI adapters.

mod commands;
mod error;
mod menu_rows;
mod messages;
mod panels;
mod preferences;
mod start_screen;

pub use commands::{CatalogCommands, ExtensionCommand};
pub use error::RegisterError;
pub use menu_rows::{ActiveBarRow, MenuRoot, MenuRow, MenuRows, PcbBarGroup};
pub use messages::Messages;
pub use panels::{ExtensionPanelKind, Panels};
pub use preferences::{PreferenceValues, Preferences};
pub use start_screen::{StartScreenSections, StartScreenSlot};

use super::{ExtensionId, ExtensionView};

/// Everything one extension registers, kept apart from every other
/// extension's.
#[derive(Debug)]
#[non_exhaustive]
pub struct ExtensionPoints {
    owner: ExtensionId,
    /// Catalog commands, each with an id under the owner's `ext.<name>.`
    /// prefix.
    pub commands: CatalogCommands,
    /// Dock panels.
    pub panels: Panels,
    /// Rows in the menu bar and the Active Bar.
    pub menu_rows: MenuRows,
    /// Sections in the Start screen's extension slot.
    pub start_screen: StartScreenSections,
    /// Preferences.
    pub preferences: Preferences,
    /// The extension's own `.ftl` message files.
    pub messages: Messages,
}

/// Claim `id` for `owner`: refused when it is not under the owner's prefix,
/// is not an extension id (`ext.<name>.<snake_case>`, the one rule,
/// [`signex_commands::is_extension_id`]) or is `taken`. A refusal is also
/// pushed to `errors`, which install reads.
fn claim(
    owner: &ExtensionId,
    id: &str,
    taken: bool,
    errors: &mut Vec<RegisterError>,
) -> Result<(), RegisterError> {
    let refused = if !owner.owns(id) {
        RegisterError::NotOwned(id.to_owned())
    } else if !signex_commands::is_extension_id(id) {
        RegisterError::InvalidId(id.to_owned())
    } else if taken {
        RegisterError::Duplicate(id.to_owned())
    } else {
        return Ok(());
    };
    errors.push(refused.clone());
    Err(refused)
}

impl ExtensionPoints {
    pub(crate) fn new(owner: ExtensionId) -> Self {
        Self {
            commands: CatalogCommands::new(owner.clone()),
            panels: Panels::new(owner.clone()),
            menu_rows: MenuRows::new(owner.clone()),
            start_screen: StartScreenSections::new(owner.clone()),
            preferences: Preferences::new(owner.clone()),
            messages: Messages::new(owner.clone()),
            owner,
        }
    }

    /// The extension these registrations belong to. An extension keeps a
    /// copy to address its own [`ExtensionMsg`](super::ExtensionMsg)s.
    pub fn owner(&self) -> &ExtensionId {
        &self.owner
    }

    /// Did the extension register nothing at all?
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
            && self.panels.is_empty()
            && self.menu_rows.is_empty()
            && self.start_screen.is_empty()
            && self.preferences.is_empty()
            && self.messages.is_empty()
    }

    /// The first addition a point refused, if any. Two checks need the
    /// whole registration and are made here: a panel and a Start-screen
    /// section share one key space, since a message addressed to a key must
    /// reach one surface, so a section key a panel already uses is refused;
    /// and a menu or Active Bar row must run a command the extension added,
    /// in any order within `register`.
    pub(crate) fn first_error(&self) -> Option<RegisterError> {
        let recorded = self
            .commands
            .errors
            .iter()
            .chain(&self.panels.errors)
            .chain(&self.menu_rows.errors)
            .chain(&self.start_screen.errors)
            .chain(&self.preferences.errors)
            .chain(&self.messages.errors)
            .next()
            .cloned();
        let shared_key = || {
            self.start_screen
                .keys()
                .find(|key| self.panels.find(key).is_some())
                .map(|key| RegisterError::Duplicate(key.to_owned()))
        };
        let unregistered_row = || {
            self.menu_rows
                .commands()
                .find(|command| self.commands.get(command.as_str()).is_none())
                .map(|command| RegisterError::NotRegistered(command.as_str().to_owned()))
        };
        recorded.or_else(shared_key).or_else(unregistered_row)
    }

    /// The panel or section `key`'s view hooks, to update it.
    pub(crate) fn view_mut(&mut self, key: &str) -> Option<&mut (dyn ExtensionView + 'static)> {
        if self.panels.find(key).is_some() {
            return self.panels.view_mut(key);
        }
        self.start_screen.view_mut(key)
    }
}
