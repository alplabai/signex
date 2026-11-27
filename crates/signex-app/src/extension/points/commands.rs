//! Catalog commands an extension adds.
//!
//! The application core reads this table (ADR-0006), so this file names no
//! iced type and no GUI state. A command's id is `ext.<name>.<command>`
//! under its owner's prefix, never in `scripts/catalog-ids.txt`. Headless runs
//! follow the core's rules: the row's flags say whether the command runs
//! headless or only in a window, and its handler is UI-free either way.

use std::any::Any;
use std::fmt;

use signex_commands::AppCommandId;
use signex_commands::catalog::{CommandMetadata, HeadlessUse};
use signex_i18n_ids::Translatable;

use super::RegisterError;
use crate::app::command::CommandArgs;
use crate::extension::{ExtensionId, ExtensionMsg};

/// What a command's handler returns: the message for its owner's
/// [`Extension::update`](crate::extension::Extension::update), or the reason
/// it did nothing.
type Handler = Box<dyn Fn(&CommandArgs) -> Result<ExtensionMsg, Translatable> + Send + Sync>;

/// One command an extension added: its catalog row and its handler.
pub struct ExtensionCommand {
    metadata: &'static CommandMetadata,
    handler: Handler,
}

impl ExtensionCommand {
    /// The command's catalog row.
    pub fn metadata(&self) -> &'static CommandMetadata {
        self.metadata
    }

    /// Run the handler on `args`.
    pub(crate) fn handle(&self, args: &CommandArgs) -> Result<ExtensionMsg, Translatable> {
        (self.handler)(args)
    }
}

impl fmt::Debug for ExtensionCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtensionCommand")
            .field("id", &self.metadata.id)
            .finish_non_exhaustive()
    }
}

/// The catalog commands one extension adds, in the order it added them.
pub struct CatalogCommands {
    owner: ExtensionId,
    commands: Vec<ExtensionCommand>,
    pub(super) errors: Vec<RegisterError>,
}

impl CatalogCommands {
    pub(super) fn new(owner: ExtensionId) -> Self {
        Self {
            owner,
            commands: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Add the command `metadata` describes. `handler` runs each time the
    /// command is dispatched, from any surface: it returns the payload of the
    /// [`ExtensionMsg`] the owner's `update` then receives, or the reason the
    /// command did nothing. It reads only `args` and names no GUI state.
    ///
    /// A refused row is not added, and install refuses the whole set of
    /// extensions with the first refusal (`InstallError::Refused`).
    pub fn add<T, F>(
        &mut self,
        metadata: &'static CommandMetadata,
        handler: F,
    ) -> Result<(), RegisterError>
    where
        T: Any + fmt::Debug + Send + Sync,
        F: Fn(&CommandArgs) -> Result<T, Translatable> + Send + Sync + 'static,
    {
        if let Err(error) = self.check(metadata) {
            self.errors.push(error.clone());
            return Err(error);
        }
        let owner = self.owner.clone();
        self.commands.push(ExtensionCommand {
            metadata,
            handler: Box::new(move |args| {
                handler(args).map(|payload| ExtensionMsg::new(owner.clone(), payload))
            }),
        });
        Ok(())
    }

    /// The checks of [`Self::add`], in the order their errors are reported.
    fn check(&self, metadata: &CommandMetadata) -> Result<(), RegisterError> {
        use RegisterError as Error;
        let raw = metadata.id.to_owned();
        let built_in = |id: &str| crate::keymap::metadata_for_str(id).is_some();
        if built_in(metadata.id) {
            return Err(Error::ShadowsBuiltIn(raw));
        }
        if !self.owner.owns(metadata.id) {
            return Err(Error::NotOwned(raw));
        }
        let id = AppCommandId::new(metadata.id).map_err(|_| Error::InvalidId(raw.clone()))?;
        if id.extension_local().is_some_and(built_in) {
            return Err(Error::ShadowsBuiltIn(raw));
        }
        if self.get(metadata.id).is_some() {
            return Err(Error::Duplicate(raw));
        }
        if !metadata.operands.is_empty() {
            return Err(Error::TakesOperands(raw));
        }
        if metadata.flags.mutates_doc || metadata.flags.undoable {
            return Err(Error::MutatesDocument(raw));
        }
        match metadata.headless_use() {
            Some(HeadlessUse::WindowOnly | HeadlessUse::Runs) => Ok(()),
            None => Err(Error::HeadlessUndecided(raw)),
        }
    }

    /// The command added under `id`.
    pub fn get(&self, id: &str) -> Option<&ExtensionCommand> {
        self.commands
            .iter()
            .find(|command| command.metadata.id == id)
    }

    /// The commands added, in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = &ExtensionCommand> {
        self.commands.iter()
    }

    /// Were no commands added?
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl fmt::Debug for CatalogCommands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CatalogCommands")
            .field("owner", &self.owner)
            .field("commands", &self.commands)
            .field("errors", &self.errors)
            .finish()
    }
}
