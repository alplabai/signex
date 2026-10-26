//! Primitive-editor handlers — opening `.snxsym` / `.snxfpt` document
//! tabs, routing symbol / footprint edit events, saving primitive
//! tabs, and the associated canvas-cache / external-change plumbing.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;

#[cfg(test)]
mod cache_refresh_tests;
mod cascade;
mod container_save;
mod primitive_save;
#[cfg(test)]
mod primitive_save_fold_tests;
#[cfg(test)]
mod primitive_save_tests;
mod primitive_write;

impl Signex {
    /// Open a standalone primitive editor tab for the file at `path`.
    /// Fired by the Component Preview tab's right-click context menu on
    /// the Symbol / Footprint render panes. Standalone tab opens in WS-7.
    pub(super) fn handle_open_primitive_editor(
        &mut self,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        tracing::info!(
            target: "signex::library",
            path = %path.display(),
            "OpenPrimitiveEditor — standalone document tab opens in WS-7"
        );
        Task::none()
    }

    /// Open a `.snxsym` or `.snxfpt` as a main-window document tab.
    /// Reads the file from disk, builds the matching editor state,
    /// and pushes a `TabKind::SymbolEditor(path)` /
    /// `FootprintEditor(path)` tab into `DocumentState.tabs`.
    ///
    /// Activates an existing tab when the same path is already open
    /// instead of duplicating.
    ///
    /// The error boundary for [`Self::open_primitive`] (#532). Every
    /// failure leg used to be a `tracing::warn!` and a bare return: the
    /// user double-clicked a corrupt `.snxsym` in the project tree and
    /// **nothing happened** — no tab, no card, indistinguishable from a
    /// mis-click. The warning did reach the Messages panel, but that is
    /// a panel you have to already suspect something to open.
    ///
    /// Routes the failure the way `handle_document_file_opened` already
    /// does, plus the shared error card so it is impossible to miss.
    pub(crate) fn handle_open_primitive(&mut self, path: std::path::PathBuf) -> Task<Message> {
        match self.open_primitive(path) {
            Ok(task) => task,
            Err(error) => {
                crate::diagnostics::log_error("Failed to open primitive", &error);
                self.document_state.error_notice =
                    Some(crate::app::state::ErrorNotice::open(format!("{error:#}")));
                Task::none()
            }
        }
    }

    /// Open a `.snxsym` / `.snxfpt` tab, or say why it could not be
    /// opened. Never surfaces anything itself — see the boundary above.
    fn open_primitive(&mut self, path: std::path::PathBuf) -> anyhow::Result<Task<Message>> {
        let path = self.resolve_held_spelling(path);
        // Already open? Just activate the existing tab.
        if let Some(idx) = self.document_state.tabs.iter().position(|t| t.path == path) {
            if idx != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = idx;
                self.sync_active_tab();
            }
            return Ok(Task::none());
        }

        // A dirty draft parked by a tab close wins over the disk copy, and
        // is reachable even if the file has since vanished or broke. A
        // parked editor that is clean is stale and is replaced below.
        if self.primitive_draft_is_dirty(&path) {
            let parked_kind = if self.document_state.symbol_editors.contains_key(&path) {
                Some(crate::app::TabKind::SymbolEditor(path.clone()))
            } else if self.document_state.footprint_editors.contains_key(&path) {
                Some(crate::app::TabKind::FootprintEditor(path.clone()))
            } else {
                None
            };
            if let Some(kind) = parked_kind {
                let title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let project_id = self.document_state.project_for_path(&path).map(|p| p.id);
                self.park_active_schematic_session();
                self.document_state.tabs.push(crate::app::TabInfo {
                    title,
                    path: path.clone(),
                    cached_document: None,
                    dirty: true,
                    project_id,
                    kind,
                });
                self.document_state.active_tab = self.document_state.tabs.len() - 1;
                self.document_state.active_path = None;
                self.refresh_panel_ctx();
                return Ok(Task::none());
            }
        }

