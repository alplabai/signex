//! The extension API: how a binary built on `signex-app` adds to the
//! application (#2495).
//!
//! Extensions are trusted Rust code linked at build time. A binary hands its
//! extensions to [`crate::launch::run`] instead of running the Community
//! `main`; the Community binary hands none. Each one registers its additions
//! once, through [`ExtensionPoints`], and then receives only the
//! `Message::Extension` messages addressed to it. The whole contract,
//! versioning included, is in `docs/EXTENSION_API.md`.

mod dispatch;
mod host;
mod id;
mod message;
pub mod points;
mod prefs;
mod surfaces;
mod view;

pub use host::{ExtensionHost, InstallError};
pub use id::{ExtensionId, InvalidExtensionName};
pub use message::ExtensionMsg;
pub use points::{ExtensionCommand, ExtensionPoints, PreferenceValues, RegisterError};
pub use prefs::PreferenceSaved;
pub use view::{ExtensionView, ViewContext};
// The one extension id rule, the catalog row types a command is added with,
// and its handler's refusal.
pub use signex_commands::catalog::{CommandFlags, CommandMetadata};
pub use signex_commands::is_extension_id;
pub use signex_i18n_ids::Translatable;

use iced::Task;

use crate::app::Message;

/// The version of this API. It goes up by one in the release that changes
/// the API in a way an extension written for the previous version might not
/// compile against or might behave differently with; each version has a
/// section in `docs/EXTENSION_API.md`, "Changes by version".
pub const API_VERSION: u32 = 1;

/// One extension, installed when the application boots.
///
/// The extension object is its state: it lives in the application model, and
/// only the update loop calls into it after install, so [`Extension::update`]
/// is its one mutation site. Long or blocking work goes in the returned
/// `Task`, never inline.
pub trait Extension: Send + 'static {
    /// The extension's name, which every id it adds starts from
    /// (`ext.<name>.`). A lowercase ASCII letter, then lowercase ASCII
    /// letters, digits and `_`; unique among the installed extensions.
    fn name(&self) -> &str;

    /// Called once at install: add commands, panels, rows, sections and
    /// preferences to `points`. Adds nothing by default.
    fn register(&mut self, points: &mut ExtensionPoints) {
        let _ = points;
    }

    /// Called with each `Message::Extension` addressed to this extension,
    /// and with no other message. Does nothing by default.
    fn update(&mut self, message: ExtensionMsg) -> Task<Message> {
        let _ = message;
        Task::none()
    }

    /// Called once at launch, after install and before the first window,
    /// with the value of each preference the extension added (the stored
    /// one, else its default). Not called for an extension that added no
    /// preference. Does nothing by default.
    fn preferences_loaded(&mut self, values: &PreferenceValues) {
        let _ = values;
    }
}

#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod surface_tests;
#[cfg(test)]
pub(crate) mod test_extension;
#[cfg(test)]
mod tests;
