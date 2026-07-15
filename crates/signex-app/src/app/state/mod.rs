use std::path::PathBuf;

use signex_types::project::ProjectData;

use crate::dock::DockArea;

use super::TabInfo;

mod interaction;
mod ui;

pub use interaction::InteractionState;
pub use ui::UiState;

pub struct Signex {
    pub ui_state: UiState,
    pub document_state: DocumentState,
    pub interaction_state: InteractionState,
    /// v0.9 Library subsystem state. Borrowed independently of
    /// `document_state` so the library dispatcher can mutate it
    /// without colliding with schematic / PCB engine borrows.
    pub library: crate::library::LibraryState,
}


/// Chord-recorder overlay state for the Preferences ▸ Keyboard
/// Shortcuts pane. Holds the binding being edited plus the strokes
/// captured so far. The keyboard subscription feeds raw key events
/// here (as `PrefMsg::KeymapRecorderKeyPressed`) while this is `Some`,
/// so recording never triggers a live command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapRecorderState {
    pub command: crate::keymap::AppCommandId,
    pub command_label: String,
    pub context: crate::keymap::ShortcutContext,
    pub original_trigger: String,
    pub strokes: Vec<crate::keymap::KeyStroke>,
    pub modifiers: crate::keymap::Modifiers,
    pub recording: bool,
}

impl KeymapRecorderState {
    /// Longest chord the recorder will hold before wrapping back to a
    /// single stroke (Altium-style two/three-key gestures fit inside).
    pub const MAX_STROKES: usize = 3;

    pub fn new(
        command: crate::keymap::AppCommandId,
        command_label: String,
        context: crate::keymap::ShortcutContext,
        trigger: String,
    ) -> Self {
        // Seed the capture buffer with the binding's current key
        // sequence so the user sees what they are replacing. Pointer
        // gestures aren't keyboard-recordable, so they start empty.
        let strokes = crate::keymap::ShortcutTrigger::parse(&trigger)
            .ok()
            .and_then(|trigger| match trigger {
                crate::keymap::ShortcutTrigger::KeySequence(strokes) => Some(strokes),
                crate::keymap::ShortcutTrigger::PointerGesture(_) => None,
            })
            .unwrap_or_default();
        Self {
            command,
            command_label,
            context,
            original_trigger: trigger,
            strokes,
            modifiers: crate::keymap::Modifiers::default(),
            recording: true,
        }
    }

    pub fn trigger_text(&self) -> String {
        crate::keymap::ShortcutTrigger::KeySequence(self.strokes.clone()).display_text()
    }
}

/// Role of a non-main window opened by Signex. Phase 2 adds detached
/// modals; Phase 3 adds `UndockedTab(tab_index)` so a schematic sheet
/// can live in its own OS window.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum WindowKind {
    DetachedModal(ModalId),
    /// Undocked document tab. Stores the tab's file path (unique per
    /// open tab in Signex) so the mapping survives tab reordering or
    /// unrelated tabs closing. The `title` copy is used as the OS
    /// window title without re-reading tabs.
    UndockedTab {
        path: std::path::PathBuf,
        title: String,
    },
    /// Detached dock panel. Opened automatically when the user drags a
    /// floating panel past the main window edge. Closing the OS window
    /// reattaches the panel to its last dock region.
    DetachedPanel(crate::panels::PanelKind),
    /// v0.9-refactor-2 Component Preview — one window per open row.
    /// The preview's full state lives in `Signex::library.editors`
    /// keyed by `EditorAddress(library_path, table, row_id)`.
    ComponentEditor {
        library_path: std::path::PathBuf,
        table: String,
        row_id: signex_library::RowId,
    },
    /// Dedicated Tools -> PCB Trace Calculator utility window.
    PcbTraceCalculator,
}

/// Kind of z-order picker currently armed. Drives the first-click
/// resolve in `handle_canvas_left_click`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReorderPicker {
    /// Move selection to render just above the clicked reference.
    Above,
    /// Move selection to render just below the clicked reference.
    Below,
}

/// Custom net-colour picker state (Active Bar → Net Color → Custom).
#[derive(Debug, Clone)]
pub struct NetColorCustomState {
    pub show: bool,
    pub draft: iced::Color,
}