        // Dispatch on extension. `.snxsym` → Symbol; `.snxfpt` →
        // Footprint. Anything else is rejected with a tracing warn so
        // a stray dispatch from the project tree doesn't push a
        // bogus tab.
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match ext.as_str() {
            "snxsym" => {
                let bytes = std::fs::read(&path)
                    .map_err(|e| anyhow::anyhow!("could not read {}: {e}", path.display()))?;
                // v0.18.4 — auto-detect TOML vs legacy JSON.
                let file = signex_library::SymbolFile::from_bytes(&bytes)
                    .map_err(|e| anyhow::anyhow!("could not parse {}: {e}", path.display()))?;
                // Reported, not ignored: a container that decodes but
                // holds nothing is a different problem from a corrupt
                // one, and staying silent on it is what #532 is about.
                anyhow::ensure!(
                    !file.symbols.is_empty(),
                    "{} contains no symbols",
                    path.display()
                );

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

                self.park_active_schematic_session();
                self.document_state.tabs.push(crate::app::TabInfo {
                    title,
                    path: path.clone(),
                    cached_document: None,
                    dirty: false,
                    project_id,
                    kind: crate::app::TabKind::SymbolEditor(path.clone()),
                });
                self.document_state.active_tab = self.document_state.tabs.len() - 1;
                // Standalone primitive tabs don't drive `active_path`
                // — clear so the canvas doesn't render a stale schematic.
                self.document_state.active_path = None;
                self.refresh_panel_ctx();
                // An earlier run's recovery copy of this symbol (#1185).
                Ok(self.autosave_check_recovery(&path))
            }
            "snxfpt" => {
                // v0.13.0 — footprint editor gated off for release.
                // A `.snxfpt` opened from the tree / file dialog must
                // not push an editable FootprintEditor tab. Read-only
                // preview + Pick-Footprint binding of existing files
                // stay available elsewhere. Flip
                // `feature_flags::FOOTPRINT_EDITOR_ENABLED` to re-enable.
                if !crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
                    // A deliberate release gate, not a failure — stays
                    // silent, and must not raise the error card.
                    tracing::info!(
                        target: "signex::library",
                        path = %path.display(),
                        "open primitive: footprint editor disabled (v0.13.0) — ignoring .snxfpt open",
                    );
                    return Ok(Task::none());
                }
                let bytes = std::fs::read_to_string(&path)
                    .map_err(|e| anyhow::anyhow!("could not read {}: {e}", path.display()))?;
                // v0.18.4 — parse TOML+TSV envelope and use the first
                // footprint as the editor primitive. Multi-footprint
                // containers are not yet exposed in the editor UI.
                let file = signex_library::FootprintFile::from_toml_str(&bytes)
                    .map_err(|e| anyhow::anyhow!("could not parse {}: {e}", path.display()))?;
                anyhow::ensure!(
                    !file.footprints.is_empty(),
                    "{} contains no footprints",
                    path.display()
                );
                // v0.18.6 — keep the FootprintFile envelope around so
                // saves preserve `file_uuid` + any future multi-
                // footprint siblings instead of minting a fresh
                // single-footprint container each time.
                let display_name = file.footprints[0].name.clone();

                let title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or(display_name);
                let project_id = self.document_state.project_for_path(&path).map(|p| p.id);

                // HI-23: seed snap-disabled from the user's persisted
                // global toggle so opening a .snxfpt while snap is off
                // doesn't reset the editor to snap-on.
                let state = crate::app::FootprintDocument::new(path.clone(), file)
                    .with_global_snap_disabled(!self.ui_state.snap_enabled);
                self.document_state
                    .footprint_editors
                    .insert(path.clone(), state);

                self.park_active_schematic_session();
                self.document_state.tabs.push(crate::app::TabInfo {
                    title,
                    path: path.clone(),
                    cached_document: None,
                    dirty: false,
                    project_id,
                    kind: crate::app::TabKind::FootprintEditor(path.clone()),
                });
                self.document_state.active_tab = self.document_state.tabs.len() - 1;
                self.document_state.active_path = None;
                self.refresh_panel_ctx();
                // An earlier run's recovery copy of this footprint (#1185).
                Ok(self.autosave_check_recovery(&path))
            }
            other => anyhow::bail!(
                "{} is not a symbol or footprint file (.{other})",
                path.display()
            ),
        }
    }

    /// Apply a primitive-editor inner message to the matching tab's
    /// editor state. Path-keyed lookup distinguishes Symbol vs
    /// Footprint; the dispatcher routes to the existing canvas-state
    /// helpers so the standalone tab behaviour matches the in-Component
    /// Editor experience verbatim.
    pub(crate) fn handle_primitive_editor_event(
        &mut self,
        path: std::path::PathBuf,
        msg: PrimitiveEdit,
    ) -> Task<Message> {
        match msg {
            // Save is a sibling of the canvas-mutation messages — route
            // through the standalone save path which writes JSON back to
            // disk and (when applicable) reloads in the LibrarySet. When
            // the file doesn't exist on disk yet (newly-minted in-memory
            // tab from `Add New ▸ Symbol` / `Add New ▸ Footprint`), spawn
            // the Save-As dialog instead so the user picks where it lands
            // — same gate as the top-level `Message::File(FileMsg::Save)` path uses.
            PrimitiveEdit::Save => {
                if !path.exists() {
                    return crate::app::handlers::document_files::spawn_save_as_for_primitive(path);
                }
                match self.save_primitive_tab_at(&path) {
                    Ok(task) => task,
                    Err(e) => {
                        self.report_save_failure("Save failed", &e);
                        Task::none()
                    }
                }
            }
            PrimitiveEdit::Symbol(msg) => self.handle_symbol_primitive_edit(path, msg),
            PrimitiveEdit::Footprint(msg) => self.handle_footprint_primitive_edit(path, msg),
        }
    }

    /// Symbol-tab branch of [`Self::handle_primitive_editor_event`].
    /// Per-library display settings (sheet color, grid, unit) mutate the
    /// shared `OpenLibrary.display`; everything else routes to the
    /// standalone symbol editor keyed by `path`.
    fn handle_symbol_primitive_edit(
        &mut self,
        path: std::path::PathBuf,
        msg: SymbolEditorMsg,
    ) -> Task<Message> {
        // Per-library display settings (sheet color, grid, unit)
        // mutate `OpenLibrary.display` rather than the per-tab editor
        // state — every primitive editor opened from the same
        // `.snxlib` shares the same view settings (Altium "Document
        // Options" parity). Run these before the editor-level
        // dispatch so the editor closure doesn't see them.
        match &msg {
            SymbolEditorMsg::SetSheetColor(color) => {
                let color = *color;
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    lib.display.sheet_color = color;
                }
                self.invalidate_primitive_canvas_cache(&path);
                return Task::none();
            }
            SymbolEditorMsg::ToggleGrid => {
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    lib.display.grid_visible = !lib.display.grid_visible;
                }
                self.invalidate_primitive_canvas_cache(&path);
                return Task::none();
            }
            SymbolEditorMsg::CycleGridSize => {
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    let sizes = crate::canvas::grid::GRID_SIZES_MM;
                    let current_idx = sizes
                        .iter()
                        .position(|s| (s - lib.display.grid_size_mm).abs() < f32::EPSILON)
                        .unwrap_or(2);
                    let next_idx = (current_idx + 1) % sizes.len();
                    lib.display.grid_size_mm = sizes[next_idx];
                }
                self.invalidate_primitive_canvas_cache(&path);
                return Task::none();
            }
            SymbolEditorMsg::CycleUnit => {
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    use signex_types::coord::Unit;
                    lib.display.unit = match lib.display.unit {
                        Unit::Mm => Unit::Mil,
                        Unit::Mil => Unit::Inch,
                        Unit::Inch => Unit::Micrometer,
                        Unit::Micrometer => Unit::Mm,
                    };
                }
                // Unit only affects the status footer text — no
                // canvas redraw needed, but cache clear is harmless
                // and keeps the message handling shape consistent.
                return Task::none();
            }
            _ => {}
        }

        // Symbol-only mutations.
        if let Some(editor) = self.document_state.symbol_editors.get_mut(&path) {
            crate::library::editor::symbol::updates::apply_symbol_primitive_edit(editor, msg);
            // v0.20 — primitive editor edits (designator/size/etc, and
            // critically `placement_paused`) need the panel context
            // rebuilt so the right-dock view reads the new value next
            // frame. Without this the panel renders against stale
            // `FootprintEditorPanelContext` and TAB-pause-driven UI
            // changes (Pad form vs no Pad form) silently miss.
            self.refresh_panel_ctx();
            return Task::none();
        }

        tracing::warn!(
            target: "signex::library",
            path = %path.display(),
            "primitive editor event: no matching tab state",
        );
        Task::none()
    }

    /// Footprint-tab branch of [`Self::handle_primitive_editor_event`].
    /// Clipboard ops split-borrow `pad_clipboard` alongside the editor;
    /// everything else routes to the standalone footprint editor keyed
    /// by `path`.
    fn handle_footprint_primitive_edit(
        &mut self,
        path: std::path::PathBuf,
        msg: FootprintEditorMsg,
    ) -> Task<Message> {
        // v0.26-E — clipboard ops need both `pad_clipboard` and the
        // editor mutable simultaneously, so split-borrow at the call
        // site instead of routing through `apply_footprint_primitive_edit`.
        match &msg {
            FootprintEditorMsg::CopyPad
            | FootprintEditorMsg::CutPad
            | FootprintEditorMsg::PastePad => {
                let crate::app::DocumentState {
                    footprint_editors,
                    pad_clipboard,
                    ..
                } = &mut self.document_state;
                if let Some(editor) = footprint_editors.get_mut(&path) {
                    apply_footprint_clipboard_op(editor, pad_clipboard, &msg);
                }
                self.refresh_panel_ctx();
                return Task::none();
            }
            _ => {}
        }

        // A solve answer finds its editor by the slot's instance, not by the
        // path the task was started under: Save As and a project-tree rename
        // re-key a live editor while its solve is still running.
        let path = match &msg {
            FootprintEditorMsg::SketchSolved { instance, .. } => self
                .document_state
                .footprint_editors
                .iter()
                .find_map(|(p, e)| {
                    (e.state.sketch_solve.instance() == *instance).then(|| p.clone())
                })
                .unwrap_or(path),
            _ => path,
        };

        // Footprint-only mutations.
        if let Some(editor) = self.document_state.footprint_editors.get_mut(&path) {
            // Task 6 — capturing a preset writes straight to disk from
            // inside `apply_footprint_primitive_edit` (which only
            // borrows `editor`, not `self`). Re-read it into
            // `interaction_state` here so the very next
            // `refresh_panel_ctx()` call — a few lines down — shows
            // the new chip immediately instead of waiting for a
            // restart.
            let is_capture_preset = matches!(&msg, FootprintEditorMsg::CaptureFilterPreset);
            apply_footprint_primitive_edit(editor, msg);
            // A drag tick owes a solve, and a finished one may owe the
            // follow-up for the ticks that arrived while it ran (#1047).
            let solve = dispatch_pending(editor, &path).map(Message::Library);
            if is_capture_preset {
                self.interaction_state.footprint_filter_presets =
                    crate::fonts::read_footprint_filter_presets();
            }
            self.refresh_panel_ctx();
            return solve;
        }

        // A solve that lands after its tab's editor is gone has nothing to
        // apply to; that is the expected end of a closed tab, not a fault.
        if matches!(&msg, FootprintEditorMsg::SketchSolved { .. }) {
            tracing::debug!(
                target: "signex::library",
                path = %path.display(),
                "sketch solve answer dropped: no editor has its instance",
            );
            return Task::none();
        }

        tracing::warn!(
            target: "signex::library",
            path = %path.display(),
            "primitive editor event: no matching tab state",
        );
        Task::none()
    }

    /// Clear the canvas cache for the primitive editor tab keyed by
    /// `path`. Used by the per-library display-settings handlers so
    /// the visible canvas redraws as soon as the user flips bg /
    /// grid / etc.
    fn invalidate_primitive_canvas_cache(&mut self, path: &std::path::Path) {
        if let Some(editor) = self.document_state.symbol_editors.get_mut(path) {
            editor.canvas_cache.clear();
        }
        if let Some(editor) = self.document_state.footprint_editors.get_mut(path) {
            editor.canvas_cache.clear();
        }
    }

    /// Refresh the matching library's per-kind primitive cache so the
    /// picker modal sees the just-saved primitive without waiting
    /// for the next full `refresh_components` round-trip. No-op when
    /// `path` lives outside any mounted library.
    ///
    /// A failing listing must not blank the caches. This runs right
    /// after a primitive save, so the three caches it touches are
    /// known-good; overwriting them with empty vecs made every symbol,
    /// footprint and sim vanish from the library's picker and read as
    /// "my save didn't work" while the file was fine on disk.
    /// [`OpenLibrary::reload_primitives`] keeps each previous cache on
    /// error and reports the failure to the Messages panel at `warn`.
    fn refresh_primitive_cache_for(&mut self, path: &std::path::Path) {
        // Same `root_dir()` ancestor walk as `library_adapter_for` —
        // `lib.root` is the `.snxlib` file, the on-disk children sit
        // under its parent dir.
        let library_id = match self
            .library
            .open_libraries
            .iter()
            .find(|lib| lib.root_dir().map(|d| path.starts_with(d)).unwrap_or(false))
        {
            Some(lib) => lib.library_id,
            None => return,
        };
        // `set` and `open_libraries` are disjoint fields, so the
        // adapter borrow and the entry's `&mut` coexist.
        let Some(adapter) = self.library.set.get(library_id) else {
            return;
        };
        if let Some(lib) = self
            .library
            .open_libraries
            .iter_mut()
            .find(|lib| lib.library_id == library_id)
        {
            lib.reload_primitives(adapter);
        }
        // A saved primitive may be the symbol an open sheet embeds (#912).
        self.library.drift.check_pending = true;
    }

    /// Refresh the previews that already resolved a primitive from the
    /// library `path` lives in, so a Component Preview tab draws the
    /// bytes just saved instead of the copy it resolved earlier.
    fn reload_primitive_in_library_set(&mut self, path: &std::path::Path) {
        self.library.refresh_loaded_previews(path);
    }
}
