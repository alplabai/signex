use std::path::PathBuf;

use signex_render::{GridStyle, LabelStyle, MultisheetStyle, PowerPortStyle};
use signex_types::coord::Unit;
use signex_types::project::ProjectData;
use signex_types::theme::ThemeId;

use crate::canvas::SchematicCanvas;
use crate::dock::DockArea;
use crate::pcb_canvas::PcbCanvas;

use super::{ContextMenuState, DragTarget, DrawMode, TabInfo, TextEditState, Tool};

pub struct Signex {
    pub ui_state: UiState,
    pub document_state: DocumentState,
    pub interaction_state: InteractionState,
}

pub struct UiState {
    pub theme_id: ThemeId,
    pub unit: Unit,
    pub grid_visible: bool,
    pub snap_enabled: bool,
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub zoom: f64,
    pub grid_size_mm: f32,
    pub visible_grid_mm: f32,
    pub snap_hotspots: bool,
    pub ui_font_name: String,
    pub canvas_font_name: String,
    pub canvas_font_size: f32,
    pub canvas_font_bold: bool,
    pub canvas_font_italic: bool,
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_height: f32,
    pub window_size: (f32, f32),
    /// OS-reported scale factor for the main window (1.0 at 100 %,
    /// 1.25 at 125 %, 2.0 at 200 %, etc.). Populated on
    /// `MainWindowOpened` and refreshed on every main-window resize,
    /// which also fires when Windows moves the window to a different
    /// monitor. Used to pick the 1×/2×/3× wordmark PNG so the brand
    /// lockup stays 1:1 with device pixels.
    pub main_window_scale: f32,
    pub panel_list_open: bool,
    pub preferences_open: bool,
    pub find_replace: crate::find_replace::FindReplaceState,
    pub preferences_nav: crate::preferences::PrefNav,
    pub preferences_draft_theme: ThemeId,
    pub preferences_draft_font: String,
    pub power_port_style: PowerPortStyle,
    pub preferences_draft_power_port_style: PowerPortStyle,
    pub label_style: LabelStyle,
    pub preferences_draft_label_style: LabelStyle,
    pub multisheet_style: MultisheetStyle,
    pub preferences_draft_multisheet_style: MultisheetStyle,
    pub grid_style: GridStyle,
    pub preferences_draft_grid_style: GridStyle,
    pub preferences_dirty: bool,
    pub custom_theme: Option<signex_types::theme::CustomThemeFile>,
    /// Rename-sheet modal state. Opened from the Projects-panel tree
    /// context menu; `None` when the modal is closed.
    pub rename_dialog: Option<crate::app::RenameDialogState>,
    /// Remove-from-project modal state (Delete / Exclude / Cancel).
    pub remove_dialog: Option<crate::app::RemoveDialogState>,
    /// "Close Project — Unsaved Edits" confirmation modal. `Some`
    /// while the user is being asked to save / discard / cancel a
    /// close request that intersects `dirty_paths`. Cleared on any
    /// of the three button choices.
    pub project_close_confirm: Option<crate::app::ProjectCloseConfirmState>,
    pub erc: ErcState,
    pub annotate: AnnotateState,
    pub net_color: NetColorState,
    /// AutoFocus mode — when true, non-selected items dim on the canvas.
    pub auto_focus: bool,
    /// Per-modal offset in window pixels from the centered position.
    /// Updated when the user drags the title bar. Persists until the app
    /// closes so reopening a dialog lands where it was last placed.
    pub modal_offsets: std::collections::HashMap<ModalId, (f32, f32)>,
    /// Active modal drag: which modal is being dragged + the last mouse
    /// position so the delta can be computed from the next DragMove event.
    pub modal_dragging: Option<(ModalId, f32, f32)>,
    /// Active tab drag: which document tab is being dragged + the last
    /// mouse position. Used by auto-detach — when the cursor crosses the
    /// main window edge the tab undocks into its own OS window.
    pub tab_dragging: Option<(usize, f32, f32)>,
    /// Move-Selection dialog state (Altium's numeric ΔX / ΔY move).
    pub move_selection: MoveSelectionState,
    /// Parameter Manager dialog state.
    pub parameter_manager_open: bool,
    /// Active "pick a reference item" mode for z-order operations
    /// (BringToFrontOf / SendToBackOf). When Some, the next canvas click
    /// resolves the reference uuid and submits the Reorder command.
    pub reorder_picker: Option<ReorderPicker>,
    /// Altium-style rubber-band selection mode. Drives how the box
    /// drag classifies hits (Inside / Outside / TouchingLine).
    pub selection_mode: signex_render::schematic::hit_test::SelectionMode,
    /// Altium-style lasso in flight. `Some(points)` means the user
    /// started a lasso — each canvas click appends a vertex; a
    /// double-click or a click on the first vertex closes the polygon
    /// and commits the selection. Escape or right-click cancels.
    pub lasso_polygon: Option<Vec<signex_types::schematic::Point>>,
    /// Id of the primary app window — set once `iced::window::open` for
    /// the main window resolves. Every `view(id)` call checks this to
    /// decide whether it's rendering the main shell or a secondary
    /// (detached modal / undocked tab) window.
    pub main_window_id: Option<iced::window::Id>,
    /// Every non-main window Signex owns, keyed by its iced id. Lets
    /// `view(id)` dispatch between the main shell, detached modals, and
    /// (later) undocked tabs. `SecondaryWindowClosed` removes entries so
    /// the detached content reattaches to the main window.
    pub windows: std::collections::HashMap<iced::window::Id, WindowKind>,
    /// Paths whose async save (v0.9.1 perf path) is currently running
    /// off the UI thread. Drives the "Saving…" pill in the status bar
    /// and is cleared on `Message::SaveFileFinished`. Failed saves
    /// stay in `save_error` for a few seconds so the operator sees
    /// what happened.
    pub saving_paths: std::collections::HashSet<std::path::PathBuf>,
    /// Last save error message and the time it was set. The status
    /// bar shows this briefly, then `tick_save_error` clears it.
    pub save_error: Option<(String, std::time::Instant)>,
}