impl Default for NetColorCustomState {
    fn default() -> Self {
        Self {
            show: false,
            draft: iced::Color::from_rgb(0.40, 0.40, 0.93),
        }
    }
}

/// Transient state for the Altium-style Move Selection dialog.
/// Deltas are stored as strings so mid-edit partial values (`-`, `2.`)
/// don't panic through number parsing; the Apply handler parses them.
#[derive(Debug, Clone, Default)]
pub struct MoveSelectionState {
    pub open: bool,
    pub dx: String,
    pub dy: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum ModalId {
    AnnotateDialog,
    AnnotateResetConfirm,
    ErcDialog,
    /// Altium-style Move-Selection dialog — numeric ΔX / ΔY inputs.
    MoveSelection,
    /// F5 net-color palette.
    NetColorPalette,
    /// Parameter manager — bulk parameter editor.
    ParameterManager,
    // Reserved for future draggable modals — wired in when each dialog's
    // header gets a drag hook.
    Preferences,
    FindReplace,
    /// Rename-sheet dialog (Projects-panel leaf → Rename...).
    RenameDialog,
    /// Remove-from-project dialog (Projects-panel leaf → Remove from Project).
    RemoveDialog,
    /// Print Preview / Export PDF unified modal (File → Print Preview, File → Export PDF).
    PrintPreview,
    /// BOM Export preview modal (File → Export → Bill of Materials…).
    BomPreview,
    /// Project Options metadata modal (Projects-panel root → Project Options...).
    ProjectOptions,
    /// Enable Version Control confirm modal (Projects-panel root →
    /// Enable Version Control...).
    EnableVersionControl,
    /// v0.18.11 — Cartesian Grid Editor modal (Ctrl+G in a
    /// footprint editor).
    GridProperties,
    /// v0.18.14.1 — Custom Selection Filter modal (8-row checkbox
    /// table; opens from the Properties `Custom…` button).
    SelectionFilterCustom,
}

/// Order in which symbols are visited during Annotate. Mirrors Altium's
/// "Order of Processing" drop-down (four variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotateOrder {
    /// Top-to-bottom within each column, left-to-right across columns.
    UpThenAcross,
    /// Bottom-to-top within each column, left-to-right across columns.
    DownThenAcross,
    /// Left-to-right within each row, top-to-bottom across rows.
    AcrossThenDown,
    /// Left-to-right within each row, bottom-to-top across rows.
    AcrossThenUp,
}

/// Opaque identifier for a loaded project in the workspace. Assigned by
/// `DocumentState::next_project_id` on load and never reused, so stale
/// references (e.g. a tab pointing at a closed project) resolve to `None`
/// via `DocumentState::project_by_id` instead of silently aliasing another
/// project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProjectId(u32);

impl ProjectId {}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "proj:{}", self.0)
    }
}

/// One loaded project in the multi-project workspace. `path` is the
/// canonical identity (`.standard_pro` / `.snxprj` location on disk); `data`
/// is the parsed project contents. Multiple projects with different
/// `path`s coexist in `DocumentState.projects`; two identical `path`s
/// at once is a loader bug (existing `open_project_file` de-dupes).
#[derive(Debug, Clone)]
pub struct LoadedProject {
    pub id: ProjectId,
    pub path: PathBuf,
    pub data: ProjectData,
    /// Libraries the user has authored via the New Library flow but not
    /// yet committed to disk. The Library Options modal's Create button
    /// only registers an entry here + flips the project dirty bit;
    /// `commands::materialize_pending_library` runs at project-save time
    /// to actually write the `.snxlib`. Closes
    /// `feedback_no_disk_writes_without_user_save.md`'s "wait for
    /// explicit user save" invariant. Keyed by a temporary handle that
    /// becomes the eventual `library_id` once materialised.
    #[allow(clippy::implicit_hasher)]
    pub pending_libraries:
        std::collections::HashMap<uuid::Uuid, crate::library::commands::PendingLibrarySpec>,
}

