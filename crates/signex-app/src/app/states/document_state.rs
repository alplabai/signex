pub struct DocumentState {
    pub dock: crate::dock::DockArea,
    pub tabs: Vec<super::super::TabInfo>,
    pub active_tab: usize,
    /// All live schematic engines keyed by their on-disk path. Every
    /// open schematic tab — whether it's the active one in the main
    /// window or parked in an undocked-tab window — has a live entry
    /// here. `active_path` names which entry `active_engine()` resolves
    /// to; undocked windows look up their own entry via
    /// `engine_for_window`. Save-as rekeys an entry via
    /// `rekey_engine(old, new)`.
    pub engines: std::collections::HashMap<std::path::PathBuf, signex_engine::Engine>,
    /// The path of the schematic the main window is currently editing.
    /// `active_engine()` reads `engines.get(active_path)`. `None` means
    /// no schematic tab is active (e.g. a PCB tab is active, or nothing
    /// is open).
    pub active_path: Option<std::path::PathBuf>,
    /// Every loaded project in the workspace. Order = load order. First
    /// project becomes active on load; subsequent opens append.
    pub projects: Vec<super::LoadedProject>,
    /// Which project is "active" for handlers that operate on the workspace
    /// at large (ERC / annotate / export / save-all). Currently tracks the
    /// most-recently-loaded project plus whichever project contains the
    /// active tab; single source of truth for "where am I focused".
    pub active_project: Option<super::ProjectId>,
    /// Files with unsaved edits, keyed by absolute path. Tracks the
    /// "Altium-style" project-scoped dirty state — a file stays in this
    /// set after its tab is closed (the engine in `engines` keeps the
    /// edited document) and clears when the file is saved or the
    /// project's edits are explicitly discarded. Drives the red dot on
    /// the Projects-panel tree row independently of `tab.dirty`, which
    /// would otherwise lose the signal the moment the tab is closed.
    pub dirty_paths: std::collections::HashSet<std::path::PathBuf>,
    /// Monotonic counter used to mint `ProjectId` on load. Never reused —
    /// closing a project does not free its id (tabs may hold stale refs,
    /// which we detect by resolving through `projects` and treating None
    /// as "no longer loaded").
    pub next_project_id: u32,
    pub panel_ctx: crate::panels::PanelContext,
    /// Cache of `LibSymbol` records indexed by lib_id. Populated by
    /// the v0.10.x `.snxlib` library plumbing; kept here so the
    /// canvas-side place-component flow can resolve a symbol by id
    /// independently of which panel populated it. Was previously
    /// also fed by the legacy symbol-library scanner that v0.10.0
    /// removed (Apache-clean residual polish).
    #[allow(dead_code)]
    pub loaded_lib: std::collections::HashMap<String, signex_types::schematic::LibSymbol>,
    /// Print-preview overlay state. `Some` while the preview dialog is
    /// open. Doubles as the unified PDF Export modal — `File -> Export
    /// PDF` and `File -> Print Preview` both populate this field.
    pub preview: Option<super::PreviewState>,
    /// Pending PDF options stashed from the unified preview modal
    /// while the file picker is running. Used by
    /// `handle_export_pdf_finished` to apply user-selected options
    /// instead of defaults. Cleared after export.
    pub pending_pdf_options: Option<signex_output::PdfOptions>,
    /// Companion to `pending_pdf_options` — sheet paths to include in
    /// the export, copied from the preview's file picker. Empty set
    /// (after a Clear) means "no files chosen" and the export is
    /// rejected with a user-visible error. `None` means the preview
    /// path was bypassed (legacy direct-export caller); the export
    /// then includes every sheet by default.
    pub pending_pdf_files: Option<std::collections::HashSet<std::path::PathBuf>>,
    /// Pending BOM options stashed from the BOM preview modal while
    /// the file picker is running. Without this, the user's column /
    /// grouping / variant / include-DNP picks in the preview would
    /// be dropped on export and the actual file would be a default
    /// 6-column Grouped Base BOM. Cleared after export.
    pub pending_bom_options: Option<signex_output::BomOptions>,
    /// User-visible export error. `Some(msg)` while the error modal is shown.
    /// Populated by ExportPdfFinished/ExportNetlistFinished when the export
    /// itself (not the file dialog) fails. Cleared by DismissExportError.
    pub export_error: Option<String>,
    /// BOM preview state. `Some` while the BOM Export modal is open.
    /// Mirrors the Print Preview pattern: the user adjusts grouping /
    /// include flags / format / variant in the modal and clicks
    /// Export to drive `rfd::AsyncFileDialog` with the chosen options.
    pub bom_preview: Option<super::BomPreviewState>,
}

impl DocumentState {
    /// Mint a fresh `ProjectId` and bump the counter. Never reuses ids.
    pub fn mint_project_id(&mut self) -> super::ProjectId {
        let id = super::ProjectId(self.next_project_id);
        self.next_project_id = self.next_project_id.wrapping_add(1);
        id
    }

    pub fn project_by_id(&self, id: super::ProjectId) -> Option<&super::LoadedProject> {
        self.projects.iter().find(|p| p.id == id)
    }

    pub fn project_by_id_mut(&mut self, id: super::ProjectId) -> Option<&mut super::LoadedProject> {
        self.projects.iter_mut().find(|p| p.id == id)
    }

    /// Resolve the project that contains a file at this path. Used for
    /// per-tab project scoping (tabs store a path, we resolve to the
    /// project that parented them at load time).
    pub fn project_for_path(&self, path: &std::path::Path) -> Option<&super::LoadedProject> {
        let dir = path.parent()?;
        self.projects.iter().find(|p| p.path.parent() == Some(dir))
    }

    /// Convenience: currently-active project. Returns `None` when the
    /// workspace is empty or no project has been made active yet.
    pub fn active_loaded_project(&self) -> Option<&super::LoadedProject> {
        self.active_project.and_then(|id| self.project_by_id(id))
    }

    pub fn active_engine(&self) -> Option<&signex_engine::Engine> {
        self.engines.get(self.active_path.as_ref()?)
    }

    pub fn active_engine_mut(&mut self) -> Option<&mut signex_engine::Engine> {
        let path = self.active_path.as_ref()?.clone();
        self.engines.get_mut(&path)
    }

    /// Drop the engine for the active path. Used when closing the
    /// active tab — the tab's engine is gone, and `active_path` follows
    /// to whichever tab becomes active next.
    pub fn clear_active_engine(&mut self) {
        if let Some(path) = self.active_path.as_ref().cloned() {
            self.engines.remove(&path);
        }
        self.active_path = None;
    }

    pub fn has_active_engine(&self) -> bool {
        self.active_engine().is_some()
    }

    /// Per-window engine lookup. Main window -> the active tab's engine
    /// (same as `active_engine`). Undocked tab windows -> the engine for
    /// the path the window was opened on. All schematic engines live in
    /// `self.engines`, so every window resolves with a single HashMap
    /// lookup.
    pub fn engine_for_window(
        &self,
        window_id: iced::window::Id,
        ui: &super::UiState,
    ) -> Option<&signex_engine::Engine> {
        let target_path = if ui.main_window_id == Some(window_id) {
            self.active_path.as_ref()?
        } else {
            match ui.windows.get(&window_id)? {
                super::WindowKind::UndockedTab { path, .. } => path,
                _ => return None,
            }
        };
        self.engines.get(target_path)
    }
}