pub struct ErcState {
    /// ERC results for the currently-visible sheet. Driven by the
    /// per-sheet cache below — switching tabs repoints this at the
    /// cached violations for that sheet, so markers and the Messages
    /// panel always match what's on the canvas.
    pub violations: Vec<signex_erc::Violation>,
    /// Per-sheet ERC violation cache, keyed by the sheet's on-disk
    /// file path. Run ERC populates this for every sheet in the
    /// project; tab switches point `violations` at the matching
    /// entry without rerunning the analysis.
    pub violations_by_path: std::collections::HashMap<std::path::PathBuf, Vec<signex_erc::Violation>>,
    /// Global cursor into the flattened ERC diagnostics list spanning all
    /// sheets in `violations_by_path`. Used by next/prev navigation.
    pub focus_global_index: Option<usize>,
    /// Per-rule severity override — if empty, the rule's default is used.
    pub severity_override: std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    /// ERC dialog open flag — opens the full severity-matrix + pin-matrix UI.
    pub dialog_open: bool,
    /// Pin-connection matrix overrides — sparse map keyed by (row, col)
    /// pin-type index. Any entry present replaces the default severity
    /// for that pair; missing entries fall back to the hard-coded
    /// baseline in `pin_matrix_view`. Persisted alongside the ERC
    /// severity map.
    pub pin_matrix_overrides: std::collections::HashMap<(u8, u8), signex_erc::Severity>,
}

pub struct AnnotateState {
    /// Annotate dialog open flag. When true, the Annotate-Schematics modal
    /// covers the canvas with its preview + confirm-apply UI.
    pub dialog_open: bool,
    /// Annotate dialog: order-of-processing choice. Controls the iteration
    /// order used to assign sequential numbers.
    pub order: AnnotateOrder,
    /// Reset-and-renumber confirmation modal. When true, the Design →
    /// Reset menu item shows a confirm before discarding every number.
    pub reset_confirm: bool,
    /// Symbols whose designator the user locked against reannotation.
    /// Exposed as per-row checkboxes in the Annotate dialog; the engine
    /// skips these uuids in `annotate_with_seed_and_locks`.
    pub locked: std::collections::HashSet<uuid::Uuid>,
}