pub struct DocumentState {
    pub dock: DockArea,
    pub tabs: Vec<TabInfo>,
    pub active_tab: usize,
    /// All live schematic engines keyed by their on-disk path. Every
    /// open schematic tab — whether it's the active one in the main
    /// window or parked in an undocked-tab window — has a live entry
    /// here. `active_path` names which entry `active_engine()` resolves
    /// to; undocked windows look up their own entry via
    /// `engine_for_window`. Save-as rekeys an entry via
    /// `rekey_engine(old, new)`.
    pub engines: std::collections::HashMap<PathBuf, signex_engine::Engine>,
    /// Per-tab state for open `.snxsym` document tabs. Keyed by the
    /// file path stored on `TabInfo.path` for matching
    /// `TabKind::SymbolEditor(path)` tabs. Insert on
    /// `LibraryMessage::OpenPrimitiveEditor`; drop in
    /// `close_tab_at_index` alongside the engine cleanup.
    pub symbol_editors: std::collections::HashMap<PathBuf, super::SymbolEditorState>,
    /// Per-tab state for open `.snxfpt` document tabs. Keyed the same
    /// way as `symbol_editors`.
    pub footprint_editors: std::collections::HashMap<PathBuf, super::FootprintEditorState>,
    /// v0.26-E — process-local clipboard for footprint-editor pad
    /// Cut / Copy / Paste. Survives tab switches so a pad copied from
    /// one footprint can be pasted into another. `None` until the
    /// first Cut / Copy. Replaced wholesale on every copy. Keyed by
    /// no path on purpose — Altium parity is single-slot, last-write-
    /// wins.
    pub pad_clipboard: Option<crate::library::editor::footprint::state::EditorPad>,
    /// The path of the schematic the main window is currently editing.
    /// `active_engine()` reads `engines.get(active_path)`. `None` means
    /// no schematic tab is active (e.g. a PCB tab is active, or nothing
    /// is open).
    pub active_path: Option<PathBuf>,
    /// Every loaded project in the workspace. Order = load order. First
    /// project becomes active on load; subsequent opens append.
    pub projects: Vec<LoadedProject>,
    /// Which project is "active" for handlers that operate on the workspace
    /// at large (ERC / annotate / export / save-all). Currently tracks the
    /// most-recently-loaded project plus whichever project contains the
    /// active tab; single source of truth for "where am I focused".
    pub active_project: Option<ProjectId>,
    /// Files with unsaved edits, keyed by absolute path. Tracks the
    /// "Altium-style" project-scoped dirty state — a file stays in this
    /// set after its tab is closed (the engine in `engines` keeps the
    /// edited document) and clears when the file is saved or the
    /// project's edits are explicitly discarded. Drives the red dot on
    /// the Projects-panel tree row independently of `tab.dirty`, which
    /// would otherwise lose the signal the moment the tab is closed.
    pub dirty_paths: std::collections::HashSet<PathBuf>,
    /// Monotonic counter used to mint `ProjectId` on load. Never reused —
    /// closing a project does not free its id (tabs may hold stale refs,
    /// which we detect by resolving through `projects` and treating None
    /// as "no longer loaded").
    pub next_project_id: u32,
    pub panel_ctx: crate::panels::PanelContext,
    pub standard_lib_dir: Option<PathBuf>,
    pub loaded_lib: std::collections::HashMap<String, signex_types::schematic::LibSymbol>,
    /// Print-preview overlay state. `Some` while the preview dialog is
    /// open. Doubles as the unified PDF Export modal — `File → Export
    /// PDF` and `File → Print Preview` both populate this field.
    pub preview: Option<PreviewState>,
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
    pub pending_pdf_files: Option<std::collections::HashSet<PathBuf>>,
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
    pub bom_preview: Option<BomPreviewState>,
    /// Right-dock History panel state — current generation counter,
    /// last loaded entries, the path the load was issued for. Driven
    /// by `Message::HistoryLoaded` and re-targeted on tab switch.
    pub history: crate::panels::history::HistoryPanelState,
    /// v0.23 — Async git commit work queue. Save handlers push tuples
    /// here; `finish_update` drains them into `Task::perform` calls
    /// that run the actual commit on a tokio `spawn_blocking`. Each
    /// completion routes through `Message::Project(ProjectMsg::GitCommitDone)`.
    pub pending_git_commits: Vec<PendingGitCommit>,
    /// v0.23 — Set of `(project_root, rel_path)` pairs whose commits
    /// are currently queued or in flight. Drives the status bar's
    /// "Saving…" pill — when non-empty the user sees an indicator.
    /// An entry lands here as soon as
    /// [`Signex::commit_save_to_project_git`] enqueues the work and
    /// clears on `Message::Project(ProjectMsg::GitCommitDone)`.
    pub inflight_git_commits: std::collections::HashSet<(PathBuf, PathBuf)>,
}

