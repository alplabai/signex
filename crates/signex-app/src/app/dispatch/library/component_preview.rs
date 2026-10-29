//! Component Preview handlers — the five preview tabs (Preview /
//! Parameters / Supply / Datasheet / Simulation): editor events, tab
//! selection, and saving a row.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;
use crate::library::messages::DatasheetMsg;
use crate::library::refresh::RefreshFollowUp;
use crate::library::resolve::{ResolvedKind, report_read_failure};
use crate::library::writes::{
    PreviewRowSave, RowEdit, RowStart, WriteFailure, WriteKind, row_edit_task,
};

/// One Component Preview row save, ready to run.
struct RowWrite {
    library_path: std::path::PathBuf,
    table: String,
    /// The copy to write (hashed, stamped).
    row: signex_library::ComponentRow,
    /// Library-relative paths of the pinned PDFs committed with the row.
    needed: Vec<String>,
    /// Hashes of the staged PDFs the row pins.
    saved_hashes: Vec<String>,
}

impl Signex {
    /// Trace-only signal: a Component Preview tab was opened for the
    /// given address. Fired alongside `OpenComponentRow`.
    pub(super) fn handle_component_preview_opened(
        &mut self,
        path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        tracing::debug!(
            target: "signex::library",
            path = %path.display(),
            table = %table,
            row_id = %row_id,
            "ComponentPreviewOpened — Component Preview tab opened"
        );
        Task::none()
    }

    /// Component Preview event handler.
    pub(super) fn handle_editor_event(
        &mut self,
        address: EditorAddress,
        msg: EditorMsg,
    ) -> Task<Message> {
        // The write the save started has finished: it must be settled
        // (and its in-flight mark released) whatever the mount has become.
        let msg = match msg {
            EditorMsg::Datasheet(DatasheetMsg::AssetsStored { generation, result }) => {
                return self.finish_staged_asset_store(&address, generation, result);
            }
            // A refused file is also reported to the Messages panel, not
            // only on the Datasheet tab.
            EditorMsg::Datasheet(DatasheetMsg::UploadResult(picked)) => {
                if let Some(Err(reason)) = &picked.file {
                    crate::diagnostics::log_warning(format!(
                        "[library] datasheet PDF not pinned: {reason}"
                    ));
                    self.sync_diagnostics_panel_ctx();
                }
                EditorMsg::Datasheet(DatasheetMsg::UploadResult(picked))
            }
            other => other,
        };
        // Navigation stays available on a read-only mount (including
        // scroll / cursor / selection actions in the SPICE deck, which
        // never change its text); every other event edits or saves the
        // row, so it is refused with the explanation card before any
        // draft state changes.
        let navigates = match &msg {
            EditorMsg::CloseEditor | EditorMsg::SelectTab(_) | EditorMsg::OpenWhereUsedTab => true,
            EditorMsg::SimBodyAction(action) => !action.is_edit(),
            _ => false,
        };
        if !navigates && self.refuse_read_only_write(&address.library_path) {
            return Task::none();
        }
        match msg {
            EditorMsg::CloseEditor => return self.close_component_preview(&address),
            EditorMsg::SaveDraft | EditorMsg::Commit => {
                return self.handle_save_row(&address);
            }
            EditorMsg::ReloadRequested | EditorMsg::ReloadConfirmed
                if self
                    .library
                    .editors
                    .get(&address)
                    .is_some_and(|state| state.saving_row.is_some() || state.binding_primitive) =>
            {
                // The row is being saved: reloading now would drop the copy
                // being saved. Reload once the save has finished.
                crate::diagnostics::log_info(
                    "Reload was ignored: this row is being saved. Reload again once the save has finished.",
                );
                self.sync_diagnostics_panel_ctx();
                return Task::none();
            }
            EditorMsg::ReloadRequested => {
                // Nothing to lose when the copy is clean: reload at once.
                let needs_confirm = self
                    .library
                    .editors
                    .get(&address)
                    .is_some_and(|state| state.has_unsaved_edits());
                if needs_confirm {
                    if let Some(state) = self.library.editors.get_mut(&address) {
                        state.reload_confirming = true;
                    }
                } else {
                    self.reload_editor_from_server(&address);
                }
                return Task::none();
            }
            EditorMsg::ReloadConfirmed => {
                self.reload_editor_from_server(&address);
                return Task::none();
            }
            EditorMsg::ReloadCancelled => {
                if let Some(state) = self.library.editors.get_mut(&address) {
                    state.reload_confirming = false;
                }
                return Task::none();
            }
            EditorMsg::SelectTab(tab) => {
                self.handle_select_preview_tab(&address, tab);
                return Task::none();
            }
            EditorMsg::OpenWhereUsedTab => {
                if let Some(state) = self.library.editors.get_mut(&address) {
                    state.active_tab = PreviewTab::Preview;
                }
                return Task::none();
            }
            // Submit-for-review is dropped from the Component Preview
            // surface in v0.9-refactor-2 — review workflows happen
            // outside the row context. The variants stay in the message
            // tree for potential future revival.
            EditorMsg::SubmitForReview
            | EditorMsg::SubmitForReviewNotesChanged(_)
            | EditorMsg::SubmitForReviewCancel
            | EditorMsg::SubmitForReviewConfirm
            | EditorMsg::SubmitForReviewResult(_) => {
                // `warn!`, not `debug!`: the user pressed a control and it
                // did nothing. `debug!` is below the default
                // `LevelFilter::Info`, so in a shipped build that refusal
                // reached no one — the click looked like a hang.
                tracing::warn!(
                    target: "signex::library",
                    "submit for review did nothing: it is not wired up in the Component \
                     Preview surface"
                );
                return Task::none();
            }
            EditorMsg::Datasheet(DatasheetMsg::UploadDialog(index)) => {
                return self.begin_datasheet_pick(&address, index);
            }
            _ => {}
        }

        let Some(state) = self.library.editors.get_mut(&address) else {
            return Task::none();
        };
        apply_inline_edit(state, msg);
        Task::none()
    }

