//! Off-thread document opens (#859): start a read as a `Task`, then install
//! the result in `update` only while it is still the read that was asked for.

use std::path::PathBuf;

use iced::Task;

use super::super::super::*;
use crate::app::document_read::{LoadedDocument, OpenFailure, ReadJob, ReadRequest};

impl Signex {
    /// Run `job` on a worker for `path`. A read already in flight for the
    /// same path (a double click) spawns nothing: its completion opens the
    /// one tab.
    pub(crate) fn begin_document_read(&mut self, path: PathBuf, job: ReadJob) -> Task<Message> {
        let Some(generation) = self.document_state.pending_opens.begin(path.clone()) else {
            return Task::none();
        };
        let request = ReadRequest {
            path: path.clone(),
            generation,
        };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || job.run(&path))
                    .await
                    .unwrap_or_else(|e| Err(OpenFailure::Card(format!("spawn_blocking: {e}"))))
            },
            move |result| Message::File(FileMsg::DocumentReadFinished { request, result }),
        )
    }

    /// Install a finished read, unless it went stale while it ran: the
    /// project or all documents were closed (the claim was cancelled), the
    /// path was requested again, or another route already opened a tab.
    pub(crate) fn handle_document_read_finished(
        &mut self,
        request: ReadRequest,
        result: Result<Box<LoadedDocument>, OpenFailure>,
    ) -> Task<Message> {
        let path = request.path.clone();
        let task = self.finish_document_read(request, result);
        // A Create Sheet request from the Projects tree was waiting on it.
        self.resume_create_sheet(&path, true);
        // So was a canvas descent, which records its placement now.
        self.finish_descent(&path);
        task
    }

    fn finish_document_read(
        &mut self,
        request: ReadRequest,
        result: Result<Box<LoadedDocument>, OpenFailure>,
    ) -> Task<Message> {
        if !self
            .document_state
            .pending_opens
            .finish(&request.path, request.generation)
        {
            tracing::debug!(
                path = %request.path.display(),
                generation = request.generation,
                "dropped a stale document read"
            );
            return Task::none();
        }
        // A tab, or a dirty engine parked without one (a sheet-reference
        // rewrite loaded it while this read ran), already owns the file:
        // installing the read would replace its edits with disk content.
        if self.is_open_result_superseded(&request.path, &result) {
            if let Ok(document) = &result
                && let LoadedDocument::Schematic { title, .. } = document.as_ref()
            {
                self.attach_parked_tab_for_superseded_open(&request.path, title.clone());
                self.activate_tab_for_path(&request.path);
            }
            return Task::none();
        }
        match result {
            Ok(document) => self.install_loaded_document(request.path, *document),
            Err(failure) => {
                // No silent no-op: a descent into a missing child offers to
                // create the file (#1275).
                if matches!(failure, OpenFailure::Missing(_)) {
                    self.offer_to_create_missing_child(&request.path);
                }
                self.report_read_failure(failure);
                Task::none()
            }
        }
    }

    pub(crate) fn has_tab_for(&self, path: &std::path::Path) -> bool {
        self.document_state.tabs.iter().any(|tab| tab.path == path)
    }

    /// A finished open whose tab already exists must not make a second one
    /// aliasing the same engine (#478).
    pub(crate) fn log_duplicate_open(path: &std::path::Path) {
        tracing::debug!(
            path = %path.display(),
            "dropped a finished document read: a tab for it is already open"
        );
    }

    /// Closing every document also abandons the opens and restores still
    /// running, so none of them reopens a tab afterwards.
    pub(crate) fn cancel_in_flight_reads(&mut self) {
        self.document_state.pending_opens.cancel_all();
        self.ui_state.pending_create_sheet = None;
        self.ui_state.pending_descents.clear();
        self.document_state.pending_restores.cancel_all_retaining();
    }

    fn install_loaded_document(
        &mut self,
        path: PathBuf,
        document: LoadedDocument,
    ) -> Task<Message> {
        match document {
            LoadedDocument::Schematic { title, sheet } => {
                self.open_schematic_tab(path.clone(), title, *sheet);
                // A recovery copy from an earlier run is reconciled with the
                // file before any prompt can offer it.
                return self.autosave_check_recovery(&path);
            }
            LoadedDocument::Pcb { title, board } => self.open_pcb_tab(path, title, *board),
            LoadedDocument::Symbol(file) => {
                self.open_symbol_editor_tab(path.clone(), *file);
                // An earlier run's recovery copy of this symbol (#1185).
                return self.autosave_check_recovery(&path);
            }
            LoadedDocument::Footprint(file) => {
                self.open_footprint_editor_tab(path.clone(), *file);
                // An earlier run's recovery copy of this footprint (#1185).
                return self.autosave_check_recovery(&path);
            }
        }
        Task::none()
    }

    fn report_read_failure(&mut self, failure: OpenFailure) {
        match failure {
            OpenFailure::LogOnly(message) => crate::diagnostics::log_error(
                "Failed to open project tree document",
                &anyhow::anyhow!(message),
            ),
            OpenFailure::Missing(message) => {
                crate::diagnostics::log_info(message);
            }
            OpenFailure::Card(message) => {
                crate::diagnostics::log_error(
                    "Failed to open primitive",
                    &anyhow::anyhow!(message.clone()),
                );
                self.document_state.error_notice =
                    Some(crate::app::state::ErrorNotice::open(message));
            }
        }
    }

    fn open_symbol_editor_tab(&mut self, path: PathBuf, file: signex_library::SymbolFile) {
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                if !file.display_name.is_empty() {
                    file.display_name.clone()
                } else {
                    file.symbols[0].name.clone()
                }
            });
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);
        let state = crate::app::SymbolDocument::new(path.clone(), file);
        self.document_state
            .symbol_editors
            .insert(path.clone(), state);
        self.push_primitive_tab(
            path.clone(),
            title,
            project_id,
            crate::app::TabKind::SymbolEditor(path),
        );
    }

    fn open_footprint_editor_tab(&mut self, path: PathBuf, file: signex_library::FootprintFile) {
        // v0.18.6 — the `FootprintFile` envelope stays with the editor so
        // saves keep `file_uuid` and any multi-footprint siblings.
        let display_name = file.footprints[0].name.clone();
        // A library names a footprint file by the footprint's uuid, which says
        // nothing to the user: the tab shows the footprint's name then.
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|stem| uuid::Uuid::parse_str(stem).is_err())
            .unwrap_or(display_name);
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);
        // HI-23: seed snap-disabled from the user's persisted global toggle
        // so opening a .snxfpt while snap is off doesn't reset the editor.
        let state = crate::app::FootprintDocument::new(path.clone(), file)
            .with_global_snap_disabled(!self.ui_state.snap_enabled);
        self.document_state
            .footprint_editors
            .insert(path.clone(), state);
        self.push_primitive_tab(
            path.clone(),
            title,
            project_id,
            crate::app::TabKind::FootprintEditor(path),
        );
    }

    fn push_primitive_tab(
        &mut self,
        path: PathBuf,
        title: String,
        project_id: Option<crate::app::ProjectId>,
        kind: crate::app::TabKind,
    ) {
        self.park_active_schematic_session();
        self.document_state.tabs.push(crate::app::TabInfo {
            title,
            path,
            cached_document: None,
            project_id,
            kind,
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        // Standalone primitive tabs don't drive `active_path` — clear so the
        // canvas doesn't render a stale schematic.
        self.document_state.active_path = None;
        self.refresh_panel_ctx();
    }
}
