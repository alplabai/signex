//! The installed extensions, held in the application model, and the routing
//! of `Message::Extension` to its owner.

use std::fmt;

use iced::Task;

use super::{Extension, ExtensionId, ExtensionMsg, ExtensionPoints, InvalidExtensionName};
use crate::app::Message;

/// One installed extension and what it registered.
struct Installed {
    id: ExtensionId,
    extension: Box<dyn Extension>,
    points: ExtensionPoints,
}

/// The extensions a binary installed at boot. Empty in the Community binary.
///
/// Lives in the `Signex` model; `Signex::update` is the only place that
/// calls into an extension after install.
#[derive(Default)]
pub struct ExtensionHost {
    installed: Vec<Installed>,
}

/// Why a set of extensions cannot be installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    /// An extension's name is not a valid [`ExtensionId`].
    InvalidName(InvalidExtensionName),
    /// Two extensions gave the same name.
    DuplicateName(String),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(error) => fmt::Display::fmt(error, f),
            Self::DuplicateName(name) => {
                write!(f, "two extensions are named {name:?}")
            }
        }
    }
}

impl std::error::Error for InstallError {}

impl ExtensionHost {
    /// Install `extensions` in order: check each name, then let each one
    /// register its additions. Refuses the whole set if a name is invalid or
    /// taken, so a binary never runs with half its extensions.
    pub fn install(extensions: Vec<Box<dyn Extension>>) -> Result<Self, InstallError> {
        let mut installed: Vec<Installed> = Vec::with_capacity(extensions.len());
        for mut extension in extensions {
            let id = ExtensionId::new(extension.name()).map_err(InstallError::InvalidName)?;
            if installed.iter().any(|other| other.id == id) {
                return Err(InstallError::DuplicateName(id.name().to_owned()));
            }
            let mut points = ExtensionPoints::new(id.clone());
            extension.register(&mut points);
            installed.push(Installed {
                id,
                extension,
                points,
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

    /// Hand `message` to the one extension it names. A message for an id
    /// that is not installed is dropped.
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
        entry.extension.update(message)
    }
}

impl crate::app::Signex {
    /// The model with `extensions` installed. Called once at boot, before
    /// the first update.
    pub fn with_extensions(mut self, extensions: ExtensionHost) -> Self {
        self.extensions = extensions;
        self
    }

    /// The installed extensions.
    pub fn extensions(&self) -> &ExtensionHost {
        &self.extensions
    }
}