    fn handle_select_preview_tab(&mut self, address: &EditorAddress, tab: PreviewTab) {
        let Some(state) = self.library.editors.get_mut(address) else {
            return;
        };
        state.active_tab = tab;

        match tab {
            PreviewTab::Preview => {
                if state.symbol.is_none() {
                    let symbol_ref = state.row.symbol_ref;
                    let resolved = report_read_failure(
                        self.library.set.resolve_symbol(&symbol_ref),
                        ResolvedKind::Symbol,
                        &symbol_ref,
                        "component preview tab",
                    );
                    if let Some(state) = self.library.editors.get_mut(address) {
                        state.symbol = resolved;
                    }
                }
                if let Some(state) = self.library.editors.get_mut(address)
                    && state.footprint.is_none()
                {
                    let resolved = state.row.footprint_ref.as_ref().and_then(|r| {
                        report_read_failure(
                            self.library.set.resolve_footprint(r),
                            ResolvedKind::Footprint,
                            r,
                            "component preview tab",
                        )
                    });
                    if let Some(state) = self.library.editors.get_mut(address) {
                        state.footprint = resolved;
                    }
                }
            }
            PreviewTab::Simulation => {
                if state.sim.is_none()
                    && let Some(sim_ref) = state.row.sim_ref.as_ref()
                {
                    let sim_ref = *sim_ref;
                    let resolved = report_read_failure(
                        self.library.set.resolve_sim(&sim_ref),
                        ResolvedKind::Sim,
                        &sim_ref,
                        "component preview simulation tab",
                    );
                    if let Some(state) = self.library.editors.get_mut(address) {
                        if let Some(sim) = resolved.as_ref() {
                            state.sim_body = Some(iced::widget::text_editor::Content::with_text(
                                sim.body.as_str(),
                            ));
                        }
                        state.sim = resolved;
                    }
                }
            }
            _ => {}
        }
    }

    /// The Save button. A row that pins PDFs picked this session first has
    /// them written on a worker (`datasheet_save`), then saved when that
    /// finishes; any other row saves at once.
    pub(super) fn handle_save_row(&mut self, address: &EditorAddress) -> Task<Message> {
        match self.begin_save_row(address) {
            Ok(task) => task,
            Err(e) => {
                self.report_save_row_failure(&e);
                Task::none()
            }
        }
    }

