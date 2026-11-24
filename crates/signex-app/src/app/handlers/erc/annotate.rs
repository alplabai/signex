//! Annotate + duplicate-designator reset handlers. Split from `handlers/erc.rs`.

use iced::Task;
use signex_i18n::RenderTranslatable;

use super::super::super::*;
use super::annotate_job::PassKind;

/// Annotate a sheet that exists only on disk, in memory, WITHOUT writing it.
/// Returns the engine (path set) when the pass changed it, so the caller can
/// park it dirty like any other unsaved sheet (#669, #2760): nothing reaches
/// disk until the user saves, and the engine's history makes the change
/// undoable. `Err` means the sheet failed and was logged.
pub(super) fn annotate_unopened_in_memory(
    mut sheet: signex_types::schematic::SchematicSheet,
    path: &std::path::Path,
    mode: signex_engine::AnnotateMode,
    next_by_prefix: &mut std::collections::HashMap<String, u32>,
    scope: signex_engine::AnnotateScope<'_>,
    assigned: &mut std::collections::HashMap<usize, String>,
) -> Result<Option<signex_engine::Engine>, PassFailed> {
    let context = format!("Annotate failed on unopened sheet '{}'", path.display());
    let repairs = sheet.take_load_repairs();
    let mut engine = match signex_engine::Engine::new(sheet) {
        Ok(engine) => engine,
        Err(error) => {
            crate::diagnostics::log_error(&context, &anyhow::Error::new(error));
            return Err(PassFailed);
        }
    };
    engine.set_path(Some(path.to_path_buf()));
    match engine.annotate_in_project(mode, next_by_prefix, scope, assigned) {
        Ok(true) => {
            crate::app::sheet_repairs::log_sheet_repairs(path, &repairs);
            Ok(Some(engine))
        }
        Ok(false) => Ok(None),
        Err(error) => {
            crate::diagnostics::log_error(&context, &anyhow::Error::new(error));
            Err(PassFailed)
        }
    }
}

