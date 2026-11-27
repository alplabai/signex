//! Primitive-picker handlers — routing the symbol / footprint picker
//! modal and applying a pick to either a browser row or the active
//! Component Preview.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;
use crate::library::refresh::RefreshFollowUp;
use crate::library::resolve::{ResolvedKind, report_read_failure};
use crate::library::writes::{RowEdit, RowPatch, RowStart, WriteFailure, WriteKind, row_edit_task};

impl Signex {
    /// Open the Symbol/Footprint primitive picker modal. `target`
    /// determines what happens when the user picks something.
    pub(super) fn handle_open_primitive_picker(
        &mut self,
        kind: PrimitiveKind,
        target: PrimitivePickerTarget,
    ) -> Task<Message> {
        // Every pick except the New Component form ends in an
        // `update_row` on the target's library; a read-only mount
        // refuses up front with the plain-language card instead of
        // opening a picker whose result could only fail silently.
        if target
            .library_paths()
            .into_iter()
            .any(|library_path| self.refuse_read_only_write(library_path))
        {
            return Task::none();
        }
        self.library.primitive_picker = Some(PrimitivePickerState {
            kind,
            target,
            filter: String::new(),
            error: None,
            scroll_y: 0.0,
            loose: Vec::new(),
            copy_confirm: None,
            copying: false,
            opening_library: None,
            selection: Default::default(),
        });
        self.list_loose_symbols_task(kind)
    }

    /// Apply a primitive picker sub-message. Most variants close the
    /// modal once the pick lands.
    pub(super) fn handle_primitive_picker_msg(&mut self, msg: PrimitivePickerMsg) -> Task<Message> {
        match msg {
            PrimitivePickerMsg::SetFilter(s) => {
                if let Some(picker) = self.library.primitive_picker.as_mut() {
                    picker.filter = s;
                    picker.error = None;
                }
                Task::none()
            }
            PrimitivePickerMsg::Scrolled(offset_y) => {
                if let Some(picker) = self.library.primitive_picker.as_mut() {
                    picker.scroll_y = offset_y;
                }
                Task::none()
            }
            PrimitivePickerMsg::Cancel => {
                self.library.primitive_picker = None;
                Task::none()
            }
            PrimitivePickerMsg::Pick(primitive_ref) => self.apply_primitive_pick(primitive_ref),
            PrimitivePickerMsg::Browse => {
                let kind = self
                    .library
                    .primitive_picker
                    .as_ref()
                    .map(|p| p.kind)
                    .unwrap_or(PrimitiveKind::Symbol);
                use signex_i18n::ids;
                let (label, ext) = match kind {
                    PrimitiveKind::Symbol => (ids::FILE_DIALOG_PICK_SYMBOL, "snxsym"),
                    PrimitiveKind::Footprint => (ids::FILE_DIALOG_PICK_FOOTPRINT, "snxfpt"),
                    PrimitiveKind::Sim => (ids::FILE_DIALOG_PICK_SIM_MODEL, "snxsim"),
                    _ => (ids::FILE_DIALOG_PICK_PRIMITIVE, ""),
                };
                let dialog = self
                    .ui_state
                    .io
                    .picker
                    .dialog()
                    .title_in(&self.ui_state.i18n, label);
                Task::perform(
                    async move {
                        // The filter is named by its extension, not a label.
                        dialog.add_filter(ext, &[ext]).pick_file().await
                    },
                    |path| {
                        Message::Library(LibraryMessage::PrimitivePicker(
                            PrimitivePickerMsg::BrowseResult(path),
                        ))
                    },
                )
            }
            PrimitivePickerMsg::BrowseResult(None) => Task::none(),
            PrimitivePickerMsg::BrowseResult(Some(path)) => {
                self.handle_primitive_picker_browse_result(path)
            }
            PrimitivePickerMsg::LooseListed { read, from_browse } => {
                self.handle_loose_listed(&read, from_browse)
            }
            PrimitivePickerMsg::RowHighlighted(choice) => {
                self.handle_picker_row_highlighted(choice)
            }
            PrimitivePickerMsg::PreviewLoaded { generation, result } => {
                self.handle_picker_preview_loaded(generation, result)
            }
            PrimitivePickerMsg::PickLoose(symbol) => self.handle_loose_chosen(symbol),
            PrimitivePickerMsg::CopyConfirmed => self.handle_copy_confirmed(),
            PrimitivePickerMsg::OpenLibraryConfirmed => self.handle_open_library_confirmed(),
            PrimitivePickerMsg::CopyDeclined => self.handle_copy_declined(),
            PrimitivePickerMsg::CopyFinished { pick, result } => {
                self.handle_copy_finished(pick, result)
            }
            PrimitivePickerMsg::SymbolVersionRead {
                target,
                symbol,
                version,
            } => self.symbol_version_read(target, symbol, version),
        }
    }