pub struct NetColorState {
    /// Net-color overrides keyed by net-label text. Superseded by the
    /// per-wire `wire_color_overrides` map below which the Active-Bar
    /// net-colour flood populates; kept here so a future net-name
    /// palette can cross-reference it without another round-trip.
    #[allow(dead_code)]
    pub colors_by_net: std::collections::HashMap<String, signex_types::theme::Color>,
    /// F5 Net Color palette state — open flag and transient edit buffer.
    pub palette_open: bool,
    /// Net-color override armed from the Active Bar palette. When Some,
    /// the cursor turns into a paint-bucket over the canvas and the
    /// next click on a wire floods that color across every connected
    /// wire. Cleared after the click applies, or by Escape. Colors are
    /// render-time only — they do NOT write back to the .snxsch.
    pub pending_color: Option<signex_types::theme::Color>,
    /// Per-wire color overrides keyed by wire uuid. Populated by the
    /// net-color click; consulted when drawing wires. Not serialised.
    pub wire_color_overrides: std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
    /// App-level undo stack for net-color floods. Each entry is the
    /// full `wire_color_overrides` map captured before an action —
    /// popping one restores the previous state. This is separate from
    /// the engine's undo because net colours are render-only and
    /// shouldn't mix with document mutations.
    pub undo: Vec<std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>>,
    /// Custom net-color picker state. When `show = true`, a floating
    /// picker appears anchored to the Active Bar button; `draft` is
    /// the user's pending pick — committed on OK.
    pub custom: NetColorCustomState,
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

impl ProjectId {
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "proj:{}", self.0)
    }
}

