//! Primitive-picker handlers — routing the symbol / footprint picker
//! modal and applying a pick to either a browser row or the active
//! Component Preview.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;
use crate::library::refresh::RefreshFollowUp;
use crate::library::resolve::{ResolvedKind, report_read_failure};

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
        if let Some(library_path) = target.library_path()
            && self.refuse_read_only_write(library_path)
        {
            return Task::none();
        }
        self.library.primitive_picker = Some(PrimitivePickerState {
            kind,
            target,
            filter: String::new(),
            error: None,
        });
        Task::none()
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
                let (label, ext) = match kind {
                    PrimitiveKind::Symbol => ("Pick Symbol (*.snxsym)", "snxsym"),
                    PrimitiveKind::Footprint => ("Pick Footprint (*.snxfpt)", "snxfpt"),
                    PrimitiveKind::Sim => ("Pick Sim Model (*.snxsim)", "snxsim"),
                    _ => ("Pick Primitive", ""),
                };
                Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .set_title(label)
                            .add_filter(ext, &[ext])
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
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
        }
    }

    /// A primitive ref has been picked — apply it to the picker's
    /// configured target and close the modal.
    fn apply_primitive_pick(&mut self, primitive_ref: PrimitiveRef) -> Task<Message> {
        let Some(picker) = self.library.primitive_picker.take() else {
            return Task::none();
        };
        if let Some(library_path) = picker.target.library_path()
            && self.refuse_read_only_write(library_path)
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
            PrimitivePickerTarget::BrowserRow(address) => {
                return self.apply_primitive_pick_to_browser_row(
                    address,
                    picker.kind,
                    primitive_ref,
                );
            }
            PrimitivePickerTarget::PreviewAlternate(address) => {
                self.add_alternate_symbol_from_pick(address, primitive_ref);
            }
        }
        Task::none()
    }

    /// F15 — Library Browser row binding. Same shape as
    /// `apply_primitive_pick_to_preview` but reads/writes the row
    /// through the cache directly because there's no Component
    /// Preview tab open (the user picked from the inline preview /
    /// Properties area). Updates the row, re-hashes, persists via
    /// `adapter.update_row`, refreshes the cache.
    pub(super) fn apply_primitive_pick_to_browser_row(
        &mut self,
        address: EditorAddress,
        kind: PrimitiveKind,
        primitive_ref: PrimitiveRef,
    ) -> Task<Message> {
        // 1. Read the row from the library cache.
        let mut row = match self
            .library
            .library_at(&address.library_path)
            .and_then(|lib| lib.tables.get(&address.table))
            .and_then(|rows| {
                rows.iter()
                    .find(|r| RowId::from_uuid(r.row_id) == address.row_id)
            })
            .cloned()
        {
            Some(r) => r,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    library = %address.library_path.display(),
                    table = %address.table,
                    row_id = %address.row_id,
                    "primitive pick: row not found in cache"
                );
                return Task::none();
            }
        };
        // 2. Apply.
        match kind {
            PrimitiveKind::Symbol => row.bind_default_symbol(primitive_ref),
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
        // 4. Persist via adapter.
        let library_id = self
            .library
            .library_at(&address.library_path)
            .map(|lib| lib.library_id);
        let commit_msg = match kind {
            PrimitiveKind::Symbol => "bind symbol",
            PrimitiveKind::Footprint => "bind footprint",
            PrimitiveKind::Sim => "bind sim",
            _ => "bind primitive",
        };
        let result = match library_id.and_then(|id| self.library.set.get(id)) {
            Some(adapter) => adapter.update_row(&address.table, row, commit_msg),
            None => Err(signex_library::LibraryError::NotFound(
                address.library_path.display().to_string(),
            )),
        };
        if let Err(e) = result {
            crate::diagnostics::log_warning(format!(
                "Primitive binding for row {} was not saved: {e}",
                address.row_id
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
                state.footprint = report_read_failure(
                    self.library.set.resolve_footprint(&primitive_ref),
                    ResolvedKind::Footprint,
                    &primitive_ref,
                    "primitive pick",
                );
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
        let table = address.table.clone();
        let row_id = address.row_id;
        let library_id = self
            .library
            .library_at(&address.library_path)
            .map(|lib| lib.library_id);
        let msg = match kind {
            PrimitiveKind::Symbol => "bind symbol",
            PrimitiveKind::Footprint => "bind footprint",
            PrimitiveKind::Sim => "bind sim",
            _ => "bind primitive",
        };
        let result = match library_id.and_then(|id| self.library.set.get(id)) {
            Some(adapter) => adapter.read_row(&table, row_id).and_then(|mut stored| {
                match kind {
                    PrimitiveKind::Symbol => stored.bind_default_symbol(primitive_ref),
                    PrimitiveKind::Footprint => stored.footprint_ref = Some(primitive_ref),
                    _ => stored.sim_ref = Some(primitive_ref),
                }
                stored.server_revision = base_revision;
                stored.refresh_content_hash().map_err(|e| {
                    signex_library::LibraryError::Backend(format!("hash row content: {e}"))
                })?;
                adapter.update_row(&table, stored, msg)
            }),
            None => Err(signex_library::LibraryError::NotFound(
                address.library_path.display().to_string(),
            )),
        };
        match result {
            // Keep the editor's base revision current, or its next save
            // would be refused as stale.
            Ok(revision) => {
                if let Some(state) = self.library.editors.get_mut(&address) {
                    state.row.server_revision = revision;
                }
            }
            Err(e) => {
                // Offer "Reload saved version" when the row moved on.
                if matches!(e, signex_library::LibraryError::RowChanged { .. })
                    && let Some(state) = self.library.editors.get_mut(&address)
                {
                    state.stale_base = true;
                }
                crate::diagnostics::log_warning(format!(
                    "Primitive binding for row {} was not saved: {e}",
                    address.row_id
                ));
                self.sync_diagnostics_panel_ctx();
                return Task::none();
            }
        }
        self.refresh_library_task(&address.library_path, RefreshFollowUp::Nothing)
    }

    /// Filesystem-picked primitive — auto-mount the containing
    /// `.snxlib`, then synthesize a Pick.
    fn handle_primitive_picker_browse_result(&mut self, file: std::path::PathBuf) -> Task<Message> {
        // Locate the containing `.snxlib`. Path layout is
        // `<some>/<lib>.snxlib/<symbols|footprints|sims>/<uuid>.<ext>`.
        let snxlib_dir = file
            .ancestors()
            .find(|p| {
                p.extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("snxlib"))
                    .unwrap_or(false)
            })
            .map(|p| p.to_path_buf());
        let Some(snxlib_dir) = snxlib_dir else {
            if let Some(picker) = self.library.primitive_picker.as_mut() {
                picker.error = Some(
                    "Picked file is not inside a `.snxlib` library. v0.9 only supports primitives bound through libraries."
                        .into(),
                );
            }
            return Task::none();
        };
        // Mount the library if not already.
        if let Err(e) = commands::open_library(&mut self.library, snxlib_dir.clone()) {
            tracing::warn!(
                target: "signex::library",
                path = %snxlib_dir.display(),
                error = %e,
                "browse-pick: open_library failed"
            );
            if let Some(picker) = self.library.primitive_picker.as_mut() {
                picker.error = Some(format!("open library failed: {e}"));
            }
            return Task::none();
        }
        // Resolve library_id + parse uuid from filename.
        let library_id = match self.library.library_at(&snxlib_dir) {
            Some(lib) => lib.library_id,
            None => {
                if let Some(picker) = self.library.primitive_picker.as_mut() {
                    picker.error = Some("Library failed to mount.".into());
                }
                return Task::none();
            }
        };
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let uuid = match uuid::Uuid::parse_str(stem) {
            Ok(u) => u,
            Err(_) => {
                if let Some(picker) = self.library.primitive_picker.as_mut() {
                    picker.error = Some(format!(
                        "Filename `{stem}` is not a UUID — pick a primitive file in `<lib>.snxlib/symbols/`."
                    ));
                }
                return Task::none();
            }
        };
        let primitive_ref = PrimitiveRef::new(library_id, uuid);
        Task::done(Message::Library(LibraryMessage::PrimitivePicker(
            PrimitivePickerMsg::Pick(primitive_ref),
        )))
    }
}
