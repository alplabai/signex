//! Component Preview handlers — the five preview tabs (Preview /
//! Parameters / Supply / Datasheet / Simulation): editor events, tab
//! selection, and saving a row.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;
use crate::library::messages::DatasheetMsg;
use crate::library::resolve::{ResolvedKind, report_read_failure};

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
            EditorMsg::Datasheet(DatasheetMsg::AssetsStored(result)) => {
                self.finish_staged_asset_store(&address, result);
                return Task::none();
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

    /// Write one Component Preview row back through its library adapter
    /// and clear the editor's dirty flag. Returns why a save failed so a
    /// Save-All flow (app quit, project close) can keep the app open and
    /// name the row instead of dropping the edit. A row whose editor is
    /// already gone has nothing to save and counts as saved.
    ///
    /// PDFs the row pins from this session are written here, on the
    /// calling thread: a quit or project close needs them on disk before
    /// it goes on. The Save button writes them on a worker instead.
    pub(crate) fn try_save_row(&mut self, address: &EditorAddress) -> anyhow::Result<()> {
        let written = self.store_staged_assets_blocking(address)?;
        self.commit_row(address, &written)
    }

    /// Save the row through its adapter, committing `written_assets` (the
    /// PDFs already stored beside the library) with it, and settle the
    /// editor. Every PDF the row pins from this session must be among
    /// them: one picked while the write ran is refused, not pinned to a
    /// hash with no file.
    pub(super) fn commit_row(
        &mut self,
        address: &EditorAddress,
        written_assets: &[String],
    ) -> anyhow::Result<()> {
        use anyhow::Context as _;
        let Some(state) = self.library.editors.get(address) else {
            return Ok(());
        };
        let library_path = state.library_path.clone();
        let table = state.table.clone();
        let mut row = state.row.clone();
        let needed = super::datasheet_save::asset_rel_paths(&state.staged_assets_in_use());
        if let Some(missing) = needed.iter().find(|rel| !written_assets.contains(rel)) {
            anyhow::bail!(
                "a datasheet PDF was picked while the row was being saved ({missing} is not \
                 stored yet); save again"
            );
        }
        // A document added and never filled in is not saved (#1231).
        row.drop_empty_documents();
        row.updated = chrono::Utc::now();
        // A stale hash would be written alongside the new content, so a
        // hash failure fails the save instead of being warned past.
        row.refresh_content_hash()
            .context("refresh row content hash")?;

        let library_id = self
            .library
            .library_at(&library_path)
            .map(|lib| lib.library_id)
            .with_context(|| format!("library {} is not open", library_path.display()))?;
        let adapter = self
            .library
            .set
            .get(library_id)
            .with_context(|| format!("library {} is not mounted", library_path.display()))?;
        // A server-backed library refuses the write if the row changed
        // since this editor read it; the error keeps the editor dirty.
        // The pinned PDFs are committed in the same commit as the row.
        let saved = adapter.update_row_with_assets(
            &table,
            row.clone(),
            "edit row (signex-app)",
            written_assets,
        );
        let stale = matches!(saved, Err(signex_library::LibraryError::RowChanged { .. }));
        let revision = match saved {
            Ok(revision) => revision,
            Err(e) => {
                if stale && let Some(state) = self.library.editors.get_mut(address) {
                    state.stale_base = true;
                }
                return Err(anyhow::Error::new(e).context(format!("update row in table {table}")));
            }
        };
        row.server_revision = revision;
        if let Some(state) = self.library.editors.get_mut(address) {
            state.row = row;
            state.dirty = false;
            state.stale_base = false;
            state.reload_confirming = false;
            // Written and committed: the staged copies are spent.
            state.staged_datasheets.clear();
            state.datasheet_errors.clear();
        }
        if let Err(e) = self.library.refresh_components(&library_path) {
            tracing::warn!(target: "signex::library", error = %e, "post-save refresh failed");
        }
        Ok(())
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