    /// A primitive ref has been picked — apply it to the picker's
    /// configured target and close the modal.
    fn apply_primitive_pick(&mut self, primitive_ref: PrimitiveRef) -> Task<Message> {
        let Some(picker) = self.library.primitive_picker.take() else {
            return Task::none();
        };
        if picker
            .target
            .library_paths()
            .into_iter()
            .any(|library_path| self.refuse_read_only_write(library_path))
        {
            return Task::none();
        }
        match picker.target {
            PrimitivePickerTarget::PreviewRow(address) => {
                return self.apply_primitive_pick_to_preview(address, picker.kind, primitive_ref);
            }
            PrimitivePickerTarget::EditRowModal(address) => {
                if let Some(state) = self.library.library_browsers.get_mut(&address.library_path)
                    && let Some(modal) = state.edit_modal.as_mut()
                    && modal.address == address
                {
                    match picker.kind {
                        PrimitiveKind::Symbol => {
                            modal.draft.bind_default_symbol(primitive_ref);
                        }
                        PrimitiveKind::Footprint => {
                            modal.draft.footprint_ref = Some(primitive_ref);
                        }
                        PrimitiveKind::Sim => {
                            modal.draft.sim_ref = Some(primitive_ref);
                        }
                        _ => {}
                    }
                    modal.error = None;
                }
            }
            // A symbol is pinned at its current version, read first (#2572).
            target @ (PrimitivePickerTarget::BrowserRow(_)
            | PrimitivePickerTarget::BrowserRows { .. })
                if picker.kind == PrimitiveKind::Symbol =>
            {
                return self.read_symbol_version_then(target, primitive_ref);
            }
            PrimitivePickerTarget::BrowserRow(address) => {
                return self.apply_primitive_pick_to_browser_row(
                    address,
                    picker.kind,
                    primitive_ref,
                    None,
                );
            }
            // Only a symbol pick re-binds rows.
            PrimitivePickerTarget::BrowserRows { .. } => {}
            PrimitivePickerTarget::PreviewAlternate(address) => {
                return self.add_alternate_symbol_from_pick(address, primitive_ref);
            }
            PrimitivePickerTarget::DesignPart { sheet, part } => {
                return Task::done(Message::from(crate::app::UpdatePcbMsg::FootprintChosen {
                    sheet,
                    part,
                    footprint: primitive_ref,
                }));
            }
            PrimitivePickerTarget::BoardFootprint {
                board, footprints, ..
            } => {
                return Task::done(Message::Pcb(crate::app::PcbMsg::FootprintSwap(
                    crate::app::FootprintSwapMsg::Picked {
                        board,
                        footprints,
                        footprint: primitive_ref,
                    },
                )));
            }
            PrimitivePickerTarget::SketchParameters { editor } => {
                return self.offer_parameters_of(editor, primitive_ref);
            }
        }
        Task::none()
    }

