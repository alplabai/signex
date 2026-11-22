//! `DocumentState` lookups: projects, engines and the windows showing them.

use std::path::PathBuf;

use super::ui::UiState;
use super::{DocumentState, LoadedProject, WindowKind, scope};
use crate::app::core::project_id::ProjectId;

impl DocumentState {
    /// Mint a fresh `ProjectId` and bump the counter. Never reuses ids.
    pub fn mint_project_id(&mut self) -> ProjectId {
        ProjectId::mint(&mut self.next_project_id)
    }

    pub fn project_by_id(&self, id: ProjectId) -> Option<&LoadedProject> {
        self.projects.iter().find(|p| p.id == id)
    }

    pub fn project_by_id_mut(&mut self, id: ProjectId) -> Option<&mut LoadedProject> {
        self.projects.iter_mut().find(|p| p.id == id)
    }

    /// Resolve the project that contains a file at this path. Used for
    /// per-tab project scoping (tabs store a path, we resolve to the
    /// project that parented them at load time).
    pub fn project_for_path(&self, path: &std::path::Path) -> Option<&LoadedProject> {
        let dir = path.parent()?;
        self.projects.iter().find(|p| p.path.parent() == Some(dir))
    }

    /// Convenience: currently-active project. Returns `None` when the
    /// workspace is empty or no project has been made active yet.
    pub fn active_loaded_project(&self) -> Option<&LoadedProject> {
        self.active_project.and_then(|id| self.project_by_id(id))
    }

    /// Project that owns the *active document* — the scope for export, ERC,
    /// annotate and hierarchical child-sheet resolution.
    ///
    /// Deliberately not `active_loaded_project()`: `active_project` is a
    /// sticky workspace-wide pointer that survives focusing a tab with no
    /// project of its own, so scoping off it makes an export of a loose
    /// schematic emit another project's sheets (#406). Resolution rules and
    /// the hierarchy walk live in [`scope::project_owning_sheet`].
    ///
    /// - active schematic owned by a project → that project;
    /// - active schematic owned by none (loose) → `None`, i.e. operate on
    ///   this one document;
    /// - no active schematic at all (a PCB / symbol / footprint tab is
    ///   focused, or nothing is open) → the sticky pointer, which is what
    ///   "the workspace at large" means when there is no document to scope by.
    pub fn active_document_project(&self) -> Option<&LoadedProject> {
        let Some(path) = self.active_path.as_ref() else {
            return self.active_loaded_project();
        };
        self.document_project(path)
    }

    /// The designator standard of the project that owns `sheet` (#1416).
    pub fn designator_standard_for(
        &self,
        sheet: &std::path::Path,
    ) -> Option<signex_types::designator::DesignatorStandard> {
        self.document_project(sheet)
            .and_then(|p| p.data.designator_standard())
    }

    /// The resolved schematic defaults (#2026) of the project that owns
    /// `sheet`; the built-in table for a loose sheet.
    pub fn schematic_defaults_for(
        &self,
        sheet: &std::path::Path,
    ) -> signex_types::schematic::SchematicDefaults {
        self.document_project(sheet)
            .map_or(signex_types::schematic::SchematicDefaults::BUILT_IN, |p| {
                p.data.schematic_defaults.resolve()
            })
    }

    /// [`Self::schematic_defaults_for`] the active document, or the sheet an
    /// agent's edit list runs on.
    pub fn active_schematic_defaults(&self) -> signex_types::schematic::SchematicDefaults {
        self.edit_path()
            .map(|path| self.schematic_defaults_for(path))
            .unwrap_or_default()
    }

    /// A pre-placement form for the active document, starting from its
    /// project's text defaults (#2026).
    pub fn new_pre_placement(
        &self,
        tool_name: String,
        kind: crate::panels::PrePlacementKind,
        label_text: String,
        cursor_mm: (f64, f64),
    ) -> crate::panels::PrePlacementData {
        let mut data = crate::panels::PrePlacementData::new(
            tool_name,
            kind,
            label_text,
            cursor_mm.0,
            cursor_mm.1,
        );
        let defaults = self.active_schematic_defaults();
        data.font_size_pt = defaults.placed_text_pt();
        data.shape_fill = defaults.shape_fill;
        if let Some(path) = self.active_path.as_deref() {
            data.font = self.schematic_default_font_for(path);
        }
        data
    }

    /// The font a label or note placed on the active document stores: the
    /// project's default font when it names one, else `None` (#2026).
    pub fn placed_font_family(&self) -> Option<String> {
        self.edit_path()
            .and_then(|path| self.document_project(path))
            .and_then(|p| p.data.schematic_defaults.placed_font_family())
    }

