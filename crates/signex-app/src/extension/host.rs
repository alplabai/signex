//! The installed extensions, held in the application model, and the routing
//! of `Message::Extension` to its owner.

use std::fmt;

use iced::Task;
use signex_commands::catalog::CommandMetadata;

use super::{
    Extension, ExtensionCommand, ExtensionId, ExtensionMsg, ExtensionPoints, InvalidExtensionName,
    PreferenceValues, RegisterError,
};
use crate::app::Message;

/// One installed extension and what it registered.
pub(super) struct Installed {
    pub(super) id: ExtensionId,
    pub(super) extension: Box<dyn Extension>,
    pub(super) points: ExtensionPoints,
    /// Its preference values as last loaded or saved, for its Preferences
    /// page; `None` before `load_preferences`.
    pub(super) preference_values: Option<PreferenceValues>,
}

/// The extensions a binary installed at boot. Empty in the Community binary.
///
/// Lives in the `Signex` model; `Signex::update` is the only place that
/// calls into an extension after install.
#[derive(Default)]
pub struct ExtensionHost {
    pub(super) installed: Vec<Installed>,
}

/// Why a set of extensions cannot be installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    /// An extension's name is not a valid [`ExtensionId`].
    InvalidName(InvalidExtensionName),
    /// Two extensions gave the same name.
    DuplicateName(String),
    /// An extension point (a catalog command, a panel, a row, a section or a
    /// preference) refused one of the extension's additions: the first
    /// refusal.
    Refused {
        /// The extension's name.
        extension: String,
        /// What was refused, and why.
        error: RegisterError,
    },
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(error) => fmt::Display::fmt(error, f),
            Self::DuplicateName(name) => {
                write!(f, "two extensions are named {name:?}")
            }
            Self::Refused { extension, error } => {
                write!(f, "extension {extension:?} was refused: {error}")
            }
        }
    }
}

impl std::error::Error for InstallError {}

impl ExtensionHost {
    /// Install `extensions` in order: check each name, then let each one
    /// register its additions. Refuses the whole set if a name is invalid or
    /// taken, or a point refused an addition, so a binary never runs with
    /// half its extensions.
    pub fn install(extensions: Vec<Box<dyn Extension>>) -> Result<Self, InstallError> {
        let mut installed: Vec<Installed> = Vec::with_capacity(extensions.len());
        for mut extension in extensions {
            let id = ExtensionId::new(extension.name()).map_err(InstallError::InvalidName)?;
            if installed.iter().any(|other| other.id == id) {
                return Err(InstallError::DuplicateName(id.name().to_owned()));
            }
            let mut points = ExtensionPoints::new(id.clone());
            extension.register(&mut points);
            if let Some(error) = points.first_error() {
                return Err(InstallError::Refused {
                    extension: id.name().to_owned(),
                    error,
                });
            }
            installed.push(Installed {
                id,
                extension,
                points,
                preference_values: None,
            });
        }
        Ok(Self { installed })
    }

    /// Is no extension installed?
    pub fn is_empty(&self) -> bool {
        self.installed.is_empty()
    }

    /// The installed extensions' ids, in install order.
    pub fn ids(&self) -> impl Iterator<Item = &ExtensionId> {
        self.installed.iter().map(|entry| &entry.id)
    }

    /// What each installed extension registered, in install order.
    pub fn points(&self) -> impl Iterator<Item = &ExtensionPoints> {
        self.installed.iter().map(|entry| &entry.points)
    }

    /// The installed command `id`. Only an `ext.` id can name one, and only
    /// its owner's table is searched.
    pub fn command(&self, id: &str) -> Option<&ExtensionCommand> {
        let rest = id.strip_prefix(signex_commands::EXTENSION_ID_PREFIX)?;
        let (name, _) = rest.split_once('.')?;
        self.installed
            .iter()
            .find(|entry| entry.id.name() == name)
            .and_then(|entry| entry.points.commands.get(id))
    }

    /// Every installed command, by extension in install order, then in the
    /// order each extension added them.
    pub fn commands(&self) -> impl Iterator<Item = &ExtensionCommand> {
        self.installed
            .iter()
            .flat_map(|entry| entry.points.commands.iter())
    }

    /// Hand `message` to the one extension it names: to the panel or
    /// Start-screen section it is addressed to, else to the extension
    /// itself. A message for an id that is not installed is dropped.
    pub(crate) fn route(&mut self, message: ExtensionMsg) -> Task<Message> {
        let Some(entry) = self
            .installed
            .iter_mut()
            .find(|entry| entry.id == *message.extension())
        else {
            tracing::warn!(
                extension = message.extension().name(),
                "dropped a message for an extension that is not installed"
            );
            return Task::none();
        };
        if let Some(view) = message.view().and_then(|key| entry.points.view_mut(key)) {
            return view.update(message);
        }
        // A saved preference shows on the extension's Preferences page.
        if let Some(saved) = message.payload::<super::PreferenceSaved>()
            && saved.result.is_ok()
        {
            let values = entry
                .preference_values
                .get_or_insert_with(|| entry.points.preferences.values_from(&Default::default()));
            values.set(saved.key, saved.value.clone());
        }
        entry.extension.update(message)
    }
}

impl crate::app::Signex {
    /// The model with `extensions` installed. Called once at boot, before
    /// the first update. A panel the saved dock layout names under a key
    /// an installed extension registered is docked where it was saved. The
    /// extensions' `.ftl` files join the message catalog, and their commands
    /// the keymap's names.
    pub fn with_extensions(mut self, extensions: ExtensionHost) -> Self {
        self.extensions = extensions;
        let host = &self.extensions;
        self.document_view
            .dock
            .adopt_extension_panels(|key| host.panel(key));
        if self.extensions.is_empty() {
            return self;
        }
        self.ui_state.extension_commands = self.extensions.command_names();
        if !self.extensions.message_files().is_empty() {
            self.relocalize_for_extensions();
        }
        self.ui_state.refresh_shortcut_sections();
        self
    }

    /// The installed extensions.
    pub fn extensions(&self) -> &ExtensionHost {
        &self.extensions
    }

    /// The catalog row of `command`: a built-in one, else an installed
    /// extension's. The lookup every dispatch path that must also know an
    /// extension's commands goes through.
    pub fn command_metadata(
        &self,
        command: &signex_commands::AppCommandId,
    ) -> Option<&'static CommandMetadata> {
        crate::keymap::metadata_for(command).or_else(|| {
            self.extensions
                .command(command.as_str())
                .map(ExtensionCommand::metadata)
        })
    }
}
