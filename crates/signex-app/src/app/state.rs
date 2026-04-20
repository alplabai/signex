use std::path::PathBuf;

use signex_render::PowerPortStyle;
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
    pub panel_list_open: bool,
    pub preferences_open: bool,
    pub find_replace: crate::find_replace::FindReplaceState,
    pub preferences_nav: crate::preferences::PrefNav,
    pub preferences_draft_theme: ThemeId,
    pub preferences_draft_font: String,
    pub power_port_style: PowerPortStyle,
    pub preferences_draft_power_port_style: PowerPortStyle,
    pub preferences_dirty: bool,
    pub custom_theme: Option<signex_types::theme::CustomThemeFile>,
    /// Index of a tab queued for close-confirmation because it has unsaved
    /// edits. While `Some`, an overlay modal blocks other interaction with
    /// Save / Discard / Cancel actions.
    pub close_tab_confirm: Option<usize>,
    /// ERC results for the currently-visible sheet. Driven by the
    /// per-sheet cache below — switching tabs repoints this at the
    /// cached violations for that sheet, so markers and the Messages
    /// panel always match what's on the canvas.
    pub erc_violations: Vec<signex_erc::Violation>,
    /// Per-sheet ERC violation cache, keyed by the sheet's on-disk
    /// file path. Run ERC populates this for every sheet in the
    /// project; tab switches point `erc_violations` at the matching
    /// entry without rerunning the analysis.
    pub erc_violations_by_path:
        std::collections::HashMap<std::path::PathBuf, Vec<signex_erc::Violation>>,
    /// Per-rule severity override — if empty, the rule's default is used.
    pub erc_severity_override:
        std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    /// Net-color overrides keyed by net-label text. Superseded by the
    /// per-wire `wire_color_overrides` map below which the Active-Bar
    /// net-colour flood populates; kept here so a future net-name
    /// palette (maybe the F5 dialog) can cross-reference it without
    /// another round-trip through state plumbing.
    #[allow(dead_code)]
    pub net_colors: std::collections::HashMap<String, signex_types::theme::Color>,
    /// AutoFocus mode — when true, non-selected items dim on the canvas.
    pub auto_focus: bool,
    /// Annotate dialog open flag. When true, the Annotate-Schematics modal
    /// covers the canvas with its preview + confirm-apply UI.
    pub annotate_dialog_open: bool,
    /// Annotate dialog: order-of-processing choice. Controls the iteration
    /// order used to assign sequential numbers.
    pub annotate_order: AnnotateOrder,
    /// ERC dialog open flag — opens the full severity-matrix + pin-matrix UI.
    pub erc_dialog_open: bool,
    /// Reset-and-renumber confirmation modal. When true, the Design →
    /// Reset menu item shows a confirm before discarding every number.
    pub annotate_reset_confirm: bool,
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
    /// F5 Net Color palette state — open flag and transient edit buffer.
    pub net_color_palette_open: bool,
    /// Parameter Manager dialog state.
    pub parameter_manager_open: bool,
    /// Active "pick a reference item" mode for z-order operations
    /// (BringToFrontOf / SendToBackOf). When Some, the next canvas click
    /// resolves the reference uuid and submits the Reorder command.
    pub reorder_picker: Option<ReorderPicker>,
    /// Pin-connection matrix overrides — sparse map keyed by (row, col)
    /// pin-type index. Any entry present replaces the default severity
    /// for that pair; missing entries fall back to the hard-coded
    /// baseline in `pin_matrix_view`. Persisted alongside the ERC
    /// severity map.
    pub pin_matrix_overrides: std::collections::HashMap<(u8, u8), signex_erc::Severity>,
    /// Symbols whose designator the user locked against reannotation.
    /// Exposed as per-row checkboxes in the Annotate dialog; the engine
    /// skips these uuids in `annotate_with_seed_and_locks`.
    pub annotate_locked: std::collections::HashSet<uuid::Uuid>,
    /// Altium-style rubber-band selection mode. Drives how the box
    /// drag classifies hits (Inside / Outside / TouchingLine).
    pub selection_mode: signex_render::schematic::hit_test::SelectionMode,
    /// Net-color override armed from the Active Bar palette. When Some,
    /// the cursor turns into a paint-bucket over the canvas and the
    /// next click on a wire floods that color across every connected
    /// wire. Cleared after the click applies, or by Escape. Colors are
    /// render-time only — they do NOT write back to the .kicad_sch.
    pub pending_net_color: Option<signex_types::theme::Color>,
    /// Per-wire color overrides keyed by wire uuid. Populated by the
    /// net-color click; consulted when drawing wires. Not serialised.
    pub wire_color_overrides: std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
    /// Altium-style lasso in flight. `Some(points)` means the user
    /// started a lasso — each canvas click appends a vertex; a
    /// double-click or a click on the first vertex closes the polygon
    /// and commits the selection. Escape or right-click cancels.
    pub lasso_polygon: Option<Vec<signex_types::schematic::Point>>,
    /// App-level undo stack for net-color floods. Each entry is the
    /// full `wire_color_overrides` map captured before an action —
    /// popping one restores the previous state. This is separate from
    /// the engine's undo because net colours are render-only and
    /// shouldn't mix with document mutations.
    pub net_color_undo: Vec<std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>>,
    /// Custom net-color picker state. When `show = true`, a floating
    /// iced_aw ColorPicker appears anchored to the Active Bar button;
    /// `draft` is the user's pending pick — committed on OK.
    pub net_color_custom: NetColorCustomState,
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
    CloseTabConfirm,
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

pub struct DocumentState {
    pub dock: DockArea,
    pub tabs: Vec<TabInfo>,
    pub active_tab: usize,
    pub engine: Option<signex_engine::Engine>,
    pub project_path: Option<PathBuf>,
    pub project_data: Option<ProjectData>,
    pub panel_ctx: crate::panels::PanelContext,
    pub kicad_lib_dir: Option<PathBuf>,
    pub loaded_lib: std::collections::HashMap<String, signex_types::schematic::LibSymbol>,
}

pub struct InteractionState {
    pub current_tool: Tool,
    pub canvas: SchematicCanvas,
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
    pub last_mouse_pos: (f32, f32),
    pub active_bar_menu: Option<crate::active_bar::ActiveBarMenu>,
    pub selection_filters: std::collections::HashSet<crate::active_bar::SelectionFilter>,
    pub selection_slots: [Vec<signex_types::schematic::SelectedItem>; 8],
    pub last_tool: std::collections::HashMap<String, crate::active_bar::ActiveBarAction>,
    pub pending_power: Option<(String, String)>,
    pub pending_port: Option<(signex_types::schematic::LabelType, String)>,
}
