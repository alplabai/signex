//! Annotate + duplicate-designator reset handlers. Split from `handlers/erc.rs`.

use iced::Task;

use super::super::super::*;
use super::annotate_job::PassKind;

/// Annotate a sheet that exists only on disk and encode it for the write
/// worker. `None` when nothing changed or the sheet failed; every failure is
/// logged with the path. Only for sheets with no engine at all — an engine (open
/// or parked) holds the user's unsaved edits and is annotated in place instead
/// (#669).
pub(super) fn annotate_unopened_for_write(
    sheet: signex_types::schematic::SchematicSheet,
    path: &std::path::Path,
    mode: signex_engine::AnnotateMode,
    next_by_prefix: &mut std::collections::HashMap<String, u32>,
    scope: signex_engine::AnnotateScope<'_>,
    assigned: &mut std::collections::HashMap<usize, String>,
) -> Option<Vec<u8>> {
    let context = format!("Annotate failed on unopened sheet '{}'", path.display());
    let mut engine = match signex_engine::Engine::new(sheet) {
        Ok(engine) => engine,
        Err(error) => {
            crate::diagnostics::log_error(&context, &anyhow::Error::new(error));
            return None;
        }
    };
    match engine.annotate_in_project(mode, next_by_prefix, scope, assigned) {
        Ok(true) => {}
        Ok(false) => return None,
        Err(error) => {
            crate::diagnostics::log_error(&context, &anyhow::Error::new(error));
            return None;
        }
    }
    encode_unopened("Annotate", path, &engine)
}

/// The sheet `engine` holds as `.snxsch` bytes, for the write worker. `None`
/// (logged with the path, and the objects a refusal names) when it cannot be
/// encoded.
pub(super) fn encode_unopened(
    label: &str,
    path: &std::path::Path,
    engine: &signex_engine::Engine,
) -> Option<Vec<u8>> {
    match engine.serialize() {
        Ok(text) => Some(text.into_bytes()),
        Err(error) => {
            let error = anyhow::Error::new(error);
            crate::diagnostics::log_error(
                &format!("{label} could not encode '{}'", path.display()),
                &error,
            );
            crate::app::handlers::document_files::log_foreign_column_offenders(&error);
            None
        }
    }
}

/// Annotate a sheet that exists only on disk, in memory, WITHOUT writing it.
/// Returns the engine (path set) when the pass changed it, so the caller can
/// park it dirty like any other unsaved sheet (#669): nothing reaches disk
/// until the user saves, and the engine's history makes the change undoable.
/// Re-prefix uses this; plain Annotate uses `annotate_unopened_for_write`.
/// `Err` means the sheet failed and was logged.
pub(super) fn annotate_unopened_in_memory(
    sheet: signex_types::schematic::SchematicSheet,
    path: &std::path::Path,
    mode: signex_engine::AnnotateMode,
    next_by_prefix: &mut std::collections::HashMap<String, u32>,
    scope: signex_engine::AnnotateScope<'_>,
    assigned: &mut std::collections::HashMap<usize, String>,
) -> Result<Option<signex_engine::Engine>, PassFailed> {
    let context = format!("Annotate failed on unopened sheet '{}'", path.display());
    let mut engine = match signex_engine::Engine::new(sheet) {
        Ok(engine) => engine,
        Err(error) => {
            crate::diagnostics::log_error(&context, &anyhow::Error::new(error));
            return Err(PassFailed);
        }
    };
    engine.set_path(Some(path.to_path_buf()));
    match engine.annotate_in_project(mode, next_by_prefix, scope, assigned) {
        Ok(true) => Ok(Some(engine)),
        Ok(false) => Ok(None),
        Err(error) => {
            crate::diagnostics::log_error(&context, &anyhow::Error::new(error));
            Err(PassFailed)
        }
    }
}

/// A sheet of the pass failed (already logged with its path).
pub(super) struct PassFailed;

/// Sheets in the project written by a newer minor format version. This build
/// holds them read-only (every save is refused), so the bulk passes leave them
/// untouched: editing one would only strand a dirty sheet that can never be
/// saved. Their designators still count as taken in the annotate seed pass.
pub(super) fn read_only_sheets(
    documents: &DocumentState,
    project_set: &crate::app::project_sheets::ProjectSheetSet,
) -> std::collections::BTreeSet<std::path::PathBuf> {
    project_set
        .sheets
        .iter()
        .filter(|(path, sheet)| {
            documents
                .engines
                .get(*path)
                .map_or(sheet.is_read_only(), |engine| {
                    engine.document().is_read_only()
                })
        })
        .map(|(path, _)| path.clone())
        .collect()
}

/// The sentence appended to a bulk pass's result when sheets were skipped.
fn skipped_summary(skipped: &std::collections::BTreeSet<std::path::PathBuf>) -> String {
    let names: Vec<String> = skipped
        .iter()
        .map(|p| {
            p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        })
        .collect();
    format!(
        "{} sheet(s) were written by a newer Signex and are read-only in this build, so \
         they were left unchanged: {}. Open the project in the newer version to number them.",
        names.len(),
        names.join(", ")
    )
}

