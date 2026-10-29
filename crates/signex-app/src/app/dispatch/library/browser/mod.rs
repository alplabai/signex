//! Library Browser tab handlers — opening the browser, adding /
//! deleting / editing component rows, inline cell commits, and
//! opening a Component Preview row from the browser.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.
//!
//! The folder carries the `browser` namespace: grid/row handlers live in
//! `grid`, and table handlers in
//! `tables`; this `mod.rs` keeps the browser-tab handlers.

mod cell_commit;
mod grid;
mod modal_reload;
mod param_commit;
mod row_delete;
mod row_save;
mod tables;

use super::*;
use crate::library::writes::AfterRowCreated;

pub(super) use cell_commit::SavedCell;

impl Signex {
    /// Open `.snxlib` at `path` as a Library Browser tab.
    ///
    /// Step 1 — the mount — now happens **off the UI thread** (#99 part
    /// 2c). `LocalGitAdapter::open` plus the table/primitive scans are
    /// the expensive half of this gesture and blocking `update()` on them
    /// dropped frames; the tab opens one update tick later instead, when
    /// `LibraryMessage::MountFinished` lands and
    /// [`Self::finish_open_library_browser`] runs steps 2 and 3.
    ///
    /// Three outcomes, and conflating any two of them is a bug:
    /// already-mounted finishes immediately, an in-flight preparation
    /// does nothing (its completion will finish, on the upgraded intent),
    /// and only a fresh request spawns.
    pub(crate) fn handle_open_library_browser(
        &mut self,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        use crate::library::mount::{MountIntent, MountRequest};

        tracing::info!(
            target: "signex::library",
            path = %path.display(),
            exists = path.exists(),
            already_mounted = self.library.library_at(&path).is_some(),
            "open_library_browser: enter"
        );
        // A mount of a library in a project whose folder is moving would
        // read a path about to vanish.
        if self.refuse_with_notice_while_project_moves(
            &path,
            "open",
            crate::app::state::ErrorNotice::open,
        ) {
            return Task::none();
        }
        match self
            .library
            .request_mount(&path, MountIntent::OpenBrowserTab)
        {
            MountRequest::AlreadyMounted => self.finish_open_library_browser(path),
            MountRequest::InFlight => {
                // A preparation for this path is already running and the
                // call above upgraded its intent to `OpenBrowserTab`, so
                // its completion opens the tab. Spawning a second
                // preparation here would race two adapters onto one path.
                tracing::debug!(
                    target: "signex::library",
                    path = %path.display(),
                    "open_library_browser: mount already in flight; intent upgraded"
                );
                Task::none()
            }
            MountRequest::Spawn => {
                let spawn_path = path.clone();
                Task::perform(
                    crate::library::mount::prepare_mount_off_thread(spawn_path),
                    move |prepared| {
                        Message::Library(LibraryMessage::MountFinished {
                            path: path.clone(),
                            prepared,
                        })
                    },
                )
            }
        }
    }