fn sheet_name(path: &std::path::Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
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
    /// one), a read of it is in flight or a save holds it: an engine parked
    /// now would race that worker's result. The reason leads the Messages line.
    pub(super) fn unopened_write_blocked(&self, pass: &str, path: &std::path::Path) -> bool {
        let Some(reason) = self.unopened_block_reason(path) else {
            return false;
        };
        let name = sheet_name(path);
        crate::diagnostics::log_warning(format!("{pass}: {name} was left unchanged, {reason}"));
        true
    }

    /// Why a bulk pass cannot change the tab-less sheet at `path` now.
    fn unopened_block_reason(&self, path: &std::path::Path) -> Option<&'static str> {
        if self.document_state.pending_restores.contains(path) {
            Some("a History restore of it is still running")
        } else if self.document_state.pending_opens.contains(path) {
            Some("a read of it is still in flight")
        } else if self.document_state.document_saves.is_saving(path) {
            Some("a save of it is still running")
        } else {
            None
        }
    }

    /// Refuse a Reset & Renumber that would leave a sheet out (#1753): the
    /// sheet keeps its designators, and the numbers handed out around it can
    /// repeat them. Says which sheets and why; nothing has been changed yet.
    /// True when refused.
    pub(super) fn refuse_reset_that_skips(
        &mut self,
        label: &str,
        project_set: &crate::app::project_sheets::ProjectSheetSet,
        ordered_paths: &[std::path::PathBuf],
        read_only: &std::collections::BTreeSet<std::path::PathBuf>,
    ) -> bool {
        let skipped = self.sheets_a_reset_would_skip(project_set, ordered_paths, read_only);
        if skipped.is_empty() {
            return false;
        }
        let detail = signex_i18n::Translatable::new(signex_i18n::ids::NOTICE_RUN_WOULD_SKIP)
            .arg("label", label)
            .arg("sheets", skipped.join(", "));
        crate::diagnostics::log_warning(detail.render_en_us());
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::annotate(detail));
        true
    }

    /// Refuse a Reset All that could not reach a sheet (#2760): one undo
    /// could not restore a project only partly reset. True when refused.
    pub(super) fn refuse_reset_all_that_skips(
        &mut self,
        label: &str,
        project_set: &crate::app::project_sheets::ProjectSheetSet,
        ordered_paths: &[std::path::PathBuf],
    ) -> bool {
        let skipped =
            self.sheets_a_reset_would_skip(project_set, ordered_paths, &Default::default());
        if skipped.is_empty() {
            return false;
        }
        self.refuse_run(
            label,
            &format!(
                "it could not reach {}, so the project would be only partly reset. Nothing was \
                 changed; try again once the sheet is free.",
                skipped.join(", ")
            ),
        );
        true
    }

    /// The sheets a reset would leave out, each with why.
    fn sheets_a_reset_would_skip(
        &self,
        project_set: &crate::app::project_sheets::ProjectSheetSet,
        ordered_paths: &[std::path::PathBuf],
        read_only: &std::collections::BTreeSet<std::path::PathBuf>,
    ) -> Vec<String> {
        let mut skipped: Vec<String> = read_only
            .iter()
            .map(|path| {
                format!(
                    "{} (written by a newer Signex, read-only here)",
                    sheet_name(path)
                )
            })
            .collect();
        // A sheet that could not be read is skipped too, and its designators
        // are missing from the counter's view of the project.
        skipped.extend(
            project_set
                .unreadable
                .iter()
                .map(|(path, why)| format!("{} (it could not be read: {why})", sheet_name(path))),
        );
        for path in ordered_paths {
            if self.document_state.engines.contains_key(path)
                || !project_set.sheets.contains_key(path)
                || read_only.contains(path)
            {
                continue;
            }
            if let Some(reason) = self.unopened_block_reason(path) {
                skipped.push(format!("{} ({reason})", sheet_name(path)));
            }
        }
        skipped
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
            signex_i18n::Translatable::new(signex_i18n::ids::NOTICE_RUN_SKIPPED_READ_ONLY)
                .arg("label", pass)
                .arg("summary", summary),
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
        let detail = signex_i18n::Translatable::new(signex_i18n::ids::NOTICE_RUN_PROJECT_MOVING)
            .arg("label", what)
            .arg("path", path.display().to_string());
        crate::diagnostics::log_warning(detail.render_en_us());
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
        let detail = signex_i18n::Translatable::new(signex_i18n::ids::NOTICE_RUN_REFUSED)
            .arg("label", what)
            .arg("reason", refusal.to_string());
        crate::diagnostics::log_warning(detail.render_en_us());
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::annotate(detail));
        true
    }

    /// Annotate the whole project. The sheets are read and numbered by
    /// [`Self::begin_project_run`]'s two steps; nothing blocks `update`, and
    /// no file changes until the user saves (#2760).
    pub(crate) fn handle_annotate(&mut self, mode: signex_engine::AnnotateMode) -> Task<Message> {
        self.begin_project_run(PassKind::Annotate(mode))
    }

    pub(crate) fn handle_open_annotate_dialog(&mut self) -> Task<Message> {
        self.ui_state.annotate_dialog_open = true;
        self.interaction_state.context_menu = None;
        self.reset_annotate_session();
        // These big modals live in their own OS window from the moment they
        // open. `handle_detach_modal` is idempotent, so re-opening while a
        // window already exists just no-ops.
        self.handle_detach_modal(super::super::super::state::ModalId::AnnotateDialog)
    }

    pub(crate) fn handle_close_annotate_dialog(&mut self) -> Task<Message> {
        self.ui_state.annotate_dialog_open = false;
        // The ticks belong to this dialog session: a later run from a menu
        // covers every sheet.
        self.reset_annotate_session();
        // A Re-prefix request belongs to this dialog session: leaving it
        // makes any lookup in flight and any open confirm stale.
        self.invalidate_reprefix();
        let close_confirm = self.drain_reprefix_window_close();
        // So is an open Reset confirm: without the ticks its Run would
        // cover every sheet.
        let close_reset = if self.ui_state.annotate_reset_confirm {
            self.handle_close_annotate_reset_confirm()
        } else {
            Task::none()
        };
        let close = self.close_detached_modal(super::super::super::state::ModalId::AnnotateDialog);
        Task::batch([close, close_confirm, close_reset])
    }

    /// Reset Duplicate Designators: find references that
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

    /// Forget the dialog session's view, filter and ticks; the reset the
    /// confirm runs stays, as a confirm may be open over the dialog.
    fn reset_annotate_session(&mut self) {
        let session = &mut self.ui_state.annotate_session;
        let reset_mode = session.reset_mode;
        **session = crate::app::handlers::erc::AnnotateSession {
            reset_mode,
            ..Default::default()
        };
        self.recompute_annotate_entries();
    }

    /// A sheet row's tick was clicked: the file joins or leaves the run.
    pub(crate) fn handle_annotate_sheet_tick_toggled(&mut self, path: std::path::PathBuf) {
        let unticked = &mut self.ui_state.annotate_session.unticked;
        if !unticked.remove(&path) {
            unticked.insert(path);
        }
        self.recompute_annotate_entries();
    }

    /// The sheets table's header tick: every loaded sheet joins (`true`) or
    /// leaves the run.
    pub(crate) fn handle_annotate_all_sheets_ticked(&mut self, on: bool) {
        let paths = self.ui_state.annotate_preview.sheet_paths();
        let unticked = &mut self.ui_state.annotate_session.unticked;
        if on {
            unticked.clear();
        } else {
            unticked.extend(paths);
        }
        self.recompute_annotate_entries();
    }

    /// Reset All or Reset & Renumber was asked for: open the confirm for that
    /// reset, with the preview loaded so it can name its scope (#76).
    pub(crate) fn handle_annotate_reset_pressed(
        &mut self,
        mode: signex_engine::AnnotateMode,
    ) -> Task<Message> {
        self.ui_state.annotate_session.reset_mode = mode;
        let open = self.handle_open_annotate_reset_confirm();
        Task::batch([open, self.refresh_annotate_preview()])
    }

    /// The sheets the dialog's ticks leave out of an Annotate run. Empty
    /// unless the dialog is open: a run from a menu covers every sheet.
    pub(super) fn annotate_unticked_sheets(
        &self,
    ) -> std::collections::BTreeSet<std::path::PathBuf> {
        if self.ui_state.annotate_dialog_open {
            self.ui_state.annotate_session.unticked.clone()
        } else {
            Default::default()
        }
    }
}