impl Signex {
    /// Whether a bulk pass must leave the tab-less sheet at `path` alone
    /// because a History restore of it is still running (even a cancelled
    /// one) or a read of it is in flight: the write would race the worker,
    /// and a read landing later would install the pre-pass content over it.
    /// The reason leads the Messages line.
    pub(super) fn unopened_write_blocked(&self, pass: &str, path: &std::path::Path) -> bool {
        let reason = if self.document_state.pending_restores.contains(path) {
            "a History restore of it is still running"
        } else if self.document_state.pending_opens.contains(path) {
            "a read of it is still in flight"
        } else if self.document_state.document_saves.is_saving(path) {
            "a save of it is still running"
        } else {
            return false;
        };
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        crate::diagnostics::log_warning(format!("{pass}: {name} was left unchanged, {reason}"));
        true
    }

    /// Tell the user which sheets a bulk pass skipped: Messages panel plus a
    /// dialog, because a quiet log line would read as "everything was done".
    pub(super) fn report_read_only_skipped(
        &mut self,
        pass: &str,
        skipped: &std::collections::BTreeSet<std::path::PathBuf>,
    ) {
        if skipped.is_empty() {
            return;
        }
        let summary = skipped_summary(skipped);
        crate::diagnostics::log_warning(format!("{pass}: {summary}"));
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::sheet_edit(
            format!("{pass} skipped read-only sheets. {summary}"),
        ));
    }

    /// Refuse a project-wide rewrite up front, with a notice, when any of
    /// `sheets` lies in a project whose folder is being renamed: the unopened
    /// sheets would be written into the old folder. True when refused.
    pub(super) fn refuse_annotate_while_project_moves<'a>(
        &mut self,
        sheets: impl IntoIterator<Item = &'a std::path::Path>,
        what: &str,
    ) -> bool {
        let Some(path) = sheets
            .into_iter()
            .find(|p| self.path_is_in_moving_project(p))
        else {
            return false;
        };
        let detail = format!(
            "{what} was not run: {} is in a project whose folder is being renamed. Try again once the rename has finished.",
            path.display()
        );
        crate::diagnostics::log_warning(detail.clone());
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::annotate(detail));
        true
    }

    /// Refuse a project-wide rewrite up front when a rename or delete holds
    /// any of `sheets`, a declared page or the project folder: writing the
    /// rest would leave the designators on disk inconsistent (#859). True when
    /// refused.
    pub(super) fn refuse_annotate_while_path_op<'a>(
        &mut self,
        sheets: impl IntoIterator<Item = &'a std::path::Path>,
        what: &str,
    ) -> bool {
        let Err(refusal) = self.refuse_project_wide_while_path_op(sheets, "rewritten") else {
            return false;
        };
        let detail = format!("{what} was not run: {refusal}");
        crate::diagnostics::log_warning(detail.clone());
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::annotate(detail));
        true
    }

    /// Annotate the whole project. The sheets are read, numbered and written
    /// by [`Self::begin_project_run`]'s three steps; nothing blocks `update`.
    pub(crate) fn handle_annotate(&mut self, mode: signex_engine::AnnotateMode) -> Task<Message> {
        self.begin_project_run(PassKind::Annotate(mode))
    }

    pub(crate) fn handle_open_annotate_dialog(&mut self) -> Task<Message> {
        self.ui_state.annotate_dialog_open = true;
        self.interaction_state.context_menu = None;
        // Altium parity: these big modals live in their own OS window
        // from the moment they open — no in-window overlay, no drag-off
        // dance. `handle_detach_modal` is idempotent, so re-opening
        // while a window already exists just no-ops.
        self.handle_detach_modal(super::super::super::state::ModalId::AnnotateDialog)
    }

    pub(crate) fn handle_close_annotate_dialog(&mut self) -> Task<Message> {
        self.ui_state.annotate_dialog_open = false;
        // A Re-prefix request belongs to this dialog session: leaving it
        // makes any lookup in flight and any open confirm stale.
        self.invalidate_reprefix();
        let close_confirm = self.drain_reprefix_window_close();
        let close = self.close_detached_modal(super::super::super::state::ModalId::AnnotateDialog);
        Task::batch([close, close_confirm])
    }

    /// Altium's "Reset Duplicate Designators" — find references that
    /// appear on more than one part across the WHOLE project and reset just
    /// those to `{prefix}?`. The project is the one assembler's answer; see
    /// `reset_duplicates.rs` for the numbering half.
    pub(crate) fn handle_reset_duplicate_designators(&mut self) -> Task<Message> {
        self.begin_project_run(PassKind::ResetDuplicates)
    }

    pub(crate) fn handle_annotate_order_changed(
        &mut self,
        order: super::super::super::state::AnnotateOrder,
    ) -> Task<Message> {
        self.ui_state.annotate_order = order;
        // The cached preview rows were ordered under the old setting.
        self.recompute_annotate_entries();
        Task::none()
    }
}