    /// Steps 2 and 3 of opening a Library Browser tab — seed the
    /// per-browser state, then activate or push the tab.
    ///
    /// Split out of [`Self::handle_open_library_browser`] so the mount
    /// completion handler (`handle_mount_finished`) can finish the
    /// gesture without re-running the mount. Pure code motion: the body
    /// below is what ran inline before #99 part 2c, in the same order.
    pub(super) fn finish_open_library_browser(
        &mut self,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        // 2. Seed per-browser state if the path isn't already there.
        // 2b. Default `active_table` to the first table the library
        //     exposes, if any. Compute it through an immutable borrow
        //     before we take the mutable browser-entry.
        let default_table: Option<String> = self.library.library_at(&path).and_then(|lib| {
            let mut names: Vec<&String> = lib.tables.keys().collect();
            names.sort();
            names.first().map(|s| (*s).clone())
        });

        // Hydrate persisted search query for this library (per-path,
        // not global) the first time a browser tab opens this session.
        // This session's edits win over the disk: a debounced write may
        // not have landed yet.
        let persisted_search = match self.document_state.search_prefs.library_search(&path) {
            Some(typed) => typed,
            None => crate::fonts::read_library_browser_searches()
                .remove(&path)
                .unwrap_or_default(),
        };

        let entry = self
            .library
            .library_browsers
            .entry(path.clone())
            .or_insert_with(|| {
                let mut s = crate::library::state::LibraryBrowserState::new(path.clone());
                s.search = persisted_search;
                s
            });

        if entry.active_table.is_none() {
            entry.active_table = default_table;
        }

        // 3. Activate an existing tab if one is already open for this
        //    path; otherwise push a fresh tab.
        if let Some(idx) = self.document_state.tabs.iter().position(|t| t.path == path) {
            if idx != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = idx;
                self.sync_active_tab();
            }
            return Task::none();
        }

        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| path.display().to_string());
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);

        self.park_active_schematic_session();
        self.document_state.tabs.push(crate::app::TabInfo {
            title,
            path: path.clone(),
            cached_document: None,
            project_id,
            kind: crate::app::TabKind::LibraryBrowser(path),
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        // Library Browser tabs don't drive `active_path` — clear so the
        // canvas pane doesn't render a stale schematic.
        self.document_state.active_path = None;
        self.refresh_panel_ctx();
        Task::none()
    }

    /// Inline "+ Component" — mint a draft row directly into the
    /// browser's active table without opening any modal. The user
    /// fills in `internal_pn`, `manufacturer`, `mpn` etc. via the
    /// grid's inline cell editor; symbol / footprint binding lives in
    /// the Properties panel for the selected row.
    ///
    /// Library is implicit (the browser tab's library), table is the
    /// browser's `active_table` or — when none is selected — the
    /// generic-class default resolved through
    /// `manifest.table_for_class("generic")`. Closes F17 / F18 of the
    /// 2026-05-03 library polish: the New Component modal's library
    /// dropdown was meaningless inside a library tab, and the modal
    /// itself was a step the user didn't want.
    pub(super) fn handle_browser_add_component(
        &mut self,
        library_path: std::path::PathBuf,
        table: Option<String>,
    ) -> Task<Message> {
        if self.refuse_read_only_write(&library_path) {
            return Task::none();
        }
        let library_idx = match self
            .library
            .open_libraries
            .iter()
            .position(|lib| lib.root == library_path)
        {
            Some(idx) => idx,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    library = %library_path.display(),
                    "browser: Add Component — library not mounted"
                );
                return Task::none();
            }
        };

        // Resolve target table — explicit arg wins, else fall back to
        // the generic class default.
        let generic = signex_library::ComponentClass::generic();
        let resolved_table = match table {
            Some(t) if !t.trim().is_empty() => t,
            _ => match self
                .library
                .open_libraries
                .get(library_idx)
                .and_then(|lib| self.library.set.get(lib.library_id))
                .map(|adapter| adapter.manifest().table_for_class(generic.as_str()))
            {
                Some(t) => t,
                None => {
                    tracing::warn!(
                        target: "signex::library",
                        library = %library_path.display(),
                        "browser: Add Component — no active table and no class default"
                    );
                    return Task::none();
                }
            },
        };

        // F19 (2026-05-03 library polish, "we had a talk about basic
        // parameters"): infer the row's class from the resolved table
        // name so a row added to the "resistors" table comes in as a
        // resistor (not a generic). Reverse-lookup is cheap: strip the
        // trailing "s" and verify that the candidate class round-trips
        // back to the same table via `manifest.table_for_class`.
        // Falls back to `generic` if no match — this covers user-named
        // tables like "passives" that don't follow the pluralisation
        // convention.
        let class = self
            .library
            .open_libraries
            .get(library_idx)
            .and_then(|lib| self.library.set.get(lib.library_id))
            .and_then(|adapter| {
                let manifest = adapter.manifest();
                resolved_table.strip_suffix('s').and_then(|stem| {
                    if manifest.table_for_class(stem) == resolved_table {
                        Some(signex_library::ComponentClass::new(stem))
                    } else {
                        None
                    }
                })
            })
            .unwrap_or(generic);

        self.create_row_task(
            library_idx,
            &resolved_table,
            class,
            AfterRowCreated::SelectInBrowser,
        )
    }

    /// Phase 2 — open the delete-row confirm modal. Records
    /// `(table, row_id, internal_pn)` on the browser state so the
    /// modal can render a confident message.
    pub(super) fn handle_browser_delete_row_request(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        if self.refuse_read_only_write(&library_path) {
            return Task::none();
        }
        let internal_pn = self
            .library
            .library_at(&library_path)
            .and_then(|lib| lib.tables.get(&table))
            .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
            .map(|r| r.internal_pn.as_str().to_string())
            .unwrap_or_else(|| format!("row {row_id}"));
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.delete_confirm = Some(DeleteConfirmState {
                table,
                row_id,
                internal_pn,
            });
        }
        Task::none()
    }

    /// Open the Edit Component Details modal for a row. Loads the row
    /// from the library cache and seeds the modal with a working copy.
    pub(super) fn handle_browser_open_edit_modal(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        if self.refuse_read_only_write(&library_path) {
            return Task::none();
        }
        let row = self
            .library
            .library_at(&library_path)
            .and_then(|lib| lib.tables.get(&table))
            .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
            .cloned();
        let Some(row) = row else {
            tracing::warn!(
                target: "signex::library",
                path = %library_path.display(),
                table = %table,
                row = %row_id,
                "browser open edit modal: row not found in cache"
            );
            return Task::none();
        };
        let address = EditorAddress::new(library_path.clone(), table, row_id);
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.edit_modal = Some(EditRowModalState::new(address, row));
        }
        Task::none()
    }

    /// Apply a `BrowserEditMsg` to the active edit modal for `library_path`.
    pub(super) fn handle_browser_edit_msg(
        &mut self,
        library_path: std::path::PathBuf,
        msg: BrowserEditMsg,
    ) -> Task<Message> {
        if self.refuse_read_only_write(&library_path) {
            return Task::none();
        }
        // Some variants need to fire follow-up tasks (open picker,
        // close modal). We collect those into `next` and return them
        // after releasing the borrow.
        let mut next: Option<Task<Message>> = None;
        // Save needs a separate path — we read the draft, drop the
        // borrow, run the adapter call, then resume.
        let mut save_request: Option<(EditorAddress, signex_library::ComponentRow)> = None;
        let mut close_modal = false;
        // Replace the draft with the stored row once the borrow is released.
        let mut reload_address: Option<EditorAddress> = None;
        // The class template of the row in the modal: its slots fix units
        // and carry the sign rules.
        let template: Option<signex_library::ParameterTemplate> = self
            .library
            .library_browsers
            .get(&library_path)
            .and_then(|b| b.edit_modal.as_ref())
            .and_then(|m| {
                self.library
                    .row_template(&library_path, m.draft.class.as_str())
            })
            .cloned();
        let template = template.as_ref();
        if let Some(state) = self.library.library_browsers.get_mut(&library_path)
            && let Some(modal) = state.edit_modal.as_mut()
        {
            match msg {
                BrowserEditMsg::SetInternalPn(s) => {
                    modal.draft.internal_pn = signex_library::InternalPn::new(s);
                    modal.error = None;
                }
                BrowserEditMsg::SetClass(class) => {
                    modal.draft.class = class;
                    modal.error = None;
                }
                BrowserEditMsg::SetState(state_v) => {
                    modal.draft.state = state_v;
                    modal.error = None;
                }
                BrowserEditMsg::SetDatasheetUrl(_)
                    if !matches!(
                        modal.draft.datasheet,
                        signex_library::DatasheetRef::Url { .. }
                    ) => {}
                BrowserEditMsg::SetDatasheetUrl(s) => {
                    // The modal shows no label: the primary keeps its
                    // label while it points where it did when the modal
                    // opened, and drops it once re-pointed (#1231).
                    modal.draft.datasheet = signex_library::DatasheetRef::url(s);
                    let (opened, label) = &modal.opened_primary;
                    modal.draft.datasheet_label = if modal.draft.datasheet == *opened {
                        label.clone()
                    } else {
                        String::new()
                    };
                    modal.error = None;
                }
                BrowserEditMsg::SetManufacturer(s) => {
                    modal.draft.primary_mpn.manufacturer = s;
                    modal.error = None;
                }
                BrowserEditMsg::SetMpn(s) => {
                    modal.draft.primary_mpn.mpn = s;
                    modal.error = None;
                }
                BrowserEditMsg::SetDescription(s) => {
                    let s = single_line(&s);
                    // `'''` ends the table's TOML literal string, so save
                    // would refuse it; refuse the keystroke instead.
                    if s.contains("'''") {
                        modal.error = Some("A description cannot contain '''.".to_string());
                    } else {
                        modal.draft.description = s;
                        modal.error = None;
                    }
                }
                BrowserEditMsg::SetParamValue { key, value } => {
                    let entry = modal
                        .param_buf
                        .entry(key.clone())
                        .or_insert_with(|| (String::new(), String::new()));
                    entry.0 = value;
                    // A prefix or unit typed into the number is refused at
                    // once. Another reason that is showing stays until its
                    // own field changes.
                    let prefix = format!("Parameter \"{key}\"");
                    match param_commit::number_field_problem(modal, &key, template) {
                        Some(problem) => modal.error = Some(problem),
                        None if modal
                            .error
                            .as_deref()
                            .is_some_and(|e| e.starts_with(&prefix)) =>
                        {
                            modal.error = None;
                        }
                        None => {}
                    }
                }
                BrowserEditMsg::SetParamUnit { key, unit } => {
                    param_commit::pick_unit(modal, &key, unit, template);
                }
                BrowserEditMsg::ParamPrefixPicked { key, prefix } => {
                    modal.param_prefix.insert(key.clone(), prefix);
                    param_commit::commit_param_buffer(modal, &key, template);
                }
                BrowserEditMsg::CommitParam { key } => {
                    param_commit::commit_param_buffer(modal, &key, template)
                }
                BrowserEditMsg::AddParam => {
                    // Find a unique key like "param_N".
                    let mut idx = modal.draft.parameters.len() + 1;
                    let key = loop {
                        let candidate = format!("param_{idx}");
                        if !modal.draft.parameters.contains_key(&candidate) {
                            break candidate;
                        }
                        idx += 1;
                    };
                    modal
                        .draft
                        .parameters
                        .insert(key.clone(), signex_library::ParamValue::Text(String::new()));
                    modal.param_buf.insert(key, (String::new(), String::new()));
                }
                BrowserEditMsg::DeleteParam { key } => {
                    modal.draft.parameters.remove(&key);
                    modal.param_buf.remove(&key);
                    modal.param_prefix.remove(&key);
                }
                BrowserEditMsg::SetTags(s) => {
                    modal.tags_buf = s;
                    modal.error = None;
                }
                BrowserEditMsg::OpenSymbolPicker => {
                    next = Some(Task::done(Message::Library(
                        LibraryMessage::OpenPrimitivePicker {
                            kind: PrimitiveKind::Symbol,
                            target: PrimitivePickerTarget::EditRowModal(modal.address.clone()),
                        },
                    )));
                }
                BrowserEditMsg::OpenFootprintPicker => {
                    next = Some(Task::done(Message::Library(
                        LibraryMessage::OpenPrimitivePicker {
                            kind: PrimitiveKind::Footprint,
                            target: PrimitivePickerTarget::EditRowModal(modal.address.clone()),
                        },
                    )));
                }
                // The row is being written: a second Save would be refused
                // as stale, and a reload would drop the draft being saved.
                BrowserEditMsg::Save | BrowserEditMsg::ReloadConfirmed
                    if modal.saving.is_some() =>
                {
                    crate::diagnostics::log_info(
                        "This row is being saved; try again once the save has finished.",
                    );
                }
                BrowserEditMsg::Save => {
                    // Typed-but-not-Entered parameter edits live only in
                    // `param_buf`; commit them before the draft snapshot,
                    // and refuse the save if one is invalid.
                    match param_commit::uncommitted_param_edits(
                        &modal.draft.parameters,
                        &modal.param_buf,
                        &modal.param_prefix,
                        template,
                    ) {
                        Ok(edits) => modal.draft.parameters.extend(edits),
                        Err((key, reason)) => {
                            modal.error = Some(format!(
                                "Parameter \"{key}\" has an invalid value ({reason}); fix it before saving."
                            ));
                            return Task::none();
                        }
                    }
                    // Flush the tags buffer to `parameters["tags"]`
                    // before snapshotting the draft. Empty buffer drops
                    // the entry so we don't keep a dangling empty
                    // string in the param map.
                    let trimmed = modal.tags_buf.trim();
                    if trimmed.is_empty() {
                        modal.draft.parameters.remove("tags");
                    } else {
                        modal.draft.parameters.insert(
                            "tags".to_string(),
                            signex_library::ParamValue::Text(trimmed.to_string()),
                        );
                    }
                    save_request = Some((modal.address.clone(), modal.draft.clone()));
                }
                BrowserEditMsg::Cancel => {
                    close_modal = true;
                }
                BrowserEditMsg::ReloadRequested => modal.reload_confirming = true,
                BrowserEditMsg::ReloadCancelled => modal.reload_confirming = false,
                BrowserEditMsg::ReloadConfirmed => {
                    reload_address = Some(modal.address.clone());
                }
            }
        }
        if close_modal && let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.edit_modal = None;
        }
        if let Some(address) = reload_address {
            self.reload_edit_modal_from_server(&library_path, address);
        }
        let mut refresh = Task::none();
        if let Some((address, draft)) = save_request {
            refresh = self.start_modal_row_save(&library_path, address, draft);
        }
        Task::batch([refresh, next.unwrap_or_else(Task::none)])
    }

    /// Open the Component Preview tab for `(library_path, table, row_id)`.
    /// Re-uses the existing tab if one is already open.
    pub(super) fn handle_open_component_row(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        let address = EditorAddress::new(library_path.clone(), table.clone(), row_id);
        let synthetic_path = address.synthetic_tab_path();

        if let Some(idx) = self
            .document_state
            .tabs
            .iter()
            .position(|t| t.path == synthetic_path)
        {
            if idx != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = idx;
                self.sync_active_tab();
            }
            return Task::none();
        }

        // Pre-load the row from the adapter via `read_row`; if it
        // fails we surface and bail without leaving an empty tab
        // behind.
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    "open component row: library not open"
                );
                return Task::none();
            }
        };
        let row_result = self
            .library
            .set
            .get(library_id)
            .ok_or_else(|| {
                signex_library::LibraryError::NotFound(library_path.display().to_string())
            })
            .and_then(|adapter| adapter.read_row(&table, row_id));
        let row = match row_result {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(target: "signex::library", error = %e, "open component row: read_row failed");
                return Task::none();
            }
        };

        let title = row.internal_pn.as_str().to_string();
        let project_id = self
            .document_state
            .project_for_path(&synthetic_path)
            .map(|p| p.id);
        // A dirty draft parked by a tab close wins over the stored row; a
        // clean parked editor is stale and is replaced.
        if !self
            .library
            .editors
            .get(&address)
            .is_some_and(|editor| editor.dirty)
        {
            let preview = ComponentPreviewState::from_row(library_path.clone(), table.clone(), row);
            self.library.editors.insert(address.clone(), preview);
        }
        self.park_active_schematic_session();
        self.document_state.tabs.push(crate::app::TabInfo {
            title,
            path: synthetic_path,
            cached_document: None,
            project_id,
            kind: crate::app::TabKind::ComponentEditor(crate::app::ComponentEditorTab {
                library_path: address.library_path.clone(),
                table: address.table.clone(),
                row_id: address.row_id,
            }),
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        self.document_state.active_path = None;
        self.refresh_panel_ctx();
        Task::none()
    }
}

/// `s` with each run of control characters replaced by one space: a tab or
/// newline cannot live in a table cell, so pasted multi-line text stays
/// readable instead of running together.
fn single_line(s: &str) -> String {
    s.split(char::is_control)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