    /// Import from another footprint (mockup row 189): hand the picked
    /// footprint's parameters to the sketch table at `editor`. Copies
    /// nothing yet; the table's list asks which to take.
    fn offer_parameters_of(
        &mut self,
        editor: std::path::PathBuf,
        primitive_ref: PrimitiveRef,
    ) -> Task<Message> {
        use crate::library::editor::footprint::parameters_table::SketchParametersMsg;
        let Some(footprint) = report_read_failure(
            self.library.set.resolve_footprint(&primitive_ref),
            ResolvedKind::Footprint,
            &primitive_ref,
            "sketch parameters import",
        ) else {
            crate::diagnostics::log_warning(
                "No parameters were imported: the footprint could not be read.",
            );
            self.sync_diagnostics_panel_ctx();
            return Task::none();
        };
        let parameters: Vec<(String, String)> = footprint
            .sketch
            .as_ref()
            .map(|sketch| {
                sketch
                    .parameters
                    .iter()
                    .map(|(name, expr)| (name.clone(), expr.clone()))
                    .collect()
            })
            .unwrap_or_default();
        Task::done(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: editor,
            msg: crate::library::messages::PrimitiveEdit::Footprint(
                crate::library::messages::FootprintEditorMsg::SketchParameters(
                    SketchParametersMsg::ImportSourcePicked {
                        source: footprint.name,
                        parameters,
                    },
                ),
            ),
        }))
    }

    /// F15 — Library Browser row binding. Same shape as
    /// `apply_primitive_pick_to_preview` but reads/writes the row
    /// through the cache directly because there's no Component
    /// Preview tab open (the user picked from the inline preview /
    /// Properties area). Updates the row, re-hashes, persists via
    /// `adapter.update_row`, refreshes the cache.
    ///
    /// A symbol bound with `symbol_version` is pinned at it (#2572): the
    /// picked symbol's current version, read by `read_symbol_version_then`.
    /// `None` leaves the row's pin as it is.
    pub(super) fn apply_primitive_pick_to_browser_row(
        &mut self,
        address: EditorAddress,
        kind: PrimitiveKind,
        primitive_ref: PrimitiveRef,
        symbol_version: Option<String>,
    ) -> Task<Message> {
        // 1. Read the row from the library cache.
        let Some(mut row) = self.cached_browser_row(&address) else {
            return Task::none();
        };
        // 2. Apply.
        match kind {
            PrimitiveKind::Symbol => {
                row.bind_default_symbol(primitive_ref);
                if let Some(version) = &symbol_version {
                    row.symbol_version = version.clone();
                }
            }
            PrimitiveKind::Footprint => row.footprint_ref = Some(primitive_ref),
            PrimitiveKind::Sim => row.sim_ref = Some(primitive_ref),
            _ => return Task::none(),
        }
        // 3. Re-hash.
        match signex_library::hash_row_content(&row) {
            Ok(h) => row.content_hash = h,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    error = %e,
                    "browser-row primitive pick: hash failed"
                );
                return Task::none();
            }
        }
        // 4. Persist via the adapter, on a worker.
        let commit_msg = bind_message(kind);
        let table = address.table.clone();
        let library_path = address.library_path.clone();
        let row_id = address.row_id;
        let bound = row.clone();
        let patch: RowPatch = Box::new(move |stored| match kind {
            PrimitiveKind::Symbol => {
                stored.bind_default_symbol(primitive_ref);
                if let Some(version) = symbol_version {
                    stored.symbol_version = version;
                }
            }
            PrimitiveKind::Footprint => stored.footprint_ref = bound.footprint_ref,
            _ => stored.sim_ref = bound.sim_ref,
        });
        let kind_done = WriteKind::BrowserRowBound { address, kind };
        let edit = RowEdit {
            table,
            row_id,
            start: RowStart::Full(Box::new(row)),
            patch: Some(patch),
            assets: Vec::new(),
            message: commit_msg,
        };
        row_edit_task(&mut self.library, &library_path, kind_done, edit)
    }

    /// The row at `address` as the library cache holds it.
    fn cached_browser_row(&self, address: &EditorAddress) -> Option<signex_library::ComponentRow> {
        let row = self
            .library
            .library_at(&address.library_path)
            .and_then(|lib| lib.tables.get(&address.table))
            .and_then(|rows| {
                rows.iter()
                    .find(|r| RowId::from_uuid(r.row_id) == address.row_id)
            })
            .cloned();
        if row.is_none() {
            tracing::warn!(
                target: "signex::library",
                library = %address.library_path.display(),
                table = %address.table,
                row_id = %address.row_id,
                "primitive pick: row not found in cache"
            );
        }
        row
    }

    /// The in-use symbol Delete's Re-bind (#2572): `new`, pinned at
    /// `version`, takes the place of `old` in the row at `address`, as
    /// default and as alternate (`ComponentRow::replace_symbol`), written
    /// like a browser row's bind. A row that no longer binds `old` is not
    /// written.
    fn replace_symbol_in_browser_row(
        &mut self,
        address: EditorAddress,
        old: PrimitiveRef,
        new: PrimitiveRef,
        version: String,
    ) -> Task<Message> {
        let Some(mut row) = self.cached_browser_row(&address) else {
            return Task::none();
        };
        if !row.replace_symbol(&old, new, &version) {
            return Task::none();
        }
        match signex_library::hash_row_content(&row) {
            Ok(h) => row.content_hash = h,
            Err(e) => {
                crate::diagnostics::log_warning(format!(
                    "Row {} was not re-bound: its content could not be hashed ({e}).",
                    address.row_id
                ));
                return Task::none();
            }
        }
        let library_path = address.library_path.clone();
        let patch: RowPatch = Box::new(move |stored| {
            stored.replace_symbol(&old, new, &version);
        });
        let edit = RowEdit {
            table: address.table.clone(),
            row_id: address.row_id,
            start: RowStart::Full(Box::new(row)),
            patch: Some(patch),
            assets: Vec::new(),
            message: bind_message(PrimitiveKind::Symbol),
        };
        let done = WriteKind::BrowserRowBound {
            address,
            kind: PrimitiveKind::Symbol,
        };
        row_edit_task(&mut self.library, &library_path, done, edit)
    }

    /// Read the current version of the picked `symbol` on a worker, then
    /// bind it to the rows of `target` pinned at it (`symbol_version_read`).
    fn read_symbol_version_then(
        &mut self,
        target: PrimitivePickerTarget,
        symbol: PrimitiveRef,
    ) -> Task<Message> {
        let Some(adapter) = self.library.set.get_shared(symbol.library_id) else {
            crate::diagnostics::log_warning(
                "The symbol was not bound: the library that holds it is not open.",
            );
            self.sync_diagnostics_panel_ctx();
            return Task::none();
        };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    adapter
                        .get_symbol(symbol.uuid)
                        .map(|s| s.version)
                        .map_err(|e| e.to_string())
                })
                .await
                .unwrap_or_else(|e| Err(format!("the read task failed: {e}")))
            },
            move |version| {
                Message::Library(LibraryMessage::PrimitivePicker(
                    PrimitivePickerMsg::SymbolVersionRead {
                        target: target.clone(),
                        symbol,
                        version,
                    },
                ))
            },
        )
    }

    /// The picked symbol's version was read: bind it, pinned at that
    /// version. An unreadable version binds nothing.
    fn symbol_version_read(
        &mut self,
        target: PrimitivePickerTarget,
        symbol: PrimitiveRef,
        version: Result<String, String>,
    ) -> Task<Message> {
        let version = match version {
            Ok(version) => version,
            Err(why) => {
                crate::diagnostics::log_warning(format!(
                    "The symbol was not bound: its current version could not be read ({why})."
                ));
                self.sync_diagnostics_panel_ctx();
                return Task::none();
            }
        };
        // The library may have become read-only while the version was read.
        if target
            .library_paths()
            .into_iter()
            .any(|library_path| self.refuse_read_only_write(library_path))
        {
            return Task::none();
        }
        match target {
            PrimitivePickerTarget::BrowserRow(address) => self.apply_primitive_pick_to_browser_row(
                address,
                PrimitiveKind::Symbol,
                symbol,
                Some(version),
            ),
            PrimitivePickerTarget::BrowserRows { rows, replacing } => {
                let binds: Vec<_> = rows
                    .into_iter()
                    .map(|address| {
                        self.replace_symbol_in_browser_row(
                            address,
                            replacing,
                            symbol,
                            version.clone(),
                        )
                    })
                    .collect();
                Task::batch(binds)
            }
            _ => Task::none(),
        }
    }

    /// The browser row's primitive bind finished.
    pub(super) fn browser_row_bound(
        &mut self,
        address: EditorAddress,
        outcome: Result<Option<u64>, WriteFailure>,
    ) -> Task<Message> {
        if let Err(failure) = &outcome {
            crate::diagnostics::log_warning(format!(
                "Primitive binding for row {} was not saved: {}",
                address.row_id, failure.reason
            ));
            // A refused write leaves the cache on the stale base, so every
            // retry would be refused again: re-read the rows.
            let refresh =
                self.refresh_library_task(&address.library_path, RefreshFollowUp::Nothing);
            self.sync_diagnostics_panel_ctx();
            return refresh;
        }
        // 5. Refresh cache.
        self.refresh_library_task(
            &address.library_path,
            RefreshFollowUp::CleanEditors(vec![address.row_id]),
        )
    }

    /// Component Preview tab — apply a freshly-picked primitive ref to
    /// the row, resolve through the LibrarySet, save via update_row.
    pub(super) fn apply_primitive_pick_to_preview(
        &mut self,
        address: EditorAddress,
        kind: PrimitiveKind,
        primitive_ref: PrimitiveRef,
    ) -> Task<Message> {
        let Some(state) = self.library.editors.get_mut(&address) else {
            return Task::none();
        };
        if state.binding_primitive {
            crate::diagnostics::log_warning(
                "The primitive was not bound: another bind of this row is still being saved. Bind it again once that has finished.",
            );
            self.sync_diagnostics_panel_ctx();
            return Task::none();
        }
        if state.saving_row.is_some() {
            // The bind would move the row's revision under the running
            // save and fail it as stale: ask for it again afterwards.
            crate::diagnostics::log_warning(
                "The primitive was not bound: this row is being saved. Bind it again once the save has finished.",
            );
            self.sync_diagnostics_panel_ctx();
            return Task::none();
        }
        match kind {
            PrimitiveKind::Symbol => {
                state.row.bind_default_symbol(primitive_ref);
                state.symbol = report_read_failure(
                    self.library.set.resolve_symbol(&primitive_ref),
                    ResolvedKind::Symbol,
                    &primitive_ref,
                    "primitive pick",
                );
            }
            PrimitiveKind::Footprint => {
                state.row.footprint_ref = Some(primitive_ref);
                state.set_footprint(report_read_failure(
                    self.library.set.resolve_footprint(&primitive_ref),
                    ResolvedKind::Footprint,
                    &primitive_ref,
                    "primitive pick",
                ));
            }
            PrimitiveKind::Sim => {
                state.row.sim_ref = Some(primitive_ref);
                state.sim = report_read_failure(
                    self.library.set.resolve_sim(&primitive_ref),
                    ResolvedKind::Sim,
                    &primitive_ref,
                    "primitive pick",
                );
            }
            _ => return Task::none(),
        }
        // Only the bound field is persisted: the working copy may pin a
        // datasheet PDF that is staged and not on disk yet (#1332), and the
        // preview's other unsaved edits are saved by its own Save. The
        // stored row keeps the editor's base revision, so a row that moved
        // on is still refused as stale.
        let base_revision = state.row.server_revision;
        state.binding_primitive = true;
        // The row changed under any reload still being read.
        state.note_edit();
        // No snapshot holds the resolved primitive: undo cannot cross a bind.
        state.restart_history();
        let table = address.table.clone();
        let row_id = address.row_id;
        let msg = bind_message(kind);
        let library_path = address.library_path.clone();
        let done = WriteKind::PreviewRowBound { address, kind };
        let patch: RowPatch = Box::new(move |stored| match kind {
            PrimitiveKind::Symbol => stored.bind_default_symbol(primitive_ref),
            PrimitiveKind::Footprint => stored.footprint_ref = Some(primitive_ref),
            _ => stored.sim_ref = Some(primitive_ref),
        });
        let edit = RowEdit {
            table,
            row_id,
            start: RowStart::FromStored { base_revision },
            patch: Some(patch),
            assets: Vec::new(),
            message: msg,
        };
        row_edit_task(&mut self.library, &library_path, done, edit)
    }

    /// The Component Preview's primitive bind finished.
    pub(super) fn preview_row_bound(
        &mut self,
        address: EditorAddress,
        outcome: Result<Option<u64>, WriteFailure>,
    ) -> Task<Message> {
        if let Some(state) = self.library.editors.get_mut(&address) {
            state.binding_primitive = false;
        }
        match outcome {
            // Keep the editor's base revision current, or its next save
            // would be refused as stale.
            Ok(revision) => {
                if let Some(state) = self.library.editors.get_mut(&address) {
                    state.row.server_revision = revision;
                }
            }
            Err(failure) => {
                if let Some(state) = self.library.editors.get_mut(&address) {
                    // The bound row never reached disk: no clean mark holds.
                    state.history.forget();
                    state.dirty = true;
                    // Offer "Reload saved version" when the row moved on.
                    if failure.row_changed {
                        state.stale_base = true;
                    }
                }
                crate::diagnostics::log_warning(format!(
                    "Primitive binding for row {} was not saved: {}",
                    address.row_id, failure.reason
                ));
                self.sync_diagnostics_panel_ctx();
                return Task::none();
            }
        }
        self.refresh_library_task(&address.library_path, RefreshFollowUp::Nothing)
    }

    /// Filesystem-picked primitive. A file of a MOUNTED library binds from
    /// that library (`library_holding`, built on the one ownership rule); a symbol
    /// file of an unmounted one is classified off-thread with
    /// `enclosing_snxlib`.
    fn handle_primitive_picker_browse_result(&mut self, file: std::path::PathBuf) -> Task<Message> {
        let Some(kind) = self.library.primitive_picker.as_ref().map(|p| p.kind) else {
            return Task::none();
        };
        if kind == PrimitiveKind::Symbol {
            // The read answers mounted (bind) or unmounted (copy / open the
            // library, #1841) off-thread; see `handle_loose_listed`.
            return self.read_browsed_loose_symbol(file);
        }
        let Some(library_id) = self
            .library
            .library_holding_with(&file, kind, self.identity_fold_for(&file))
            .map(|lib| lib.library_id)
        else {
            let folder = match kind {
                PrimitiveKind::Footprint => "footprints",
                _ => "sims",
            };
            let message = format!(
                "{}: not in the `{folder}/` folder of an open library.",
                file.display()
            );
            crate::diagnostics::log_warning(&message);
            if let Some(picker) = self.library.primitive_picker.as_mut() {
                picker.error = Some(message);
            }
            return Task::none();
        };
        let uuid = match self.library.primitive_uuid_in_file(library_id, &file) {
            Ok(uuid) => uuid,
            Err(why) => {
                if let Some(picker) = self.library.primitive_picker.as_mut() {
                    picker.error = Some(why);
                }
                return Task::none();
            }
        };
        Task::done(Message::Library(LibraryMessage::PrimitivePicker(
            PrimitivePickerMsg::Pick(PrimitiveRef::new(library_id, uuid)),
        )))
    }
}

/// The commit message of binding a primitive of `kind` to a row.
fn bind_message(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => "bind symbol",
        PrimitiveKind::Footprint => "bind footprint",
        PrimitiveKind::Sim => "bind sim",
        _ => "bind primitive",
    }
}