/// One loaded project in the multi-project workspace. `path` is the
/// canonical identity (`.snxprj` / `.snxprj` location on disk); `data`
/// is the parsed project contents. Multiple projects with different
/// `path`s coexist in `DocumentState.projects`; two identical `path`s
/// at once is a loader bug (existing `open_project_file` de-dupes).
#[derive(Debug, Clone)]
pub struct LoadedProject {
    pub id: ProjectId,
    pub path: PathBuf,
    pub data: ProjectData,
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
    /// Cache of `LibSymbol` records indexed by lib_id. Populated by
    /// the v0.10.x `.snxlib` library plumbing; kept here so the
    /// canvas-side place-component flow can resolve a symbol by id
    /// independently of which panel populated it. Was previously
    /// also fed by the legacy symbol-library scanner that v0.10.0
    /// removed (Apache-clean residual polish).
    #[allow(dead_code)]
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

pub struct InteractionState {
    pub current_tool: Tool,
    /// The main-window schematic canvas. Every non-main window carries
    /// its own `SchematicCanvas` inside `canvases`, keyed by that
    /// window's `iced::window::Id`. The event-dispatch layer swaps a
    /// per-window canvas into this slot while handling an event so the
    /// hundreds of `active_canvas_mut()` call sites don't need to know
    /// about per-window routing.
    pub canvas: SchematicCanvas,
    /// Extra schematic canvases owned by non-main windows (undocked
    /// tabs). Populated on `Message::UndockedTabOpened`; drained on
    /// `Message::SecondaryWindowClosed`. Reads go through
    /// `canvas_for_window`; writes happen via the dispatch swap trick.
    pub canvases: std::collections::HashMap<iced::window::Id, SchematicCanvas>,
    pub pcb_canvas: PcbCanvas,
    pub dragging: Option<DragTarget>,
    pub drag_start_pos: Option<f32>,
    pub drag_start_size: f32,
    pub tab_drag_origin: Option<(f32, f32)>,
    pub undo_stack: crate::undo::UndoStack,
    pub wire_points: Vec<signex_types::schematic::Point>,
    pub wire_drawing: bool,
    /// 3-click arc placement buffer. Holds the first two clicks
    /// (start, mid); the third click commits as SchDrawing::Arc.
    pub arc_points: Vec<signex_types::schematic::Point>,
    /// Freehand polygon placement buffer. Accumulates clicks until
    /// the user presses Enter / double-clicks / right-clicks.
    pub polyline_points: Vec<signex_types::schematic::Point>,
    /// Two-click shape placement: first click sets the anchor, second
    /// click commits. Used by Tool::Line, Tool::Rectangle, Tool::Circle.
    pub shape_anchor: Option<signex_types::schematic::Point>,
    pub clipboard_wires: Vec<signex_types::schematic::Wire>,
    pub clipboard_buses: Vec<signex_types::schematic::Bus>,
    pub clipboard_labels: Vec<signex_types::schematic::Label>,
    pub clipboard_symbols: Vec<signex_types::schematic::Symbol>,
    pub clipboard_junctions: Vec<signex_types::schematic::Junction>,
    pub clipboard_no_connects: Vec<signex_types::schematic::NoConnect>,
    pub clipboard_text_notes: Vec<signex_types::schematic::TextNote>,
    pub draw_mode: DrawMode,
    pub editing_text: Option<TextEditState>,
    pub context_menu: Option<ContextMenuState>,
    /// Projects-panel tree-view right-click menu state. Separate from
    /// `context_menu` (canvas-scoped) because the two menus have no
    /// overlap in actions and the canvas menu depends on placement /
    /// selection state that does not exist in the panel context.
    pub project_tree_context_menu: Option<crate::app::ProjectTreeContextMenuState>,
    /// Document-tab right-click menu state. Anchored at the right-click
    /// coordinates inside the tab strip; carries the index of the
    /// clicked tab so per-tab actions ("Close [filename]") resolve
    /// against the correct entry. Mutually exclusive with
    /// `context_menu` and `project_tree_context_menu` — opening one
    /// dismisses the others.
    pub tab_context_menu: Option<crate::app::TabContextMenuState>,
    /// Currently-expanded submenu inside the right-click context menu
    /// (None when no submenu is shown). Always cleared when
    /// `context_menu` becomes None.
    pub context_submenu: Option<crate::app::ContextSubmenu>,
    /// `(kind, hover_started_at)` for the submenu launcher the cursor
    /// is currently hovering. The 50 ms hover-tick subscription opens
    /// the submenu once `hover_started_at + 200 ms <= Instant::now()`,
    /// matching the standard Altium / Windows menu delay.
    pub pending_submenu: Option<(crate::app::ContextSubmenu, std::time::Instant)>,
    /// Which submenu launcher row the cursor is currently over, or
    /// `None`. Paired with `submenu_panel_hovered` to decide whether
    /// the open submenu should stay visible.
    pub submenu_launcher_hovered: Option<crate::app::ContextSubmenu>,
    /// Whether the cursor is currently over the opened submenu panel.
    pub submenu_panel_hovered: bool,
    /// Timestamp of when *both* the launcher and the panel became
    /// unhovered. The 50 ms tick closes the submenu once 150 ms has
    /// elapsed, giving the user time to cross the gap between the two
    /// zones without the menu collapsing mid-traversal.
    pub submenu_unhovered_since: Option<std::time::Instant>,
    pub last_mouse_pos: (f32, f32),
    /// Most recent project-tree row click — `(path, timestamp)`. Used
    /// to detect double-clicks: a `TreeMsg::Select` for a path within
    /// `TREE_DOUBLE_CLICK_WINDOW` of a previous click on the same
    /// path opens the file. Single clicks just highlight via
    /// `panel_ctx.selected_tree_path`. Cleared whenever the panel ctx
    /// is rebuilt from disk-state changes that invalidate the path
    /// indices. `None` when no row has been clicked yet this session.
    pub last_tree_click: Option<(Vec<usize>, std::time::Instant)>,
    pub active_bar_menu: Option<crate::active_bar::ActiveBarMenu>,
    pub selection_filters: std::collections::HashSet<crate::active_bar::SelectionFilter>,
    /// User-defined custom filter presets (capped at
    /// `crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT`). Loaded from
    /// `~/.config/signex/prefs.json` on launch and written back when
    /// edited from the Properties panel.
    pub custom_filter_presets: Vec<crate::active_bar::CustomFilterPreset>,
    /// Index of the active preset tab in the Properties-panel editor.
    /// Clamped to `0..custom_filter_presets.len()` whenever the list
    /// changes; ignored entirely when the list is empty.
    pub active_custom_filter_tab: usize,
    pub selection_slots: [Vec<signex_types::schematic::SelectedItem>; 8],
    pub last_tool: std::collections::HashMap<String, crate::active_bar::ActiveBarAction>,
    pub pending_power: Option<(String, String)>,
    pub pending_port: Option<(signex_types::schematic::LabelType, String)>,
}

impl InteractionState {
    pub fn active_canvas(&self) -> &SchematicCanvas {
        &self.canvas
    }

    pub fn active_canvas_mut(&mut self) -> &mut SchematicCanvas {
        &mut self.canvas
    }

    /// Per-window canvas lookup. Returns the per-window `SchematicCanvas`
    /// if one is registered (undocked windows), otherwise the main
    /// window's shared canvas. Writes from canvas events still go
    /// through the main-canvas slot; see the dispatch swap trick in
    /// `dispatch::ui::handle_canvas_event_in_window`.
    pub fn canvas_for_window(&self, window_id: iced::window::Id) -> &SchematicCanvas {
        self.canvases.get(&window_id).unwrap_or(&self.canvas)
    }

    #[allow(dead_code)]
    pub fn canvas_for_window_mut(&mut self, window_id: iced::window::Id) -> &mut SchematicCanvas {
        // `get_mut` returns `Option<&mut V>`. Match rather than
        // `contains_key` + `get_mut().unwrap()` to avoid the double
        // lookup and the unwrap.
        match self.canvases.get_mut(&window_id) {
            Some(canvas) => canvas,
            None => &mut self.canvas,
        }
    }
}
