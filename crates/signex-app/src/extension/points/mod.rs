//! The extension points: what one extension adds to the application.
//!
//! [`Extension::register`](super::Extension::register) fills an
//! [`ExtensionPoints`] once, when the extension is installed. Each point has a
//! file of its own. A point not open yet is empty; the change that opens it
//! adds its entry type and its `add` method there and lists it in
//! `docs/EXTENSION_API.md`, "Changes by version".
//!
//! Boundaries (ADR-0006): [`CatalogCommands`] is what the application core
//! sees, so it names no iced type. The other points are GUI adapters.

mod commands;
mod menu_rows;
mod panels;
mod preferences;
mod start_screen;

pub use commands::{CatalogCommands, CommandRegistrationError, ExtensionCommand};
pub use menu_rows::MenuRows;
pub use panels::Panels;
pub use preferences::Preferences;
pub use start_screen::StartScreenSections;

use super::ExtensionId;

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
}

impl ExtensionPoints {
    pub(crate) fn new(owner: ExtensionId) -> Self {
        Self {
            commands: CatalogCommands::new(owner.clone()),
            owner,
            panels: Panels::default(),
            menu_rows: MenuRows::default(),
            start_screen: StartScreenSections::default(),
            preferences: Preferences::default(),
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
    }
}