/// v0.23 — One queued commit for the async git pipeline. Stays
/// resident in `DocumentState.pending_git_commits` until
/// `finish_update` drains it into a `Task::perform`.
#[derive(Debug, Clone)]
pub struct PendingGitCommit {
    pub project_root: PathBuf,
    pub rel_path: PathBuf,
    pub message: String,
}

/// Which sidebar tab is currently shown inside the BOM preview's
/// Properties panel — Altium-style General / Columns split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BomSidebarTab {
    General,
    Columns,
}

/// Live BOM preview state — the rolled-up table for the active project
/// plus the user-editable options that drive the next rollup. Re-rolled
/// whenever an option toggle fires.
pub struct BomPreviewState {
    pub options: signex_output::BomOptions,
    pub table: signex_output::BomTable,
    /// Available variants for the active project. Reserved for the
    /// variant picker dropdown — empty when no variants are defined.
    /// Currently only seeded; the picker UI lands in v0.8.1.
    #[allow(dead_code)]
    pub variants: Vec<String>,
    /// Active sort spec — `(column index in options.columns, ascending)`.
    /// `None` = render rollup order (the default emit order from
    /// `bom::rollup`). Click a header cell to set; click the same one
    /// again to flip direction.
    pub sort: Option<(usize, bool)>,
    /// In-flight column drag — `Some(from_idx)` while the user is
    /// holding the mouse down on a header cell. The header only
    /// renders the drag highlight once the cursor has moved past
    /// `column_drag_press_x` by at least the threshold (see
    /// `view`); a quick press-and-release counts as a click and
    /// the cell never lights up.
    pub column_drag: Option<usize>,
    /// Cursor x at the moment the column drag was armed. Compared
    /// against `last_mouse_pos.0` in the view to decide whether
    /// the press has graduated into an actual drag.
    pub column_drag_press_x: Option<f32>,
    /// Index of the column header currently under the cursor.
    /// Tracked via on_enter/on_exit on each header cell so the
    /// release handler (which fires on the press-source widget,
    /// not the cursor target) can resolve where the drop landed.
    pub column_hover: Option<usize>,
    /// Per-column width overrides keyed by index in
    /// `options.columns`. Populated as the user drags a header
    /// resize handle; consulted by the width helper before falling
    /// back to the per-`BomColumn` default. Cleared on close.
    pub column_widths: std::collections::HashMap<usize, f32>,
    /// In-flight column-resize state — `Some` while the user is
    /// dragging a header's right-edge handle. `start_x` is the
    /// global cursor x at press; `start_width` is the column's
    /// width at press. Width updates each mouse-move tick are
    /// computed against these baselines.
    pub column_resize: Option<ColumnResizeState>,
    /// Currently-shown tab inside the Properties sidebar.
    pub sidebar_tab: BomSidebarTab,
}

#[derive(Debug, Clone, Copy)]
pub struct ColumnResizeState {
    pub idx: usize,
    pub start_x: f32,
    pub start_width: f32,
}

/// Tabs inside the unified Export PDF modal — Preview is the
/// rasterised page view, Settings is the multi-section configuration
/// panel (file picker, additional settings, structure settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfPreviewTab {
    Preview,
    Settings,
}

/// Output PDF resolution preset — Altium parity. Drives the Quality
/// dropdown in the Settings tab. The export pipeline is vector-only
/// today, so DPI only affects the *preview* rasterisation: a higher
/// preset gives a sharper preview when you zoom in. The mapped DPI
/// for the export-side `PdfOptions.dpi` is the picker label (72/300/
/// 600) so future raster fallbacks (embedded images) get the user
/// intent verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfQuality {
    Draft72,
    Medium300,
    High600,
}

impl PdfQuality {
    /// DPI used to rasterise the on-screen preview. Capped well below
    /// the export label so an A4 page doesn't blow up to ~35 MB of
    /// RGBA at 600 DPI.
    pub fn preview_dpi(self) -> f64 {
        match self {
            PdfQuality::Draft72 => 72.0,
            PdfQuality::Medium300 => 144.0,
            PdfQuality::High600 => 200.0,
        }
    }