    /// Log a failed row save and raise the Save Failed card: the dirty
    /// flag stays set, so without the card the Save button looks like it
    /// worked (#110).
    pub(crate) fn report_save_row_failure(&mut self, error: &anyhow::Error) {
        crate::diagnostics::log_error("save row failed", error);
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::save(format!(
            "The component row could not be saved: {error:#}"
        )));
    }

    /// Write the row on a worker, committing the PDFs it pins from this
    /// session (which must be among `written_assets`, the ones already
    /// stored beside the library) with it. The write takes its turn in the
    /// library's queue, so it neither overtakes nor is overtaken by the
    /// other row writes of that library (#893).
    ///
    /// The row saved is the copy in `saving_row`, taken when Save was
    /// pressed; edits made meanwhile stay unsaved. `batch` names the Save
    /// All whose settle step needs the outcome, recorded in
    /// `LibraryState::batch_row_failures` instead of raising a card.
    pub(super) fn begin_row_write(
        &mut self,
        address: &EditorAddress,
        written_assets: &[String],
        batch: Option<u64>,
    ) -> anyhow::Result<Task<Message>> {
        let Some(write) = self.prepare_row_write(address, written_assets)? else {
            return Ok(Task::none());
        };
        let generation = self
            .library
            .editors
            .get(address)
            .map_or(0, |state| state.saving_generation);
        let RowWrite {
            library_path,
            table,
            row,
            needed,
            saved_hashes,
        } = write;
        let kind = WriteKind::PreviewRowSaved {
            address: address.clone(),
            generation,
            batch,
            saved: Box::new(PreviewRowSave {
                row: row.clone(),
                saved_hashes,
            }),
        };
        let edit = RowEdit {
            table,
            row_id: address.row_id,
            start: RowStart::Full(Box::new(row)),
            patch: None,
            assets: needed,
            message: "edit row (signex-app)",
        };
        Ok(row_edit_task(&mut self.library, &library_path, kind, edit))
    }

    /// A row save finished.
    pub(in crate::app::dispatch::library) fn preview_row_saved(
        &mut self,
        address: EditorAddress,
        generation: u64,
        batch: Option<u64>,
        saved: PreviewRowSave,
        written: Option<Box<signex_library::ComponentRow>>,
        outcome: Result<Option<u64>, WriteFailure>,
    ) -> Task<Message> {
        // The save this result belongs to is gone when its editor was
        // closed (and maybe reopened, even saved again) meanwhile: the
        // editor then holds a different live row, which this result must
        // not settle.
        let current = self.library.editors.get(&address).is_some_and(|state| {
            state.saving_row.is_some() && state.saving_generation == generation
        });
        match outcome {
            Ok(revision) => {
                if current {
                    let row = written.map_or(saved.row, |row| *row);
                    self.apply_row_written(&address, row, &saved.saved_hashes, revision);
                }
            }
            Err(failure) => {
                if current {
                    self.row_write_failed(&address, failure.row_changed);
                }
                let error =
                    anyhow::anyhow!("update row in table {}: {}", address.table, failure.reason);
                match batch {
                    // A Save All is waiting on this row: its settle step
                    // reports the failure and keeps the window / library
                    // open.
                    Some(batch) => {
                        let label = self.library.editor_label(&address);
                        crate::diagnostics::log_error(
                            &format!("Save All: could not save {label}"),
                            &error,
                        );
                        self.library
                            .batch_row_failures
                            .insert((batch, address.clone()), (label, format!("{error:#}")));
                    }
                    None => self.report_save_row_failure(&error),
                }
            }
        }
        // The editor stays dirty on its old base revision after a failure;
        // the refresh shows what is stored.
        self.refresh_library_task(&address.library_path, RefreshFollowUp::Nothing)
    }

    /// Everything a row save needs, taken from the editor: the copy to
    /// write and the PDFs it pins. `Ok(None)` when the editor is gone.
    /// A failure leaves the editor as it was before the save, not saving.
    fn prepare_row_write(
        &mut self,
        address: &EditorAddress,
        written_assets: &[String],
    ) -> anyhow::Result<Option<RowWrite>> {
        let Some(library_path) = self
            .library
            .editors
            .get(address)
            .map(|state| state.library_path.clone())
        else {
            return Ok(None);
        };
        // The mounted adapter still points at the old path while the library
        // is being renamed (#859).
        let refusal = self.refuse_while_path_op(&library_path, "saved");
        let Some(state) = self.library.editors.get_mut(address) else {
            return Ok(None);
        };
        let prepared = refusal.and_then(|()| Self::row_write_of(state, written_assets));
        if prepared.is_err() {
            state.saving_row = None;
        }
        prepared.map(Some)
    }

    fn row_write_of(
        state: &mut ComponentPreviewState,
        written_assets: &[String],
    ) -> anyhow::Result<RowWrite> {
        use anyhow::Context as _;
        if state.saving_row.is_none() {
            // A typed number still in its edit buffer is part of the row.
            crate::library::component_preview::updates::flush_edit_buffers(state)
                .map_err(anyhow::Error::msg)?;
        }
        let mut row = state
            .saving_row
            .clone()
            .unwrap_or_else(|| state.row.clone());
        let saved_hashes: Vec<String> = state
            .staged_assets_pinned_by(&row)
            .into_iter()
            .map(|asset| asset.hash)
            .collect();
        let needed = super::datasheet_save::asset_rel_paths(&state.staged_assets_pinned_by(&row));
        if let Some(missing) = needed.iter().find(|rel| !written_assets.contains(rel)) {
            anyhow::bail!("a datasheet PDF ({missing}) is not stored yet; save again");
        }
        // A document added and never filled in is not saved (#1231).
        row.drop_empty_documents();
        row.updated = chrono::Utc::now();
        // A stale hash would be written alongside the new content, so a
        // hash failure fails the save instead of being warned past.
        row.refresh_content_hash()
            .context("refresh row content hash")?;
        Ok(RowWrite {
            library_path: state.library_path.clone(),
            table: state.table.clone(),
            row,
            needed,
            saved_hashes,
        })
    }

    /// A row save was refused or failed: the editor is no longer saving, and
    /// offers "Reload saved version" when the row moved on.
    fn row_write_failed(&mut self, address: &EditorAddress, stale: bool) {
        if let Some(state) = self.library.editors.get_mut(address) {
            state.saving_row = None;
            if stale {
                state.stale_base = true;
            }
        }
    }

    /// The row was written at `revision`: settle the editor.
    fn apply_row_written(
        &mut self,
        address: &EditorAddress,
        mut row: signex_library::ComponentRow,
        saved_hashes: &[String],
        revision: Option<u64>,
    ) {
        row.server_revision = revision;
        let Some(state) = self.library.editors.get_mut(address) else {
            return;
        };
        let snapshot = state.saving_row.take();
        let edited_since_save = snapshot.is_some_and(|saved| saved != state.row);
        state.stale_base = false;
        state.reload_confirming = false;
        if edited_since_save {
            // The saved copy is stored; what was typed since is not.
            // Only the new base revision is taken over.
            state.row.server_revision = revision;
        } else {
            state.row = row;
            state.dirty = false;
            state.datasheet_errors.clear();
        }
        // Written and committed: those staged copies are spent (a PDF
        // picked since stays staged).
        state
            .staged_datasheets
            .retain(|hash, _| !saved_hashes.contains(hash));
    }

    /// Replace the working copy of one Component Preview with the row the
    /// library stores now (content and base revision), dropping its unsaved
    /// edits. A row that cannot be read keeps the working copy.
    pub(crate) fn reload_editor_from_server(&mut self, address: &EditorAddress) {
        let Some(library_id) = self
            .library
            .library_at(&address.library_path)
            .map(|lib| lib.library_id)
        else {
            return;
        };
        let read = self
            .library
            .set
            .get(library_id)
            .ok_or_else(|| {
                signex_library::LibraryError::NotFound(address.library_path.display().to_string())
            })
            .and_then(|adapter| adapter.read_row(&address.table, address.row_id));
        let row = match read {
            Ok(row) => row,
            Err(e) => {
                crate::diagnostics::log_warning(format!(
                    "Row {} could not be reloaded: {e}",
                    address.row_id
                ));
                self.sync_diagnostics_panel_ctx();
                return;
            }
        };
        let Some(old) = self.library.editors.get(address) else {
            return;
        };
        let tab = old.active_tab;
        let mut fresh = crate::library::state::ComponentPreviewState::from_row(
            address.library_path.clone(),
            address.table.clone(),
            row,
        );
        fresh.active_tab = tab;
        self.library.editors.insert(address.clone(), fresh);
        self.handle_select_preview_tab(address, tab);
    }

    /// After a write to `rows` that did not go through their Component
    /// Previews (browser cell or modal edit, primitive bind, cascade), bring
    /// every CLEAN preview of those rows to the stored content and revision.
    /// A DIRTY preview keeps its base revision so its own save is refused as
    /// stale (#115) instead of silently overwriting the write.
    pub(crate) fn refresh_clean_editors(&mut self, library_path: &std::path::Path, rows: &[RowId]) {
        let clean: Vec<EditorAddress> = self
            .library
            .editors
            .iter()
            .filter(|(a, s)| {
                a.library_path == library_path && rows.contains(&a.row_id) && !s.has_unsaved_edits()
            })
            .map(|(a, _)| a.clone())
            .collect();
        for address in clean {
            self.reload_editor_from_server(&address);
        }
    }

    /// Close one Component Preview editor, discarding its draft: closes
    /// the tab that shows it, or drops the orphan editor state when no
    /// tab does.
    pub(crate) fn close_component_preview(&mut self, address: &EditorAddress) -> Task<Message> {
        let synthetic = address.synthetic_tab_path();
        if let Some(idx) = self
            .document_state
            .tabs
            .iter()
            .position(|t| t.path == synthetic)
        {
            return self.close_tab_now(idx);
        }
        self.library.editors.remove(address);
        Task::none()
    }
}
