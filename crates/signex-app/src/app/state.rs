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
