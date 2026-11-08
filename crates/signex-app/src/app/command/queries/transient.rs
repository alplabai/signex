//! A headless query on a project sheet that is not loaded (#2172).
//!
//! The sheet is read and held in a tabless engine only while the query
//! answers, the way Replace does for a closed sheet, then dropped: it owns no
//! path afterwards, so a later open reads the disk. No tab is made and no
//! window, focus or selection is touched. A sheet that cannot be read is named
//! in Messages and left out; `read_document` then refuses it with the reason.

use std::path::{Path, PathBuf};

use iced::Task;

use super::{CommandArgs, CommandOutcome, OperandValue, query_command_ids};
use crate::app::document_ids::{DocumentEntry, DocumentKind};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// Read and parse the sheet at `path`.
fn read_sheet(path: &Path) -> Result<signex_types::schematic::SchematicSheet, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    signex_types::format::SnxSchematic::parse(&text)
        .map(|parsed| parsed.sheet)
        .map_err(|error| error.to_string())
}

impl Signex {
    /// [`Self::run_query`] for the headless entry: the unloaded project sheets
    /// the query needs are loaded for the answer and dropped after it.
    pub(crate) fn run_query_headless(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Option<(CommandOutcome, Task<Message>)> {
        if !query_command_ids().any(|query| query == command.as_str()) {
            return None;
        }
        self.read_wanted_document_headers_now();
        let installed = self.install_for_query(command, args);
        let answered = self.run_query(command, args);
        for path in installed {
            self.document_state.document_ids.end_transient(&path);
            // Not `remove_engine`: that moves `documents_epoch`, which cancels
            // a person's running Annotate, Replace All or project read.
            self.document_state.engines.remove(&path);
        }
        answered
    }

    /// The unloaded schematic documents `command` reads: the one a
    /// `read_document` names, every one for `resolve` and `find_items`.
    fn unloaded_sheets_wanted(
        &self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Vec<DocumentEntry> {
        let unloaded_sheet = |entry: &DocumentEntry| {
            entry.key.is_none() && entry.kind == DocumentKind::Schematic && entry.path.is_some()
        };
        match command.as_str() {
            "read_document" => match args.operand("document") {
                Some(OperandValue::Document(target)) => self
                    .resolve_document_target(target)
                    .ok()
                    .filter(unloaded_sheet)
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            },
            "resolve" | "find_items" => self
                .document_entries()
                .into_iter()
                .filter(unloaded_sheet)
                .collect(),
            _ => Vec::new(),
        }
    }

    fn install_for_query(&mut self, command: &AppCommandId, args: &CommandArgs) -> Vec<PathBuf> {
        let mut installed = Vec::new();
        for entry in self.unloaded_sheets_wanted(command, args) {
            let Some(path) = entry.path else { continue };
            let engine = read_sheet(&path).and_then(|sheet| {
                signex_engine::Engine::new_with_path(sheet, Some(path.clone()))
                    .map_err(|error| error.to_string())
            });
            match engine {
                Ok(engine) => {
                    self.document_state
                        .document_ids
                        .begin_transient(path.clone());
                    // Not `insert_engine`: that moves `documents_epoch` (see the drop below).
                    self.document_state.engines.insert(path.clone(), engine);
                    installed.push(path);
                }
                Err(reason) => crate::diagnostics::log_warning(format!(
                    "{} could not be read for the query: {reason}",
                    path.display()
                )),
            }
        }
        installed
    }
}