    /// DPI written to `PdfOptions.dpi` at export time. Vector content
    /// ignores this; future raster fallbacks (embedded images,
    /// rasterised symbol bodies) honour the verbatim picker label.
    pub fn export_dpi(self) -> f32 {
        match self {
            PdfQuality::Draft72 => 72.0,
            PdfQuality::Medium300 => 300.0,
            PdfQuality::High600 => 600.0,
        }
    }
}

impl std::fmt::Display for PdfQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PdfQuality::Draft72 => "Draft (72 dpi)",
            PdfQuality::Medium300 => "Medium (300 dpi)",
            PdfQuality::High600 => "High (600 dpi)",
        };
        f.write_str(s)
    }
}

/// Open-print-preview state — rasterised pages + which one is currently
/// shown full-size. Pages are produced by `signex_output::PreviewRasterizer`
/// when the user invokes File → Print Preview (Ctrl+P).
///
/// **Single source of truth.** Every option that's also on
/// `signex_output::PdfOptions` lives ONLY on `pdf_options`; the
/// dispatcher mutates that struct directly so the rasterizer and
/// exporter see one consistent view. Fields on this struct itself are
/// the leftovers — UI presentation (active tab, quality enum), the
/// rasterised pages, and pan/zoom interaction state.
pub struct PreviewState {
    pub pages: Vec<signex_output::PreviewPage>,
    pub page_handles: Vec<iced::widget::image::Handle>,
    pub selected: usize,
    pub pdf_options: signex_output::PdfOptions,
    pub specific_page_input: String,
    /// Multiplicative zoom for the preview image. 1.0 = fit-to-viewport;
    /// scroll wheel multiplies by `1.10`/`1/1.10`. Clamped to
    /// `[Self::ZOOM_MIN, Self::ZOOM_MAX]` in the handler so very fast
    /// wheel bursts can't blow the image up to gigabytes.
    pub zoom: f32,
    /// Currently-shown tab inside the Export PDF modal.
    pub active_tab: PdfPreviewTab,
    /// Pan offset in logical pixels — added to the image origin so the
    /// user can drag a zoomed-in page around the viewport. Reset to
    /// (0, 0) when zoom ≤ 1 (no pan needed) and on page change.
    pub pan: (f32, f32),
    /// In-flight pan drag — `Some((origin_pan, press_x, press_y))`
    /// while the user is holding the mouse down on the preview
    /// surface. Updated every move via the global mouse handler.
    pub panning: Option<((f32, f32), f32, f32)>,
    /// Files chosen for export from the active project's sheet list.
    /// Empty = all files (default at open). When non-empty, only the
    /// listed paths are rasterised + exported. Driven by the file
    /// picker in the Settings tab.
    pub selected_files: std::collections::HashSet<PathBuf>,
    /// Available variants for the active project — drives the variant
    /// picker dropdown options. The currently-selected value lives on
    /// `pdf_options.variant`.
    pub variants: Vec<String>,
    /// Quality preset shown in the Settings tab dropdown. Mapped to
    /// `pdf_options.dpi` at export time; the preview always rasterises
    /// at 96 DPI for speed.
    pub quality: PdfQuality,
}

impl PreviewState {
    pub const ZOOM_MIN: f32 = 0.25;
    pub const ZOOM_MAX: f32 = 6.0;
    pub const ZOOM_STEP: f32 = 1.10;
}

impl DocumentState {
    /// Mint a fresh `ProjectId` and bump the counter. Never reuses ids.
    pub fn mint_project_id(&mut self) -> ProjectId {
        let id = ProjectId(self.next_project_id);
        self.next_project_id = self.next_project_id.wrapping_add(1);
        id
    }

    pub fn project_by_id(&self, id: ProjectId) -> Option<&LoadedProject> {
        self.projects.iter().find(|p| p.id == id)
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
        let target_path = if ui.main_window_id == Some(window_id) {
            self.active_path.as_ref()?
        } else {
            match ui.windows.get(&window_id)? {
                WindowKind::UndockedTab { path, .. } => path,
                _ => return None,
            }
        };
        self.engines.get(target_path)
    }
}
