//! Library Browser tab handlers — opening the browser, adding /
//! deleting / editing component rows, inline cell commits, and
//! opening a Component Preview row from the browser.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;

impl Signex {
    /// Open `.snxlib` at `path` as a Library Browser tab. Mounts the
    /// library if not already mounted, seeds the browser state, and
    /// pushes (or activates) a `TabKind::LibraryBrowser` tab. Phase 1.
    pub(crate) fn handle_open_library_browser(
        &mut self,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        tracing::info!(
            target: "signex::library",
            path = %path.display(),
            exists = path.exists(),
            already_mounted = self.library.library_at(&path).is_some(),
            "open_library_browser: enter"
        );
        // 1. Mount the library if it isn't already. `open_library` is
        //    idempotent — re-mounting an already-open library is a
        //    no-op.
        if let Err(e) = commands::open_library(&mut self.library, path.clone()) {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "open_library_browser: open_library failed"
            );
        }

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
        // Reading the prefs file every open is fine — single-digit
        // milliseconds and only on user gesture.
        let persisted_search = crate::fonts::read_library_browser_searches()
            .remove(&path)
            .unwrap_or_default();

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
            dirty: false,
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

        match commands::create_component_row(
            &mut self.library,
            library_idx,
            &resolved_table,
            "", // empty PN — user fills it in via inline cell editor
            class,
            None, // symbol_ref bound later via Properties panel
            None, // footprint_ref bound later
        ) {
            Ok(row_id) => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.active_table = Some(resolved_table.clone());
                    state.selected_row = Some(row_id);
                }
                tracing::info!(
                    target: "signex::library",
                    library = %library_path.display(),
                    table = %resolved_table,
                    row_id = %row_id,
                    "browser: minted draft row inline"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    library = %library_path.display(),
                    table = %resolved_table,
                    error = %e,
                    "browser: inline row create failed"
                );
            }
        }
        Task::none()
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

    /// Confirm step — actually delete the row through
    /// `adapter.delete_row` and refresh the cache.
    pub(super) fn handle_browser_delete_row_confirm(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    "browser delete: library not mounted"
                );
                return Task::none();
            }
        };
        let adapter = match self.library.set.get(library_id) {
            Some(a) => a,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    "browser delete: adapter not present in set"
                );
                return Task::none();
            }
        };
        match adapter.delete_row(&table, row_id, "delete row") {
            Ok(_) => {
                tracing::info!(
                    target: "signex::library",
                    path = %library_path.display(),
                    table = %table,
                    row = %row_id,
                    "browser delete: row removed"
                );
                if let Err(e) = self.library.refresh_components(&library_path) {
                    tracing::warn!(
                        target: "signex::library",
                        path = %library_path.display(),
                        error = %e,
                        "browser delete: refresh_components failed"
                    );
                }
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    if state.selected_row == Some(row_id) {
                        state.selected_row = None;
                    }
                    state.delete_confirm = None;
                    // Drop any cached cell-edit buffers for the gone row.
                    state.cell_edit.retain(|(rid, _), _| *rid != row_id);
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    table = %table,
                    error = %e,
                    "browser delete: delete_row failed"
                );
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.delete_confirm = None;
                }
            }
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
        // Some variants need to fire follow-up tasks (open picker,
        // close modal). We collect those into `next` and return them
        // after releasing the borrow.
        let mut next: Option<Task<Message>> = None;
        // Save needs a separate path — we read the draft, drop the
        // borrow, run the adapter call, then resume.
        let mut save_request: Option<(EditorAddress, signex_library::ComponentRow)> = None;
        let mut close_modal = false;
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
                BrowserEditMsg::SetDatasheetUrl(s) => {
                    modal.draft.datasheet = signex_library::DatasheetRef::url(s);
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
                BrowserEditMsg::SetParamValue { key, value } => {
                    let entry = modal
                        .param_buf
                        .entry(key)
                        .or_insert_with(|| (String::new(), String::new()));
                    entry.0 = value;
                }
                BrowserEditMsg::SetParamUnit { key, unit } => {
                    let entry = modal
                        .param_buf
                        .entry(key)
                        .or_insert_with(|| (String::new(), String::new()));
                    entry.1 = unit;
                }
                BrowserEditMsg::CommitParam { key } => {
                    if let Some((value, unit)) = modal.param_buf.get(&key).cloned() {
                        let pv = if !unit.trim().is_empty() {
                            // Try parse as f64 first, otherwise store as text.
                            value
                                .parse::<f64>()
                                .ok()
                                .map(|n| signex_library::ParamValue::Measurement {
                                    value: n,
                                    unit: unit.clone(),
                                })
                                .unwrap_or_else(|| {
                                    signex_library::ParamValue::Text(format!("{value} {unit}"))
                                })
                        } else if let Ok(n) = value.parse::<f64>() {
                            signex_library::ParamValue::Number(n)
                        } else if value.eq_ignore_ascii_case("true") {
                            signex_library::ParamValue::Bool(true)
                        } else if value.eq_ignore_ascii_case("false") {
                            signex_library::ParamValue::Bool(false)
                        } else {
                            signex_library::ParamValue::Text(value)
                        };
                        modal.draft.parameters.insert(key, pv);
                    }
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
                BrowserEditMsg::Save => {
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
            }
        }
        if close_modal && let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.edit_modal = None;
        }
        if let Some((address, mut draft)) = save_request {
            // Refresh content_hash before saving.
            match signex_library::hash_row_content(&draft) {
                Ok(h) => {
                    draft.content_hash = h;
                }
                Err(e) => {
                    if let Some(state) = self.library.library_browsers.get_mut(&library_path)
                        && let Some(modal) = state.edit_modal.as_mut()
                    {
                        modal.error = Some(format!("hash failed: {e}"));
                    }
                    return next.unwrap_or_else(Task::none);
                }
            }
            let library_id = self
                .library
                .library_at(&address.library_path)
                .map(|lib| lib.library_id);
            let result = match library_id.and_then(|id| self.library.set.get(id)) {
                Some(adapter) => adapter.update_row(&address.table, draft, "edit row"),
                None => Err(signex_library::LibraryError::NotFound(
                    address.library_path.display().to_string(),
                )),
            };
            match result {
                Ok(_) => {
                    if let Err(e) = self.library.refresh_components(&address.library_path) {
                        tracing::warn!(
                            target: "signex::library",
                            path = %address.library_path.display(),
                            error = %e,
                            "browser edit: refresh_components failed"
                        );
                    }
                    if let Some(state) =
                        self.library.library_browsers.get_mut(&address.library_path)
                    {
                        state.edit_modal = None;
                    }
                }
                Err(e) => {
                    if let Some(state) = self.library.library_browsers.get_mut(&library_path)
                        && let Some(modal) = state.edit_modal.as_mut()
                    {
                        modal.error = Some(e.to_string());
                    }
                }
            }
        }
        next.unwrap_or_else(Task::none)
    }

    /// Commit a per-cell inline edit to the row. Re-hashes + persists.
    pub(super) fn handle_browser_cell_commit(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
        column: String,
    ) -> Task<Message> {
        // Drop the buffer eagerly — if the save fails we re-insert below.
        let buf = match self
            .library
            .library_browsers
            .get_mut(&library_path)
            .and_then(|s| s.cell_edit.remove(&(row_id, column.clone())))
        {
            Some(v) => v,
            None => return Task::none(),
        };
        // Read the current row from the cache, mutate, re-hash, save.
        let mut row = match self
            .library
            .library_at(&library_path)
            .and_then(|lib| lib.tables.get(&table))
            .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
            .cloned()
        {
            Some(r) => r,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    table = %table,
                    row = %row_id,
                    "browser cell commit: row not found in cache"
                );
                return Task::none();
            }
        };
        match column.as_str() {
            "internal_pn" => {
                row.internal_pn = signex_library::InternalPn::new(buf.clone());
            }
            "manufacturer" => {
                row.primary_mpn.manufacturer = buf.clone();
            }
            "mpn" => {
                row.primary_mpn.mpn = buf.clone();
            }
            other if other.starts_with("parameters.") => {
                let key = &other["parameters.".len()..];
                // Preserve unit on commit by reading the existing value.
                let new_value = match row.parameters.get(key) {
                    Some(signex_library::ParamValue::Measurement { unit, .. }) => {
                        match buf.parse::<f64>() {
                            Ok(n) => signex_library::ParamValue::Measurement {
                                value: n,
                                unit: unit.clone(),
                            },
                            Err(_) => signex_library::ParamValue::Text(buf.clone()),
                        }
                    }
                    Some(signex_library::ParamValue::Number(_)) => match buf.parse::<f64>() {
                        Ok(n) => signex_library::ParamValue::Number(n),
                        Err(_) => signex_library::ParamValue::Text(buf.clone()),
                    },
                    Some(signex_library::ParamValue::Bool(_)) => {
                        if buf.eq_ignore_ascii_case("true") {
                            signex_library::ParamValue::Bool(true)
                        } else if buf.eq_ignore_ascii_case("false") {
                            signex_library::ParamValue::Bool(false)
                        } else {
                            signex_library::ParamValue::Text(buf.clone())
                        }
                    }
                    _ => signex_library::ParamValue::Text(buf.clone()),
                };
                row.parameters.insert(key.to_string(), new_value);
            }
            _ => {
                tracing::warn!(
                    target: "signex::library",
                    column = %column,
                    "browser cell commit: unknown column"
                );
                return Task::none();
            }
        }
        match signex_library::hash_row_content(&row) {
            Ok(h) => row.content_hash = h,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    error = %e,
                    "browser cell commit: hash failed; reverting buffer"
                );
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.cell_edit.insert((row_id, column), buf);
                }
                return Task::none();
            }
        }
        let library_id = self
            .library
            .library_at(&library_path)
            .map(|lib| lib.library_id);
        let result = match library_id.and_then(|id| self.library.set.get(id)) {
            Some(adapter) => adapter.update_row(&table, row, "edit cell"),
            None => Err(signex_library::LibraryError::NotFound(
                library_path.display().to_string(),
            )),
        };
        if let Err(e) = result {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "browser cell commit: update_row failed"
            );
            if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                state.cell_edit.insert((row_id, column), buf);
            }
            return Task::none();
        }
        if let Err(e) = self.library.refresh_components(&library_path) {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "browser cell commit: refresh_components failed"
            );
        }
        Task::none()
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
        let preview = ComponentPreviewState::from_row(library_path.clone(), table.clone(), row);
        self.library.editors.insert(address.clone(), preview);
        self.park_active_schematic_session();
        self.document_state.tabs.push(crate::app::TabInfo {
            title,
            path: synthetic_path,
            cached_document: None,
            dirty: false,
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
