//! Primitive-editor handlers — opening `.snxsym` / `.snxfpt` document
//! tabs, routing symbol / footprint edit events, saving primitive
//! tabs, and the associated canvas-cache / external-change plumbing.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;

impl Signex {
    /// Open a `.snxsym` or `.snxfpt` as a main-window document tab.
    /// Reads the file from disk, builds the matching editor state,
    /// and pushes a `TabKind::SymbolEditor(path)` /
    /// `FootprintEditor(path)` tab into `DocumentState.tabs`.
    ///
    /// Activates an existing tab when the same path is already open
    /// instead of duplicating; surfaces parse / IO failures via
    /// `tracing::warn` (and silently bails — leaving the tab bar
    /// untouched).
    pub(crate) fn handle_open_primitive(&mut self, path: std::path::PathBuf) -> Task<Message> {
        // Already open? Just activate the existing tab.
        if let Some(idx) = self.document_state.tabs.iter().position(|t| t.path == path) {
            if idx != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = idx;
                self.sync_active_tab();
            }
            return Task::none();
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
                let bytes = match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            error = %e,
                            "open primitive: read .snxsym failed",
                        );
                        return Task::none();
                    }
                };
                // v0.18.4 — auto-detect TOML vs legacy JSON.
                let file = match signex_library::SymbolFile::from_bytes(&bytes) {
                    Ok(f) if !f.symbols.is_empty() => f,
                    Ok(_) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            "open primitive: .snxsym contains zero symbols",
                        );
                        return Task::none();
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            error = %e,
                            "open primitive: parse .snxsym failed",
                        );
                        return Task::none();
                    }
                };

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

                let state = crate::app::SymbolEditorState::new(path.clone(), file);
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
                    kind: crate::app::TabKind::SymbolEditor(path),
                });
                self.document_state.active_tab = self.document_state.tabs.len() - 1;
                // Standalone primitive tabs don't drive `active_path`
                // — clear so the canvas doesn't render a stale schematic.
                self.document_state.active_path = None;
                self.refresh_panel_ctx();
                Task::none()
            }
            "snxfpt" => {
                // v0.13.0 — footprint editor gated off for release.
                // A `.snxfpt` opened from the tree / file dialog must
                // not push an editable FootprintEditor tab. Read-only
                // preview + Pick-Footprint binding of existing files
                // stay available elsewhere. Flip
                // `feature_flags::FOOTPRINT_EDITOR_ENABLED` to re-enable.
                if !crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
                    tracing::info!(
                        target: "signex::library",
                        path = %path.display(),
                        "open primitive: footprint editor disabled (v0.13.0) — ignoring .snxfpt open",
                    );
                    return Task::none();
                }
                let bytes = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            error = %e,
                            "open primitive: read .snxfpt failed",
                        );
                        return Task::none();
                    }
                };
                // v0.18.4 — parse TOML+TSV envelope and use the first
                // footprint as the editor primitive. Multi-footprint
                // containers are not yet exposed in the editor UI.
                let file = match signex_library::FootprintFile::from_toml_str(&bytes) {
                    Ok(f) if !f.footprints.is_empty() => f,
                    Ok(_) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            "open primitive: .snxfpt contains zero footprints",
                        );
                        return Task::none();
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            error = %e,
                            "open primitive: parse .snxfpt failed",
                        );
                        return Task::none();
                    }
                };
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
                let state = crate::app::FootprintEditorState::new(path.clone(), file)
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
                    kind: crate::app::TabKind::FootprintEditor(path),
                });
                self.document_state.active_tab = self.document_state.tabs.len() - 1;
                self.document_state.active_path = None;
                self.refresh_panel_ctx();
                Task::none()
            }
            other => {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    ext = %other,
                    "open primitive: unsupported extension",
                );
                Task::none()
            }
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
                    return crate::app::handlers::document_files::spawn_save_as_for_new_primitive(
                        path,
                    );
                }
                self.save_primitive_tab_at(&path);
                Task::none()
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
            if is_capture_preset {
                self.interaction_state.footprint_filter_presets =
                    crate::fonts::read_footprint_filter_presets();
            }
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

    /// Write the primitive at `path` back to disk as JSON, commit
    /// through the matching adapter (when the file lives under a
    /// mounted `.snxlib/`), mark the tab clean, and ask the
    /// `LibrarySet` to reload its cached copy so any open Component
    /// Preview tabs see the new bytes.
    pub(crate) fn save_primitive_tab_at(&mut self, path: &std::path::Path) {
        // Symbol path — write the full multi-symbol container back to
        // disk so other symbols in the same file are preserved.
        if let Some(editor) = self.document_state.symbol_editors.get_mut(path) {
            // Refresh the active symbol's + the file's updated
            // timestamps so downstream consumers can detect the rewrite.
            let now = chrono::Utc::now();
            editor.primitive_mut().updated = now;
            editor.file.updated = now;
            // v0.18.4 — emit TOML envelope (mirror of v0.18.2 .snxfpt).
            let toml_text = match editor.file.to_toml_string() {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        error = %e,
                        "save primitive: serialize symbol file failed",
                    );
                    return;
                }
            };
            if let Err(e) = atomic_write(path, toml_text.as_bytes()) {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "save primitive: write .snxsym failed",
                );
                return;
            }
            // Capture the symbol name for the commit message before
            // dropping the editor borrow.
            let sym_name = editor.primitive().name.clone();
            editor.dirty = false;
            // Clear the project-scoped dirty marker if any callers
            // had set it.
            self.document_state.dirty_paths.remove(path);
            // Clear the matching tab's dirty flag too.
            if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
                tab.dirty = false;
            }
            // Commit through the matching adapter so the edit lands
            // in git history. No-op when the file lives outside any
            // mounted library (lone-file edit) or when the adapter
            // has no version control (database backend).
            self.commit_external_change_for(path, &format!("save symbol {sym_name}"));
            // v0.22 Phase 8.4 extension — also commit into the
            // owning project's git repo (if `enable_git` is on for
            // that project). When both library- and project-scope VC
            // are enabled the file picks up two parallel commit
            // histories — library tracks symbol-only churn, project
            // tracks the full project snapshot.
            self.commit_save_to_project_git(path, &format!("Save symbol {sym_name}"));
            // Refresh the matching library's primitive cache so the
            // picker modal picks up the new symbol immediately.
            self.refresh_primitive_cache_for(path);
            // Best-effort LibrarySet reload so Component Preview
            // tabs that already cached the primitive see the new bytes.
            self.reload_primitive_in_library_set(path);
            // v0.14.2 — refresh panel ctx so the project-tree red
            // dirty dot drops on the row (same F10 fix as the
            // footprint branch below + the schematic save handler).
            self.refresh_panel_ctx();
            return;
        }

        // Footprint path.
        if let Some(editor) = self.document_state.footprint_editors.get_mut(path) {
            // Sync the canvas-mirrored pad list back into the
            // primitive before serialising — `state.pads` is
            // authoritative on the editor side; without this, in-
            // editor pad edits wouldn't persist.
            let now = chrono::Utc::now();
            {
                let (state, primitive) = editor.parts_mut();
                crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(
                    state, primitive,
                );
            }
            editor.primitive_mut().updated = now;
            editor.file.updated = now;
            // v0.18.6 — emit the editor's persisted FootprintFile
            // directly. `file_uuid` and any multi-footprint siblings
            // are preserved across saves (mirror of SymbolEditorState).
            let toml_text = match editor.file.to_toml_string() {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        error = %e,
                        "save primitive: serialize footprint failed",
                    );
                    return;
                }
            };
            if let Err(e) = atomic_write(path, toml_text.as_bytes()) {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "save primitive: write .snxfpt failed",
                );
                return;
            }
            let fp_name = editor.primitive().name.clone();
            editor.dirty = false;
            self.document_state.dirty_paths.remove(path);
            if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
                tab.dirty = false;
            }
            self.commit_external_change_for(path, &format!("save footprint {fp_name}"));
            // v0.22 Phase 8.4 extension — same as the symbol branch:
            // also commit into the owning project's git repo when
            // its `enable_git` is on. Library + project repos can
            // run in parallel; nested `.snxlib/.git/` is opaque to
            // the project repo at the same path.
            self.commit_save_to_project_git(path, &format!("Save footprint {fp_name}"));
            self.refresh_primitive_cache_for(path);
            self.reload_primitive_in_library_set(path);
            // v0.14.2 — same F10 pattern as the schematic save: the
            // project-tree red dirty dot reads cached
            // `panel_ctx.projects[*].sheets[*].is_dirty`, which only
            // refreshes inside `refresh_panel_ctx`. Without this
            // call the dot lingers on the row even though
            // `dirty_paths` no longer contains the path.
            self.refresh_panel_ctx();
        }
    }

    /// Find the open library whose root contains `path`, then ask its
    /// adapter to stage + commit. Best-effort: silently returns when
    /// no mounted library covers `path` (lone-file edit) or when the
    /// commit itself fails (warning is emitted via tracing). Never
    /// blocks the user — the file write already succeeded.
    fn commit_external_change_for(&self, path: &std::path::Path, message: &str) {
        // Find the open library whose working dir is an ancestor of
        // `path`. `lib.root` is the `.snxlib` *file* path now, so we
        // walk against its parent directory (where `symbols/` and
        // `footprints/` actually live).
        let lib = self
            .library
            .open_libraries
            .iter()
            .find(|lib| lib.root_dir().map(|d| path.starts_with(d)).unwrap_or(false));
        let Some(lib) = lib else {
            return;
        };
        let Some(adapter) = self.library.set.get(lib.library_id) else {
            return;
        };
        if let Err(e) = adapter.commit_external_change(path, message) {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "save primitive: commit_external_change failed (file written; commit deferred)",
            );
        }
    }

    /// Refresh the matching library's per-kind primitive cache so the
    /// picker modal sees the just-saved primitive without waiting
    /// for the next full `refresh_components` round-trip. No-op when
    /// `path` lives outside any mounted library.
    fn refresh_primitive_cache_for(&mut self, path: &std::path::Path) {
        // Same `root_dir()` ancestor walk as
        // `commit_external_change_for` — `lib.root` is the `.snxlib`
        // file, the on-disk children sit under its parent dir.
        let library_id = match self
            .library
            .open_libraries
            .iter()
            .find(|lib| lib.root_dir().map(|d| path.starts_with(d)).unwrap_or(false))
        {
            Some(lib) => lib.library_id,
            None => return,
        };
        // Two-step borrow dance: snapshot the listings through the
        // mounted adapter, then move them onto the OpenLibrary entry.
        let (symbols, footprints, sims) = match self.library.set.get(library_id) {
            Some(adapter) => (
                adapter.list_symbols().unwrap_or_default(),
                adapter.list_footprints().unwrap_or_default(),
                adapter.list_sims().unwrap_or_default(),
            ),
            None => return,
        };
        if let Some(lib) = self
            .library
            .open_libraries
            .iter_mut()
            .find(|lib| lib.library_id == library_id)
        {
            lib.cached_symbols = symbols;
            lib.cached_footprints = footprints;
            lib.cached_sims = sims;
        }
    }

    /// Walk the open libraries to find one whose root contains
    /// `path` (e.g. `…/mylib.snxlib/symbols/foo.snxsym` lives under
    /// `…/mylib.snxlib/`), and ask the matching adapter to reload
    /// the primitive UUID encoded in the file. The adapter's
    /// `reload_primitive` (where supported) repopulates its in-memory
    /// cache so any Component Preview tabs that resolve through
    /// `LibrarySet` see the new bytes on the next render.
    ///
    /// Best-effort — returns silently when the path isn't under a
    /// mounted library or when the adapter has no reload hook.
    fn reload_primitive_in_library_set(&mut self, _path: &std::path::Path) {
        // Stubbed pending the corresponding `LibrarySet::reload_primitive`
        // helper. The standalone editor tab already holds the
        // authoritative copy of the primitive in memory and on-disk
        // round-trips happen here; Component Preview tabs pull
        // through `LibrarySet::resolve_*` on the next view, so the
        // only hole this leaves is a Preview tab that has already
        // resolved + cached its primitive in editor state.
    }
}
