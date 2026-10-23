//! Component Preview handlers — the five preview tabs (Preview /
//! Parameters / Supply / Datasheet / Simulation): editor events, tab
//! selection, and saving a row.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;
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
        match msg {
            EditorMsg::CloseEditor => return self.close_component_preview(&address),
            EditorMsg::SaveDraft | EditorMsg::Commit => {
                self.handle_save_row(&address);
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
            EditorMsg::DatasheetUploadDialog => {
                let library_path = address.library_path.clone();
                let table = address.table.clone();
                let row_id = address.row_id;
                return Task::perform(
                    async {
                        let picked = rfd::AsyncFileDialog::new()
                            .set_title("Pin datasheet PDF")
                            .add_filter("PDF", &["pdf"])
                            .add_filter("All files", &["*"])
                            .pick_file()
                            .await;
                        match picked {
                            Some(handle) => {
                                let filename = handle.file_name();
                                let bytes = handle.read().await;
                                Some((bytes, filename))
                            }
                            None => None,
                        }
                    },
                    move |result| {
                        Message::Library(LibraryMessage::EditorEvent {
                            library_path: library_path.clone(),
                            table: table.clone(),
                            row_id,
                            msg: EditorMsg::DatasheetUploadResult(result),
                        })
                    },
                );
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

    pub(super) fn handle_save_row(&mut self, address: &EditorAddress) {
        if let Err(e) = self.try_save_row(address) {
            self.report_save_row_failure(&e);
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
    pub(crate) fn try_save_row(&mut self, address: &EditorAddress) -> anyhow::Result<()> {
        use anyhow::Context as _;
        let Some(state) = self.library.editors.get(address) else {
            return Ok(());
        };
        let library_path = state.library_path.clone();
        let table = state.table.clone();
        let mut row = state.row.clone();
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
        adapter
            .update_row(&table, row.clone(), "edit row (signex-app)")
            .with_context(|| format!("update row in table {table}"))?;
        if let Some(state) = self.library.editors.get_mut(address) {
            state.row = row;
            state.dirty = false;
        }
        if let Err(e) = self.library.refresh_components(&library_path) {
            tracing::warn!(target: "signex::library", error = %e, "post-save refresh failed");
        }
        Ok(())
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