    /// The font new text on `sheet` is placed in (#2026).
    pub fn schematic_default_font_for(&self, sheet: &std::path::Path) -> String {
        self.document_project(sheet).map_or_else(
            || signex_types::schematic::SCHEMATIC_DEFAULT_FONT.to_string(),
            |p| p.data.schematic_defaults.font_family().to_string(),
        )
    }
    /// Every open engine that belongs to the same project as the active
    /// document, the active one included. A loose active document yields only
    /// itself. Used where a name must be unique across the whole hierarchy
    /// (a project-wide default such as a new child sheet's `sheetN`).
    pub fn engines_in_active_document_project(&self) -> Vec<&signex_engine::Engine> {
        let Some(project) = self.active_document_project() else {
            return self.active_engine().into_iter().collect();
        };
        scope::engines_in_project(
            &self.projects,
            &self.ownership_index(),
            &self.engines,
            project.id,
        )
    }

    /// [`engines_in_active_document_project`](Self::engines_in_active_document_project)
    /// with each engine's sheet path, so a reference can be resolved against
    /// the folder of the sheet that carries it.
    pub(crate) fn engine_entries_in_active_document_project(
        &self,
    ) -> Vec<(&PathBuf, &signex_engine::Engine)> {
        let Some(project) = self.active_document_project() else {
            return self
                .active_path
                .as_ref()
                .and_then(|path| self.engines.get_key_value(path))
                .into_iter()
                .collect();
        };
        scope::engine_entries_in_project(
            &self.projects,
            &self.ownership_index(),
            &self.engines,
            project.id,
        )
    }

    /// Record that the set of open documents, or what they hold on disk,
    /// changed. See [`DocumentState::documents_epoch`].
    pub fn note_documents_changed(&mut self) {
        self.documents_epoch += 1;
    }

    /// Install `engine` for `path` and move the epoch. Use this for any engine
    /// a background read could otherwise miss. The active-session swap in
    /// `load_gateway` and the bulk `engines.retain` calls bypass it; they act
    /// only on engines that already existed, which a background read never
    /// replaces from disk.
    pub fn insert_engine(
        &mut self,
        path: PathBuf,
        engine: signex_engine::Engine,
    ) -> Option<signex_engine::Engine> {
        self.note_documents_changed();
        // A new document at the path ends what a held engine could undo.
        self.held_for_undo.remove(&path);
        self.engines.insert(path, engine)
    }

    /// Drop the engine for `path`; see [`DocumentState::insert_engine`].
    pub fn remove_engine(&mut self, path: &std::path::Path) -> Option<signex_engine::Engine> {
        self.note_documents_changed();
        // The head read before it was open may be older than what it saved.
        self.document_ids.forget_header(path);
        self.engines.remove(path)
    }

    /// A `.snxsch` was written or deleted on disk with no engine behind it
    /// (a History restore, a deleted project file). The writers reach it through
    /// `autosave_after_disk_write` or `drop_deleted_documents`; a new
    /// engine-less writer must call one of those or this, so a background read
    /// of that file can tell it is out of date.
    pub fn note_document_written_behind_engines(&mut self) {
        self.note_documents_changed();
        self.note_sheet_edit();
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
        self.note_documents_changed();
        if let Some(path) = self.active_path.as_ref().cloned() {
            self.engines.remove(&path);
        }
        self.active_path = None;
    }

    /// The engine at `path` holds what was just read from disk: its history
    /// position is the clean point. Only when `installed`: a failed install
    /// leaves the older engine at `path`, which is not known to equal the file.
    pub(crate) fn mark_installed_engine_saved(&mut self, path: &std::path::Path, installed: bool) {
        if installed && let Some(engine) = self.engines.get_mut(path) {
            engine.mark_saved();
        }
    }

    pub fn has_active_engine(&self) -> bool {
        self.active_engine().is_some()
    }

    /// Per-window engine lookup. Main window → the active tab's engine
    /// (same as `active_engine`). Undocked tab windows → the engine for
    /// the path the window was opened on. All schematic engines live in
    /// `self.engines`, so every window resolves with a single HashMap
    /// lookup.
    pub fn engine_for_window(
        &self,
        window_id: iced::window::Id,
        ui: &UiState,
    ) -> Option<&signex_engine::Engine> {
        self.engines.get(self.sheet_path_for_window(window_id, ui)?)
    }

    /// The sheet a window shows: the active one for the main window, the tab's
    /// own for an undocked one.
    pub fn sheet_path_for_window<'a>(
        &'a self,
        window_id: iced::window::Id,
        ui: &'a UiState,
    ) -> Option<&'a std::path::PathBuf> {
        if ui.main_window_id == Some(window_id) {
            return self.active_path.as_ref();
        }
        match ui.windows.get(&window_id)? {
            WindowKind::UndockedTab { path, .. } => Some(path),
            _ => None,
        }
    }
}
