//! Main Iced application — Message enum, update loop, view tree.

use std::path::PathBuf;

use iced::widget::{canvas, column, container, row};
use iced::{Element, Length, Subscription, Task, Theme};
use signex_types::coord::Unit;
use signex_types::pcb::PcbBoard;
use signex_types::project::ProjectData;
use signex_types::schematic::SchematicSheet;
use signex_types::theme::ThemeId;

use crate::canvas::{CanvasEvent, SchematicCanvas};
use crate::dock::{DockArea, DockMessage, PanelPosition};
use crate::menu_bar::{self, MenuMessage};
use crate::pcb_canvas::PcbCanvas;
use crate::panels::PanelKind;
use crate::status_bar;
use crate::tab_bar::{self, TabMessage};
use crate::toolbar::ToolMessage;
use signex_render::PowerPortStyle;

mod bootstrap;
mod helpers;
mod load_gateway;
mod mutation_gateway;
mod selection_message;
mod update;

use helpers::constrain_segments;

// ─── Message ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    LeftPanel,
    RightPanel,
    BottomPanel,
    ComponentsSplit,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    Menu(MenuMessage),
    Tool(ToolMessage),
    Tab(TabMessage),
    Dock(DockMessage),
    StatusBar(StatusBarMsg),
    CanvasEvent(CanvasEvent),
    #[allow(dead_code)]
    ThemeChanged(ThemeId),
    UnitCycled,
    GridToggle,
    GridCycle,
    DragStart(DragTarget),
    DragMove(f32, f32),
    DragEnd,
    FileOpened(Option<PathBuf>),
    #[allow(dead_code)]
    SchematicLoaded(Box<SchematicSheet>),
    // v0.5: Editing operations
    DeleteSelected,
    Undo,
    Redo,
    RotateSelected,
    MirrorSelectedX,
    MirrorSelectedY,
    // v0.6: Full editor operations
    Selection(selection_message::SelectionMessage),
    Copy,
    Cut,
    Paste,
    Duplicate,
    SaveFile,
    SaveFileAs(PathBuf),
    CycleDrawMode,
    CancelDrawing,
    // Panel list
    TogglePanelList,
    OpenPanel(crate::panels::PanelKind),
    // Active Bar
    ActiveBar(crate::active_bar::ActiveBarMsg),
    /// TAB pressed during placement — open pre-placement properties.
    PrePlacementTab,
    /// In-place text editing: text changed.
    TextEditChanged(String),
    /// In-place text editing: submit (Enter).
    TextEditSubmit,
    /// In-place text editing: cancel (Escape handled by tool switch).
    // Context menu
    ShowContextMenu(f32, f32),
    CloseContextMenu,
    ContextAction(ContextAction),
    // Preferences dialog
    OpenPreferences,
    ClosePreferences,
    PreferencesNav(crate::preferences::PrefNav),
    PreferencesMsg(crate::preferences::PrefMsg),
    WindowResized(f32, f32),
    Noop,
}

/// Actions available in the right-click context menu.
#[derive(Debug, Clone)]
#[allow(dead_code)] // variants used in match arms, constructed via dropdown actions
pub enum ContextAction {
    Copy,
    Cut,
    Paste,
    Delete,
    SelectAll,
    ZoomFit,
    RotateSelected,
    MirrorX,
    MirrorY,
}

/// State for in-place text editing overlay.
#[derive(Debug, Clone)]
pub struct TextEditState {
    /// UUID of the element being edited.
    pub uuid: uuid::Uuid,
    /// Element kind (Label, TextNote, Symbol).
    pub kind: signex_types::schematic::SelectedKind,
    /// Current editing text.
    pub text: String,
    /// Original text (for undo).
    pub original_text: String,
    /// Screen position for the text_input overlay.
    pub screen_x: f32,
    pub screen_y: f32,
}

/// State for the floating context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    /// Screen position to render the menu at.
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub enum StatusBarMsg {
    CycleUnit,
    ToggleGrid,
    ToggleSnap,
    TogglePanelList,
}

// ─── App State ────────────────────────────────────────────────

pub struct Signex {
    pub theme_id: ThemeId,
    pub unit: Unit,
    pub grid_visible: bool,
    pub snap_enabled: bool,
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub zoom: f64,
    pub dock: DockArea,
    pub tabs: Vec<TabInfo>,
    pub active_tab: usize,
    pub current_tool: Tool,
    pub canvas: SchematicCanvas,
    pub pcb_canvas: PcbCanvas,
    pub grid_size_mm: f32,
    /// Separate visible grid spacing (what dots are drawn at).
    pub visible_grid_mm: f32,
    /// Snap to electrical object hotspots (pin endpoints, wire ends).
    pub snap_hotspots: bool,
    /// UI font family (default: "Roboto").  Applies on next restart.
    pub ui_font_name: String,
    /// Canvas font family (default: "Iosevka").  Applies immediately to canvas text.
    pub canvas_font_name: String,
    /// Canvas font size in px (mapped to renderer text scale). Applies immediately.
    pub canvas_font_size: f32,
    /// Canvas font bold style toggle.
    pub canvas_font_bold: bool,
    /// Canvas font italic style toggle.
    pub canvas_font_italic: bool,
    pub engine: Option<signex_engine::Engine>,
    pub project_path: Option<PathBuf>,
    pub project_data: Option<ProjectData>,
    pub panel_ctx: crate::panels::PanelContext,
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_height: f32,
    // active_menu removed — iced_aw MenuBar handles its own overlay state
    pub kicad_lib_dir: Option<PathBuf>,
    pub loaded_lib: std::collections::HashMap<String, signex_types::schematic::LibSymbol>,
    pub dragging: Option<DragTarget>,
    pub drag_start_pos: Option<f32>,
    pub drag_start_size: f32,
    /// Mouse position when a tab drag started (for undock threshold).
    pub tab_drag_origin: Option<(f32, f32)>,
    /// Current window size for dock-zone detection.
    pub window_size: (f32, f32),
    // v0.5: Undo/Redo
    pub undo_stack: crate::undo::UndoStack,
    // v0.5: Wire drawing state
    pub wire_points: Vec<signex_types::schematic::Point>,
    pub wire_drawing: bool,
    // v0.6: Clipboard
    pub clipboard_wires: Vec<signex_types::schematic::Wire>,
    pub clipboard_buses: Vec<signex_types::schematic::Bus>,
    pub clipboard_labels: Vec<signex_types::schematic::Label>,
    pub clipboard_symbols: Vec<signex_types::schematic::Symbol>,
    pub clipboard_junctions: Vec<signex_types::schematic::Junction>,
    pub clipboard_no_connects: Vec<signex_types::schematic::NoConnect>,
    pub clipboard_text_notes: Vec<signex_types::schematic::TextNote>,
    pub draw_mode: DrawMode,
    /// In-place text editing state (double-click on label/text note).
    pub editing_text: Option<TextEditState>,
    /// Context menu state (shown on right-click).
    pub context_menu: Option<ContextMenuState>,
    /// Last known global mouse position (for context menu positioning).
    pub last_mouse_pos: (f32, f32),
    /// Active Bar open dropdown menu.
    pub active_bar_menu: Option<crate::active_bar::ActiveBarMenu>,
    /// Selection filter state — which object types are selectable.
    pub selection_filters: std::collections::HashSet<crate::active_bar::SelectionFilter>,
    /// Selection memory slots (Ctrl+1-8 store, Alt+1-8 recall).
    pub selection_slots: [Vec<signex_types::schematic::SelectedItem>; 8],
    /// Last-used tool per Active Bar group (Altium behavior: left-click re-activates).
    pub last_tool: std::collections::HashMap<String, crate::active_bar::ActiveBarAction>,
    /// Pending power port placement (net name, lib_id).
    pub pending_power: Option<(String, String)>,
    /// Pending port placement: (label_type, shape string).
    /// Global = "input"/"output"/"bidirectional"/"passive", Hierarchical = flag shape.
    pub pending_port: Option<(signex_types::schematic::LabelType, String)>,
    /// Panel list popup visible.
    pub panel_list_open: bool,
    /// Preferences dialog open.
    pub preferences_open: bool,
    /// Selected nav item in the Preferences dialog.
    pub preferences_nav: crate::preferences::PrefNav,
    /// Draft theme selected in Preferences (live preview; committed on Save).
    pub preferences_draft_theme: ThemeId,
    /// Draft UI font name pending Save.
    pub preferences_draft_font: String,
    /// Committed power port rendering style.
    pub power_port_style: PowerPortStyle,
    /// Draft power port rendering style pending Save.
    pub preferences_draft_power_port_style: PowerPortStyle,
    /// True when the Preferences draft differs from the saved state.
    pub preferences_dirty: bool,
    /// User-loaded custom theme (imported from JSON).
    pub custom_theme: Option<signex_types::theme::CustomThemeFile>,
}

/// Wire/bus drawing mode (Altium: cycle with Shift+Space).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawMode {
    /// Horizontal then vertical (90° corners, Altium default).
    #[default]
    Ortho90,
    /// 45-degree angles allowed.
    Angle45,
    /// Free angle — straight line from last point to cursor.
    FreeAngle,
}

impl DrawMode {
    pub fn next(self) -> Self {
        match self {
            DrawMode::Ortho90 => DrawMode::Angle45,
            DrawMode::Angle45 => DrawMode::FreeAngle,
            DrawMode::FreeAngle => DrawMode::Ortho90,
        }
    }
}

#[derive(Debug)]
pub struct SchematicTabSession {
    title: String,
    path: PathBuf,
    dirty: bool,
    engine: signex_engine::Engine,
}

impl SchematicTabSession {
    pub fn new(engine: signex_engine::Engine, title: String, path: PathBuf, dirty: bool) -> Self {
        Self {
            title,
            path,
            dirty,
            engine,
        }
    }

    pub fn document(&self) -> &SchematicSheet {
        self.engine.document()
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    pub fn save(&mut self) -> Result<(), signex_engine::EngineError> {
        self.engine.set_path(Some(self.path.clone()));
        self.engine.save()?;
        self.dirty = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<(), signex_engine::EngineError> {
        self.engine.save_as(&path)?;
        self.title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Schematic".to_string());
        self.path = path;
        self.dirty = false;
        Ok(())
    }

    pub fn into_parts(self) -> (signex_engine::Engine, String, PathBuf, bool) {
        (self.engine, self.title, self.path, self.dirty)
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum TabDocument {
    Schematic(SchematicTabSession),
    Pcb(PcbBoard),
}

impl TabDocument {
    pub fn as_schematic(&self) -> Option<&SchematicSheet> {
        match self {
            Self::Schematic(session) => Some(session.document()),
            Self::Pcb(_) => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_pcb(&self) -> Option<&PcbBoard> {
        match self {
            Self::Schematic(_) => None,
            Self::Pcb(board) => Some(board),
        }
    }
}

#[derive(Debug)]
pub struct TabInfo {
    pub title: String,
    pub path: PathBuf,
    /// Inactive-tab document session cache and tab-switch restore source.
    pub cached_document: Option<TabDocument>,
    pub dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Measure,
    Wire,
    Bus,
    Label,
    Component,
    Text,
    #[allow(dead_code)]
    NoConnect,
    #[allow(dead_code)]
    BusEntry,
    Line,
    Rectangle,
    Circle,
}

impl std::fmt::Display for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tool::Select => write!(f, "Select"),
            Tool::Measure => write!(f, "Measure"),
            Tool::Wire => write!(f, "Draw Wire"),
            Tool::Bus => write!(f, "Draw Bus"),
            Tool::Label => write!(f, "Place Label"),
            Tool::Component => write!(f, "Place Component"),
            Tool::Text => write!(f, "Place Text"),
            Tool::NoConnect => write!(f, "Place No Connect"),
            Tool::BusEntry => write!(f, "Place Bus Entry"),
            Tool::Line => write!(f, "Draw Line"),
            Tool::Rectangle => write!(f, "Draw Rectangle"),
            Tool::Circle => write!(f, "Draw Circle"),
        }
    }
}

// ─── Iced Application ─────────────────────────────────────────

impl Signex {
    fn clear_measurement(&mut self) {
        self.canvas.reset_measurement();
    }

    fn clear_transient_schematic_tool_state(&mut self) {
        self.pending_power = None;
        self.pending_port = None;
        self.canvas.ghost_label = None;
        self.canvas.tool_preview = None;
        self.panel_ctx.pre_placement = None;
        self.editing_text = None;
        self.clear_measurement();

        if self.wire_drawing {
            self.wire_drawing = false;
            self.wire_points.clear();
            self.canvas.wire_preview.clear();
            self.canvas.drawing_mode = false;
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeChanged(id) => {
                self.theme_id = id;
                self.update_canvas_theme();
            }
            Message::UnitCycled | Message::StatusBar(StatusBarMsg::CycleUnit) => {
                self.unit = match self.unit {
                    Unit::Mm => Unit::Mil,
                    Unit::Mil => Unit::Inch,
                    Unit::Inch => Unit::Micrometer,
                    Unit::Micrometer => Unit::Mm,
                };
            }
            Message::GridToggle | Message::StatusBar(StatusBarMsg::ToggleGrid) => {
                self.grid_visible = !self.grid_visible;
                self.canvas.grid_visible = self.grid_visible;
                self.pcb_canvas.grid_visible = self.grid_visible;
                self.canvas.clear_bg_cache();
                self.pcb_canvas.clear_bg_cache();
            }
            Message::DragStart(target) => {
                #[cfg(debug_assertions)]
                eprintln!("[drag] START {:?}", target);
                self.dragging = Some(target);
                self.drag_start_pos = None; // set on first move
                self.drag_start_size = match target {
                    DragTarget::LeftPanel => self.left_width,
                    DragTarget::RightPanel => self.right_width,
                    DragTarget::BottomPanel => self.bottom_height,
                    DragTarget::ComponentsSplit => self.panel_ctx.components_split,
                };
            }
            Message::DragMove(x, y) => {
                // Always track global mouse position (for context menu positioning)
                self.last_mouse_pos = (x, y);
                if let Some(target) = self.dragging {
                    let pos = match target {
                        DragTarget::LeftPanel | DragTarget::RightPanel => x,
                        DragTarget::BottomPanel | DragTarget::ComponentsSplit => y,
                    };
                    if self.drag_start_pos.is_none() {
                        self.drag_start_pos = Some(pos);
                    }
                    if let Some(start) = self.drag_start_pos {
                        let delta = pos - start;
                        let (current, new_val) = match target {
                            DragTarget::LeftPanel => (
                                self.left_width,
                                (self.drag_start_size + delta).clamp(100.0, 500.0),
                            ),
                            DragTarget::RightPanel => (
                                self.right_width,
                                (self.drag_start_size - delta).clamp(100.0, 500.0),
                            ),
                            DragTarget::BottomPanel => (
                                self.bottom_height,
                                (self.drag_start_size - delta).clamp(60.0, 400.0),
                            ),
                            DragTarget::ComponentsSplit => (
                                self.panel_ctx.components_split,
                                (self.drag_start_size + delta).clamp(80.0, 600.0),
                            ),
                        };
                        // Only re-render on meaningful change (reduces lag)
                        let new_val = new_val.round();
                        if (current - new_val).abs() >= 1.0 {
                            match target {
                                DragTarget::LeftPanel => self.left_width = new_val,
                                DragTarget::RightPanel => self.right_width = new_val,
                                DragTarget::BottomPanel => self.bottom_height = new_val,
                                DragTarget::ComponentsSplit => {
                                    self.panel_ctx.components_split = new_val
                                }
                            }
                        }
                    }
                }
                // Tab drag-to-undock: if mouse moved >20px from press origin, undock the tab.
                if let (Some((pos, idx)), Some((ox, oy))) =
                    (self.dock.tab_drag, self.tab_drag_origin)
                {
                    let dx = x - ox;
                    let dy = y - oy;
                    if (dx * dx + dy * dy).sqrt() > 20.0 {
                        self.dock
                            .update(DockMessage::UndockPanel(pos, idx));
                        self.tab_drag_origin = None;
                    }
                }
                // Move floating panels that are being dragged
                for fp in &mut self.dock.floating {
                    if fp.dragging {
                        let (old_x, old_y) = self.last_mouse_pos;
                        fp.x = x - fp.width / 2.0;
                        fp.y = y - 15.0; // offset so title bar is under cursor
                        let _ = (old_x, old_y); // suppress unused
                    }
                }
            }
            Message::DragEnd if self.dragging.is_some() => {
                #[cfg(debug_assertions)]
                eprintln!("[drag] END");
                self.dragging = None;
                self.drag_start_pos = None;
            }
            Message::WindowResized(w, h) => {
                self.window_size = (w, h);
            }
            Message::DragEnd => {
                // Clear tab drag state
                self.dock.tab_drag = None;
                self.tab_drag_origin = None;
                // Dock floating panels if dragged to an edge zone
                let (mx, my) = self.last_mouse_pos;
                let (ww, wh) = self.window_size;
                let dock_zone = 120.0;
                let has_dragging = self.dock.floating.iter().any(|fp| fp.dragging);
                #[cfg(debug_assertions)]
                eprintln!("[dock-end] mouse=({mx:.0},{my:.0}) win=({ww:.0},{wh:.0}) floating={} dragging={has_dragging}",
                    self.dock.floating.len());
                if let Some(drag_idx) = self.dock.floating.iter().position(|fp| fp.dragging) {
                    let target = if mx < dock_zone {
                        Some(PanelPosition::Left)
                    } else if mx > ww - dock_zone {
                        Some(PanelPosition::Right)
                    } else if my > wh - dock_zone {
                        Some(PanelPosition::Bottom)
                    } else {
                        None
                    };
                    #[cfg(debug_assertions)]
                    eprintln!("[dock-end] target={target:?}");
                    if let Some(pos) = target {
                        self.dock.update(DockMessage::DockFloatingTo(drag_idx, pos));
                    } else {
                        self.dock.floating[drag_idx].dragging = false;
                    }
                } else {
                    for fp in &mut self.dock.floating {
                        fp.dragging = false;
                    }
                }
            }
            Message::GridCycle => {
                // Cycle grid and clear cache so it redraws
                self.canvas.clear_bg_cache();
            }
            Message::StatusBar(StatusBarMsg::ToggleSnap) => {
                self.snap_enabled = !self.snap_enabled;
                self.canvas.snap_enabled = self.snap_enabled;
            }
            Message::StatusBar(StatusBarMsg::TogglePanelList) => {
                return self.update(Message::TogglePanelList);
            }
            Message::CanvasEvent(CanvasEvent::CursorAt { x, y, zoom_pct }) => {
                self.cursor_x = x as f64;
                self.cursor_y = y as f64;
                self.zoom = zoom_pct;
                if self.current_tool == Tool::Measure
                    && self.canvas.measure_start.is_some()
                    && !self.canvas.measure_locked
                {
                    let (mx, my) = if self.snap_enabled {
                        let gs = self.grid_size_mm as f64;
                        ((x as f64 / gs).round() * gs, (y as f64 / gs).round() * gs)
                    } else {
                        (x as f64, y as f64)
                    };
                    self.canvas.measure_end = Some(signex_types::schematic::Point::new(mx, my));
                }
                // Don't clear bg cache here — crosshair is in the overlay (uncached) layer.
                // Grid only needs redraw on zoom/pan/grid-change.
            }
            Message::CanvasEvent(CanvasEvent::Clicked { world_x, world_y }) => {
                // Snap to grid if enabled
                let (wx, wy) = if self.snap_enabled {
                    let gs = self.grid_size_mm as f64;
                    ((world_x / gs).round() * gs, (world_y / gs).round() * gs)
                } else {
                    (world_x, world_y)
                };

                match self.current_tool {
                    Tool::Measure => {
                        let point = signex_types::schematic::Point::new(wx, wy);
                        if self.canvas.measure_start.is_some() && !self.canvas.measure_locked {
                            self.canvas.measure_end = Some(point);
                            self.canvas.measure_locked = true;
                        } else {
                            self.canvas.measure_start = Some(point);
                            self.canvas.measure_end = Some(point);
                            self.canvas.measure_locked = false;
                        }
                    }
                    Tool::Wire => {
                        let pt = signex_types::schematic::Point::new(wx, wy);
                        if !self.wire_drawing {
                            self.wire_drawing = true;
                            self.wire_points.clear();
                            self.wire_points.push(pt);
                            self.canvas.wire_preview = self.wire_points.clone();
                            self.canvas.drawing_mode = true;
                            self.canvas.draw_mode = self.draw_mode;
                            self.canvas.tool_preview = None;
                        } else if let Some(&start) = self.wire_points.last() {
                            // Apply draw mode constraints
                            let segments = constrain_segments(start, pt, self.draw_mode);
                            let mut wire_commands = Vec::new();
                            for seg in &segments {
                                let wire = signex_types::schematic::Wire {
                                    uuid: uuid::Uuid::new_v4(),
                                    start: seg.0,
                                    end: seg.1,
                                    stroke_width: 0.0,
                                };
                                wire_commands.push(signex_engine::Command::PlaceWireSegment { wire });
                            }
                            if !wire_commands.is_empty() {
                                self.apply_engine_commands(wire_commands, false, false);
                            }
                            let end_pt = segments.last().map(|s| s.1).unwrap_or(pt);
                            self.wire_points = vec![end_pt];
                            self.canvas.wire_preview = vec![end_pt];
                        }
                    }
                    Tool::Bus => {
                        let pt = signex_types::schematic::Point::new(wx, wy);
                        if !self.wire_drawing {
                            self.wire_drawing = true;
                            self.wire_points.clear();
                            self.wire_points.push(pt);
                            self.canvas.wire_preview = self.wire_points.clone();
                            self.canvas.drawing_mode = true;
                            self.canvas.draw_mode = self.draw_mode;
                            self.canvas.tool_preview = None;
                        } else if let Some(&start) = self.wire_points.last() {
                            let segments = constrain_segments(start, pt, self.draw_mode);
                            let mut bus_commands = Vec::new();
                            for seg in &segments {
                                let bus = signex_types::schematic::Bus {
                                    uuid: uuid::Uuid::new_v4(),
                                    start: seg.0,
                                    end: seg.1,
                                };
                                bus_commands.push(signex_engine::Command::PlaceBus { bus });
                            }
                            if !bus_commands.is_empty() {
                                self.apply_engine_commands(bus_commands, false, false);
                            }
                            let end_pt = segments.last().map(|s| s.1).unwrap_or(pt);
                            self.wire_points = vec![end_pt];
                            self.canvas.wire_preview = vec![end_pt];
                        }
                    }
                    Tool::Component if self.pending_power.is_some() => {
                        // Place power port symbol
                        if let Some((ref net_name, ref lib_id)) = self.pending_power {
                            let sym = signex_types::schematic::Symbol {
                                uuid: uuid::Uuid::new_v4(),
                                lib_id: lib_id.clone(),
                                reference: "#PWR?".to_string(),
                                value: net_name.clone(),
                                footprint: String::new(),
                                position: signex_types::schematic::Point::new(wx, wy),
                                rotation: 0.0,
                                mirror_x: false,
                                mirror_y: false,
                                unit: 1,
                                is_power: true,
                                ref_text: None,
                                val_text: Some(signex_types::schematic::TextProp {
                                    position: signex_types::schematic::Point::new(wx, wy - 1.27),
                                    rotation: 0.0,
                                    font_size: 1.27,
                                    justify_h: signex_types::schematic::HAlign::Center,
                                    justify_v: signex_types::schematic::VAlign::default(),
                                    hidden: false,
                                }),
                                fields_autoplaced: true,
                                dnp: false,
                                in_bom: false,
                                on_board: true,
                                exclude_from_sim: false,
                                locked: false,
                                fields: std::collections::HashMap::new(),
                            };
                            self.apply_engine_command(
                                signex_engine::Command::PlaceSymbol { symbol: sym },
                                false,
                                false,
                            );
                        }
                        // Stay in power placement mode for continuous placement
                    }
                    Tool::NoConnect => {
                        let nc = signex_types::schematic::NoConnect {
                            uuid: uuid::Uuid::new_v4(),
                            position: signex_types::schematic::Point::new(wx, wy),
                        };
                        self.apply_engine_command(
                            signex_engine::Command::PlaceNoConnect { no_connect: nc },
                            false,
                            false,
                        );
                        // Stay in NoConnect mode for continuous placement
                    }
                    Tool::BusEntry => {
                        let be = signex_types::schematic::BusEntry {
                            uuid: uuid::Uuid::new_v4(),
                            position: signex_types::schematic::Point::new(wx, wy),
                            size: (2.54, 2.54),
                        };
                        self.apply_engine_command(
                            signex_engine::Command::PlaceBusEntry { bus_entry: be },
                            false,
                            false,
                        );
                        // Stay in BusEntry mode for continuous placement
                    }
                    Tool::Text => {
                        let note_text = self
                            .panel_ctx
                            .pre_placement
                            .as_ref()
                            .map(|pp| pp.label_text.clone())
                            .unwrap_or_else(|| "Text".to_string());
                        let tn = signex_types::schematic::TextNote {
                            uuid: uuid::Uuid::new_v4(),
                            text: note_text,
                            position: signex_types::schematic::Point::new(wx, wy),
                            rotation: 0.0,
                            font_size: 1.27,
                            justify_h: signex_types::schematic::HAlign::Left,
                            justify_v: signex_types::schematic::VAlign::default(),
                        };
                        self.apply_engine_command(
                            signex_engine::Command::PlaceTextNote { text_note: tn },
                            false,
                            false,
                        );
                        self.current_tool = Tool::Select;
                    }
                    _ => {
                        return self.handle_selection_message(
                            selection_message::SelectionMessage::HitAt { world_x, world_y },
                        );
                    }
                }
            }
            Message::CanvasEvent(CanvasEvent::MoveSelected { dx, dy }) => {
                // Snap delta to grid
                let (dx, dy) = if self.snap_enabled {
                    let gs = self.grid_size_mm as f64;
                    ((dx / gs).round() * gs, (dy / gs).round() * gs)
                } else {
                    (dx, dy)
                };
                if (dx.abs() > 0.001 || dy.abs() > 0.001) && !self.canvas.selected.is_empty() {
                    self.apply_engine_command(
                        signex_engine::Command::MoveSelection {
                            items: self.canvas.selected.clone(),
                            dx,
                            dy,
                        },
                        true,
                        true,
                    );
                }
            }
            Message::TextEditChanged(text) => {
                if let Some(ref mut state) = self.editing_text {
                    state.text = text;
                }
            }
            Message::TextEditSubmit => {
                if let Some(state) = self.editing_text.take()
                    && state.text != state.original_text
                {
                    let engine_command = match state.kind {
                        signex_types::schematic::SelectedKind::Label => {
                            signex_engine::Command::UpdateText {
                                target: signex_engine::TextTarget::Label(state.uuid),
                                value: state.text.clone(),
                            }
                        }
                        signex_types::schematic::SelectedKind::TextNote => {
                            signex_engine::Command::UpdateText {
                                target: signex_engine::TextTarget::TextNote(state.uuid),
                                value: state.text.clone(),
                            }
                        }
                        _ => return Task::none(),
                    };
                    self.apply_engine_command(engine_command, false, true);
                }
            }
            Message::CanvasEvent(CanvasEvent::DoubleClicked {
                world_x,
                world_y,
                screen_x,
                screen_y,
            }) => {
                // Finish wire drawing on double-click
                if self.wire_drawing {
                    self.wire_drawing = false;
                    self.wire_points.clear();
                    self.canvas.wire_preview.clear();
                    self.canvas.drawing_mode = false;
                } else if let Some(snapshot) = self.active_render_snapshot() {
                    // In-place text editing: check if double-clicked on a label or text note
                    use signex_types::schematic::SelectedKind;
                    if let Some(hit) =
                        signex_render::schematic::hit_test::hit_test(snapshot, world_x, world_y)
                    {
                        let edit_info = match hit.kind {
                            SelectedKind::Label => snapshot
                                .labels
                                .iter()
                                .find(|l| l.uuid == hit.uuid)
                                .map(|l| (l.text.clone(), SelectedKind::Label)),
                            SelectedKind::TextNote => snapshot
                                .text_notes
                                .iter()
                                .find(|t| t.uuid == hit.uuid)
                                .map(|t| (t.text.clone(), SelectedKind::TextNote)),
                            _ => None,
                        };
                        if let Some((text, kind)) = edit_info {
                            // Convert world coords to approximate screen position
                            // (status bar shows cursor position, close enough)
                            self.editing_text = Some(TextEditState {
                                uuid: hit.uuid,
                                kind,
                                original_text: text.clone(),
                                text,
                                screen_x,
                                screen_y,
                            });
                        }
                    }
                }
            }
            Message::CanvasEvent(CanvasEvent::BoxSelect { x1, y1, x2, y2 }) => {
                return self.handle_selection_message(selection_message::SelectionMessage::BoxSelect {
                    x1,
                    y1,
                    x2,
                    y2,
                });
            }
            Message::PrePlacementTab => {
                // Only activate during placement tools (not Select)
                if self.current_tool != Tool::Select && self.current_tool != Tool::Measure {
                    let tool_name = format!("{}", self.current_tool);
                    let label_text = self
                        .panel_ctx
                        .pre_placement
                        .as_ref()
                        .map(|pp| pp.label_text.clone())
                        .unwrap_or_else(|| "NET".to_string());
                    let designator = self
                        .panel_ctx
                        .pre_placement
                        .as_ref()
                        .map(|pp| pp.designator.clone())
                        .unwrap_or_default();
                    self.panel_ctx.pre_placement =
                        Some(crate::panels::PrePlacementData {
                            tool_name,
                            label_text,
                            designator,
                            rotation: 0.0,
                        });
                    // Ensure Properties panel is visible in the right dock
                    self.dock
                        .add_panel(PanelPosition::Right, crate::panels::PanelKind::Properties);
                }
            }
            Message::CycleDrawMode => {
                self.draw_mode = self.draw_mode.next();
                self.canvas.draw_mode = self.draw_mode;
            }
            Message::CancelDrawing => {
                // Right-click cancels wire/bus drawing (Altium behavior)
                if self.wire_drawing {
                    self.wire_drawing = false;
                    self.wire_points.clear();
                    self.canvas.wire_preview.clear();
                    self.canvas.drawing_mode = false;
                    self.current_tool = Tool::Select;
                    self.canvas.tool_preview = None;
                }
            }
            Message::CanvasEvent(CanvasEvent::CursorMoved) => {
                // Pan/zoom changed — grid needs pixel-perfect redraw, content + selection
                // re-render with updated camera. Content cache is cleared here so it
                // picks up the new camera transform on next draw.
                self.canvas.clear_bg_cache();
                self.canvas.clear_content_cache();
                self.canvas.clear_overlay_cache();
                self.pcb_canvas.clear_bg_cache();
                self.pcb_canvas.clear_content_cache();
                // Clear pending fit after it's been applied
                self.canvas.pending_fit.set(None);
                self.pcb_canvas.pending_fit.set(None);
            }
            Message::CanvasEvent(CanvasEvent::FitAll) => {
                if self.has_active_schematic() {
                    self.canvas.fit_to_paper();
                    self.canvas.clear_bg_cache();
                    self.canvas.clear_content_cache();
                } else if self.has_active_pcb() {
                    self.pcb_canvas.fit_to_board();
                    self.pcb_canvas.clear_bg_cache();
                    self.pcb_canvas.clear_content_cache();
                }
            }
            Message::Tool(ToolMessage::SelectTool(tool)) => {
                self.current_tool = tool;
                // Set tool preview text for placement modes
                // No cursor text — Active Bar shows the active tool
                self.canvas.tool_preview = None;
                if tool == Tool::Measure {
                    self.clear_transient_schematic_tool_state();
                } else {
                    self.clear_measurement();
                    if tool == Tool::Select {
                        self.clear_transient_schematic_tool_state();
                    }
                }
            }
            Message::Menu(msg) => {
                return self.handle_menu(msg);
            }
            Message::Tab(msg) => {
                self.handle_tab(msg);
            }
            Message::Dock(msg) => {
                use signex_widgets::tree_view::{TreeIcon, TreeMsg, get_node};
                match &msg {
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetUnit(unit)) => {
                        self.unit = *unit;
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleGrid) => {
                        self.grid_visible = !self.grid_visible;
                        self.canvas.grid_visible = self.grid_visible;
                        self.pcb_canvas.grid_visible = self.grid_visible;
                        self.canvas.clear_bg_cache();
                        self.pcb_canvas.clear_bg_cache();
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleSnap) => {
                        self.snap_enabled = !self.snap_enabled;
                        self.canvas.snap_enabled = self.snap_enabled;
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::PropertiesTab(
                        idx,
                    )) => {
                        self.panel_ctx.properties_tab = *idx;
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SelectLibrary(
                        name,
                    )) => {
                        let name = name.clone();
                        if let Some(dir) = &self.kicad_lib_dir {
                            let path = dir.join(format!("{name}.kicad_sym"));
                            match std::fs::read_to_string(&path) {
                                Ok(content) => match kicad_parser::parse_symbol_lib(&content) {
                                    Ok(symbols) => {
                                        let mut syms: Vec<(String, usize)> = symbols
                                            .iter()
                                            .map(|(id, lib)| (id.clone(), lib.pins.len()))
                                            .collect();
                                        syms.sort_by(|a, b| a.0.cmp(&b.0));
                                        self.panel_ctx.library_symbols = syms;
                                        self.panel_ctx.active_library = Some(name);
                                        self.panel_ctx.selected_component = None;
                                        self.panel_ctx.selected_pins.clear();
                                        self.loaded_lib = symbols;
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to parse library: {e}");
                                    }
                                },
                                Err(e) => eprintln!("Failed to read {}: {e}", path.display()),
                            }
                        }
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ComponentFilter(
                        filter,
                    )) => {
                        self.panel_ctx.component_filter = filter.clone();
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleSection(
                        key,
                    )) => {
                        let key = key.clone();
                        if !self.panel_ctx.collapsed_sections.remove(&key) {
                            self.panel_ctx.collapsed_sections.insert(key);
                        }
                    }
                    // ── Property editing messages ──
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::EditSymbolDesignator(uuid, new_val),
                    ) => {
                        self.apply_engine_command(
                            signex_engine::Command::UpdateText {
                                target: signex_engine::TextTarget::SymbolReference(*uuid),
                                value: new_val.clone(),
                            },
                            false,
                            false,
                        );
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::EditSymbolValue(uuid, new_val),
                    ) => {
                        self.apply_engine_command(
                            signex_engine::Command::UpdateText {
                                target: signex_engine::TextTarget::SymbolValue(*uuid),
                                value: new_val.clone(),
                            },
                            false,
                            false,
                        );
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::EditSymbolFootprint(uuid, new_val),
                    ) => {
                        self.apply_engine_command(
                            signex_engine::Command::UpdateSymbolFootprint {
                                symbol_id: *uuid,
                                footprint: new_val.clone(),
                            },
                            false,
                            false,
                        );
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::ToggleSymbolMirrorX(uuid),
                    ) => {
                        self.apply_engine_command(
                            signex_engine::Command::MirrorSelection {
                                items: vec![signex_types::schematic::SelectedItem::new(
                                    *uuid,
                                    signex_types::schematic::SelectedKind::Symbol,
                                )],
                                axis: signex_engine::MirrorAxis::Vertical,
                            },
                            true,
                            true,
                        );
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::ToggleSymbolMirrorY(uuid),
                    ) => {
                        self.apply_engine_command(
                            signex_engine::Command::MirrorSelection {
                                items: vec![signex_types::schematic::SelectedItem::new(
                                    *uuid,
                                    signex_types::schematic::SelectedKind::Symbol,
                                )],
                                axis: signex_engine::MirrorAxis::Horizontal,
                            },
                            true,
                            true,
                        );
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::ToggleSymbolLocked(uuid),
                    ) => {
                        let _ = *uuid;
                        // TODO: implement locked toggling
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::ToggleSymbolDnp(uuid),
                    ) => {
                        let _ = *uuid;
                        // TODO: implement DNP toggling
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::EditLabelText(uuid, new_text),
                    ) => {
                        self.apply_engine_command(
                            signex_engine::Command::UpdateText {
                                target: signex_engine::TextTarget::Label(*uuid),
                                value: new_text.clone(),
                            },
                            false,
                            false,
                        );
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::EditTextNoteText(uuid, new_text),
                    ) => {
                        self.apply_engine_command(
                            signex_engine::Command::UpdateText {
                                target: signex_engine::TextTarget::TextNote(*uuid),
                                value: new_text.clone(),
                            },
                            false,
                            false,
                        );
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::SetPrePlacementText(text),
                    ) => {
                        if let Some(ref mut pp) = self.panel_ctx.pre_placement {
                            pp.label_text = text.clone();
                        }
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::SetPrePlacementDesignator(text),
                    ) => {
                        if let Some(ref mut pp) = self.panel_ctx.pre_placement {
                            pp.designator = text.clone();
                        }
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::SetPrePlacementRotation(rot),
                    ) => {
                        if let Some(ref mut pp) = self.panel_ctx.pre_placement {
                            pp.rotation = *rot;
                        }
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::ConfirmPrePlacement,
                    ) => {
                        // Close pre-placement — values are read when placing
                        self.panel_ctx.pre_placement = None;
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetGridSize(
                        size,
                    )) => {
                        self.grid_size_mm = *size;
                        self.panel_ctx.grid_size_mm = *size;
                        self.canvas.snap_grid_mm = *size as f64;
                        self.canvas.clear_bg_cache();
                        self.pcb_canvas.clear_bg_cache();
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetVisibleGridSize(
                        size,
                    )) => {
                        self.visible_grid_mm = *size;
                        self.panel_ctx.visible_grid_mm = *size;
                        self.canvas.visible_grid_mm = *size as f64;
                        self.pcb_canvas.visible_grid_mm = *size as f64;
                        self.canvas.clear_bg_cache();
                        self.pcb_canvas.clear_bg_cache();
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleSnapHotspots) => {
                        self.snap_hotspots = !self.snap_hotspots;
                        self.panel_ctx.snap_hotspots = self.snap_hotspots;
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetUiFont(name)) => {
                        self.ui_font_name = name.clone();
                        self.panel_ctx.ui_font_name = name.clone();
                        crate::fonts::write_ui_font_pref(name);
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetCanvasFont(name)) => {
                        self.canvas_font_name = name.clone();
                        self.panel_ctx.canvas_font_name = name.clone();
                        signex_render::set_canvas_font_name(name);
                        signex_render::set_canvas_font_style(
                            self.canvas_font_bold,
                            self.canvas_font_italic,
                        );
                        self.canvas.clear_content_cache();
                        self.canvas.clear_overlay_cache();
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetCanvasFontSize(size)) => {
                        self.canvas_font_size = *size;
                        self.panel_ctx.canvas_font_size = *size;
                        signex_render::set_canvas_font_size(*size);
                        self.canvas.clear_content_cache();
                        self.canvas.clear_overlay_cache();
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetCanvasFontBold(bold)) => {
                        self.canvas_font_bold = *bold;
                        self.panel_ctx.canvas_font_bold = *bold;
                        signex_render::set_canvas_font_style(
                            self.canvas_font_bold,
                            self.canvas_font_italic,
                        );
                        self.canvas.clear_content_cache();
                        self.canvas.clear_overlay_cache();
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetCanvasFontItalic(italic)) => {
                        self.canvas_font_italic = *italic;
                        self.panel_ctx.canvas_font_italic = *italic;
                        signex_render::set_canvas_font_style(
                            self.canvas_font_bold,
                            self.canvas_font_italic,
                        );
                        self.canvas.clear_content_cache();
                        self.canvas.clear_overlay_cache();
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::OpenCanvasFontPopup) => {
                        self.panel_ctx.canvas_font_popup_open = true;
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::CloseCanvasFontPopup) => {
                        self.panel_ctx.canvas_font_popup_open = false;
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::SetMarginVertical(zones),
                    ) => {
                        let _ = *zones;
                        // Margins stored in schematic — handle when editable
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::SetMarginHorizontal(zones),
                    ) => {
                        let _ = *zones;
                        // Margins stored in schematic — handle when editable
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::DragComponentsSplit,
                    ) => {
                        self.dragging = Some(DragTarget::ComponentsSplit);
                        self.drag_start_pos = None;
                        self.drag_start_size = self.panel_ctx.components_split;
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SelectComponent(
                        name,
                    )) => {
                        let name = name.clone();
                        if let Some(sym) = self.loaded_lib.get(&name) {
                            self.panel_ctx.selected_component = Some(name);
                            self.panel_ctx.selected_pins = sym
                                .pins
                                .iter()
                                .map(|lp| {
                                    (
                                        lp.pin.number.clone(),
                                        lp.pin.name.clone(),
                                        format!("{:?}", lp.pin.pin_type),
                                    )
                                })
                                .collect();
                            self.panel_ctx.selected_lib_symbol = Some(sym.clone());
                        }
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::Tree(
                        TreeMsg::Toggle(path),
                    )) => {
                        let path = path.clone();
                        signex_widgets::tree_view::toggle(&mut self.panel_ctx.project_tree, &path);
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::Tree(
                        TreeMsg::Select(path),
                    )) => {
                        if let Some(node) = get_node(&self.panel_ctx.project_tree, path)
                            && matches!(node.icon, TreeIcon::Schematic | TreeIcon::Pcb)
                        {
                            let filename = node.label.clone();
                            if let Some(dir) = self.project_path.as_ref().and_then(|p| p.parent()) {
                                let file_path = dir.join(&filename);
                                if file_path.exists() {
                                    // Already open? Switch to it
                                    if let Some(idx) =
                                        self.tabs.iter().position(|t| t.path == file_path)
                                    {
                                        if idx != self.active_tab {
                                            self.park_active_schematic_session();
                                            self.active_tab = idx;
                                            self.sync_active_tab();
                                        }
                                    } else if filename.ends_with(".kicad_sch")
                                        || filename.ends_with(".snxsch")
                                    {
                                        // Open new schematic tab
                                        match kicad_parser::parse_schematic_file(&file_path) {
                                            Ok(sheet) => {
                                                self.open_schematic_tab(
                                                    file_path,
                                                    filename.replace(".kicad_sch", ""),
                                                    sheet,
                                                );
                                            }
                                            Err(e) => eprintln!("Failed to parse {filename}: {e}"),
                                        }
                                    } else if filename.ends_with(".kicad_pcb")
                                        || filename.ends_with(".snxpcb")
                                    {
                                        match kicad_parser::parse_pcb_file(&file_path) {
                                            Ok(board) => {
                                                let title = filename
                                                    .trim_end_matches(".kicad_pcb")
                                                    .trim_end_matches(".snxpcb")
                                                    .to_string();
                                                self.open_pcb_tab(file_path, title, board);
                                            }
                                            Err(e) => eprintln!("Failed to parse {filename}: {e}"),
                                        }
                                    } else {
                                        eprintln!("Unsupported project tree document: {filename}");
                                    }
                                }
                            }
                        }
                    }
                    crate::dock::DockMessage::TabDragStart(..) => {
                        self.tab_drag_origin = Some(self.last_mouse_pos);
                    }
                    crate::dock::DockMessage::FloatingDragEnd(idx) => {
                        let idx = *idx;
                        // Detect dock zone from floating panel position
                        if let Some(fp) = self.dock.floating.get(idx) {
                            let (ww, wh) = self.window_size;
                            let zone = 120.0;
                            let cx = fp.x + fp.width / 2.0;
                            let cy = fp.y + fp.height / 4.0;
                            let target = if cx < zone {
                                Some(PanelPosition::Left)
                            } else if cx > ww - zone {
                                Some(PanelPosition::Right)
                            } else if cy > wh - zone {
                                Some(PanelPosition::Bottom)
                            } else {
                                None
                            };
                            eprintln!("[dock-back] fp=({:.0},{:.0}) win=({ww:.0},{wh:.0}) target={target:?}",
                                fp.x, fp.y);
                            if let Some(pos) = target {
                                self.dock.update(
                                    DockMessage::DockFloatingTo(idx, pos),
                                );
                                return Task::none();
                            }
                        }
                    }
                    _ => {}
                }
                self.dock.update(msg);
            }
            Message::FileOpened(path) => self.handle_file_opened(path),
            Message::DeleteSelected => self.handle_delete_selected(),
            Message::Undo => self.handle_undo(),
            Message::Redo => self.handle_redo(),
            Message::RotateSelected => self.handle_rotate_selected(),
            Message::MirrorSelectedX => self.handle_mirror_selected_x(),
            Message::MirrorSelectedY => self.handle_mirror_selected_y(),
            Message::CanvasEvent(CanvasEvent::CtrlClicked { world_x, world_y }) => {
                // Ctrl+click: toggle selection (multi-select)
                if let Some(snapshot) = self.active_render_snapshot()
                    && let Some(hit) =
                        signex_render::schematic::hit_test::hit_test(snapshot, world_x, world_y)
                {
                    if let Some(pos) = self.canvas.selected.iter().position(|s| s.uuid == hit.uuid)
                    {
                        self.canvas.selected.remove(pos);
                    } else {
                        self.canvas.selected.push(hit);
                    }
                    self.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            Message::Selection(msg) => return self.handle_selection_message(msg),
            Message::Cut => return self.handle_cut(),
            Message::Copy => self.handle_copy(),
            Message::Paste => self.handle_paste(),
            Message::Duplicate => self.handle_duplicate(),
            Message::SaveFile => self.handle_save_file(),
            Message::SaveFileAs(path) => self.handle_save_file_as(path),
            Message::SchematicLoaded(sheet) => {
                self.load_schematic_into_active_tab(*sheet);
            }
            Message::TogglePanelList => {
                self.panel_list_open = !self.panel_list_open;
                return Task::none();
            }
            Message::OpenPanel(kind) => {
                self.panel_list_open = false;
                self.dock.add_panel(crate::dock::PanelPosition::Right, kind);
                return Task::none();
            }
            Message::OpenPreferences => return self.handle_open_preferences(),
            Message::ClosePreferences => return self.handle_close_preferences(),
            Message::PreferencesNav(nav) => return self.handle_preferences_nav(nav),
            Message::PreferencesMsg(msg) => return self.handle_preferences_msg(msg),
            // Active Bar
            Message::ActiveBar(msg) => {
                use crate::active_bar::{ActiveBarAction, ActiveBarMsg};
                match msg {
                    ActiveBarMsg::ToggleMenu(menu) => {
                        self.active_bar_menu = Some(menu);
                        self.context_menu = None; // close context menu when AB opens
                    }
                    ActiveBarMsg::CloseMenus => {
                        self.active_bar_menu = None;
                    }
                    ActiveBarMsg::ToggleFilter(filter) => {
                        // Toggle individual filter — keep menu open
                        if self.selection_filters.contains(&filter) {
                            self.selection_filters.remove(&filter);
                        } else {
                            self.selection_filters.insert(filter);
                        }
                        return Task::none();
                    }
                    ActiveBarMsg::ToggleAllFilters => {
                        // Toggle all on/off — keep menu open
                        if self.selection_filters.len()
                            == crate::active_bar::SelectionFilter::ALL.len()
                        {
                            self.selection_filters.clear();
                        } else {
                            self.selection_filters =
                                crate::active_bar::SelectionFilter::ALL.iter().copied().collect();
                        }
                        return Task::none();
                    }
                    ActiveBarMsg::Action(action) => {
                        self.active_bar_menu = None;
                        // Store last-used tool per group
                        let group = match &action {
                            ActiveBarAction::DrawWire
                            | ActiveBarAction::DrawBus
                            | ActiveBarAction::PlaceBusEntry
                            | ActiveBarAction::PlaceNetLabel => Some("wiring"),
                            ActiveBarAction::PlacePowerGND
                            | ActiveBarAction::PlacePowerVCC
                            | ActiveBarAction::PlacePowerPlus12
                            | ActiveBarAction::PlacePowerPlus5
                            | ActiveBarAction::PlacePowerMinus5
                            | ActiveBarAction::PlacePowerArrow
                            | ActiveBarAction::PlacePowerWave
                            | ActiveBarAction::PlacePowerBar
                            | ActiveBarAction::PlacePowerCircle
                            | ActiveBarAction::PlacePowerSignalGND
                            | ActiveBarAction::PlacePowerEarth => Some("power"),
                            ActiveBarAction::PlaceTextString
                            | ActiveBarAction::PlaceTextFrame
                            | ActiveBarAction::PlaceNote => Some("text"),
                            ActiveBarAction::DrawArc
                            | ActiveBarAction::DrawFullCircle
                            | ActiveBarAction::DrawEllipticalArc
                            | ActiveBarAction::DrawEllipse
                            | ActiveBarAction::DrawLine
                            | ActiveBarAction::DrawRectangle
                            | ActiveBarAction::DrawRoundRectangle
                            | ActiveBarAction::DrawPolygon
                            | ActiveBarAction::DrawBezier
                            | ActiveBarAction::PlaceGraphic => Some("shapes"),
                            ActiveBarAction::PlaceSignalHarness
                            | ActiveBarAction::PlaceHarnessConnector
                            | ActiveBarAction::PlaceHarnessEntry => Some("harness"),
                            ActiveBarAction::PlacePort
                            | ActiveBarAction::PlaceOffSheetConnector => Some("port"),
                            ActiveBarAction::PlaceSheetSymbol
                            | ActiveBarAction::PlaceSheetEntry
                            | ActiveBarAction::PlaceDeviceSheetSymbol
                            | ActiveBarAction::PlaceReuseBlock => Some("sheet"),
                            ActiveBarAction::PlaceParameterSet
                            | ActiveBarAction::PlaceNoERC
                            | ActiveBarAction::PlaceDiffPair
                            | ActiveBarAction::PlaceBlanket
                            | ActiveBarAction::PlaceCompileMask => Some("directives"),
                            _ => None,
                        };
                        if let Some(g) = group {
                            self.last_tool.insert(g.to_string(), action.clone());
                        }
                        match action {
                            ActiveBarAction::ToolSelect => {
                                return self
                                    .update(Message::Tool(ToolMessage::SelectTool(Tool::Select)));
                            }
                            ActiveBarAction::DrawWire => {
                                return self
                                    .update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
                            }
                            ActiveBarAction::DrawBus => {
                                return self
                                    .update(Message::Tool(ToolMessage::SelectTool(Tool::Bus)));
                            }
                            ActiveBarAction::PlaceNetLabel => {
                                return self
                                    .update(Message::Tool(ToolMessage::SelectTool(Tool::Label)));
                            }
                            ActiveBarAction::PlaceComponent => {
                                return self.update(Message::Tool(ToolMessage::SelectTool(
                                    Tool::Component,
                                )));
                            }
                            ActiveBarAction::PlaceTextString => {
                                return self
                                    .update(Message::Tool(ToolMessage::SelectTool(Tool::Text)));
                            }
                            ActiveBarAction::DrawLine => {
                                return self
                                    .update(Message::Tool(ToolMessage::SelectTool(Tool::Line)));
                            }
                            ActiveBarAction::DrawRectangle => {
                                return self.update(Message::Tool(ToolMessage::SelectTool(
                                    Tool::Rectangle,
                                )));
                            }
                            ActiveBarAction::DrawFullCircle => {
                                return self
                                    .update(Message::Tool(ToolMessage::SelectTool(Tool::Circle)));
                            }
                            ActiveBarAction::RotateSelection => {
                                return self.update(Message::RotateSelected);
                            }
                            ActiveBarAction::FlipSelectedX => {
                                return self.update(Message::MirrorSelectedX);
                            }
                            ActiveBarAction::FlipSelectedY => {
                                return self.update(Message::MirrorSelectedY);
                            }
                            // Power port placement
                            ActiveBarAction::PlacePowerGND => {
                                self.pending_power = Some(("GND".into(), "power:GND".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = Some("GND".into());
                            }
                            ActiveBarAction::PlacePowerVCC => {
                                self.pending_power = Some(("VCC".into(), "power:VCC".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = Some("VCC".into());
                            }
                            ActiveBarAction::PlacePowerPlus12 => {
                                self.pending_power = Some(("+12V".into(), "power:+12V".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = Some("+12V".into());
                            }
                            ActiveBarAction::PlacePowerPlus5 => {
                                self.pending_power = Some(("+5V".into(), "power:+5V".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = Some("+5V".into());
                            }
                            ActiveBarAction::PlacePowerMinus5 => {
                                self.pending_power = Some(("-5V".into(), "power:-5V".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = Some("-5V".into());
                            }
                            ActiveBarAction::PlacePowerArrow
                            | ActiveBarAction::PlacePowerWave
                            | ActiveBarAction::PlacePowerBar
                            | ActiveBarAction::PlacePowerCircle => {
                                self.pending_power = Some(("PWR".into(), "power:PWR_FLAG".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = Some("PWR".into());
                            }
                            ActiveBarAction::PlacePowerSignalGND => {
                                self.pending_power = Some(("GNDREF".into(), "power:GNDREF".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = Some("GNDREF".into());
                            }
                            ActiveBarAction::PlacePowerEarth => {
                                self.pending_power = Some(("Earth".into(), "power:Earth".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = Some("Earth".into());
                            }
                            // Alignment operations
                            ActiveBarAction::AlignLeft
                            | ActiveBarAction::AlignRight
                            | ActiveBarAction::AlignTop
                            | ActiveBarAction::AlignBottom
                            | ActiveBarAction::AlignHorizontalCenters
                            | ActiveBarAction::AlignVerticalCenters
                            | ActiveBarAction::DistributeHorizontally
                            | ActiveBarAction::DistributeVertically
                            | ActiveBarAction::AlignToGrid => {
                                self.align_selected(&action);
                            }
                            ActiveBarAction::SelectAll => {
                                return self.update(Message::Selection(
                                    selection_message::SelectionMessage::SelectAll,
                                ));
                            }
                            // Port placement (Global Label)
                            ActiveBarAction::PlacePort => {
                                self.current_tool = Tool::Label;
                                self.pending_port = Some((
                                    signex_types::schematic::LabelType::Global,
                                    "bidirectional".to_string(),
                                ));
                                self.canvas.ghost_label =
                                    Some(signex_types::schematic::Label {
                                        uuid: uuid::Uuid::new_v4(),
                                        text: "PORT".to_string(),
                                        position: signex_types::schematic::Point::new(0.0, 0.0),
                                        rotation: 0.0,
                                        label_type: signex_types::schematic::LabelType::Global,
                                        shape: "bidirectional".to_string(),
                                        font_size: 1.27,
                                        justify: signex_types::schematic::HAlign::Left,
                                    });
                            }
                            ActiveBarAction::PlaceOffSheetConnector => {
                                self.current_tool = Tool::Label;
                                self.pending_port = Some((
                                    signex_types::schematic::LabelType::Hierarchical,
                                    String::new(),
                                ));
                                self.canvas.ghost_label =
                                    Some(signex_types::schematic::Label {
                                        uuid: uuid::Uuid::new_v4(),
                                        text: "SHEET".to_string(),
                                        position: signex_types::schematic::Point::new(0.0, 0.0),
                                        rotation: 0.0,
                                        label_type: signex_types::schematic::LabelType::Hierarchical,
                                        shape: String::new(),
                                        font_size: 1.27,
                                        justify: signex_types::schematic::HAlign::Left,
                                    });
                            }
                            // Bus Entry placement
                            ActiveBarAction::PlaceBusEntry => {
                                self.current_tool = Tool::Component;
                                self.pending_power = None; // clear power mode
                            }
                            // Sheet Symbol placement
                            ActiveBarAction::PlaceSheetSymbol
                            | ActiveBarAction::PlaceSheetEntry
                            | ActiveBarAction::PlaceDeviceSheetSymbol
                            | ActiveBarAction::PlaceReuseBlock => {
                                // TODO: implement sheet symbol placement tool
                            }
                            // Not yet implemented — no-op
                            _ => {}
                        }
                    }
                }
                return Task::none();
            }
            // Context menu actions
            Message::ShowContextMenu(x, y) => {
                // Right-click during placement mode: cancel placement (Altium behavior)
                if self.current_tool != Tool::Select {
                    self.clear_transient_schematic_tool_state();
                    self.current_tool = Tool::Select;
                    return Task::none();
                }
                // Don't show context menu if Active Bar dropdown is open
                if self.active_bar_menu.is_none() {
                    self.context_menu = Some(ContextMenuState { x, y });
                }
                return Task::none();
            }
            Message::CloseContextMenu => {
                self.context_menu = None;
                return Task::none();
            }
            Message::ContextAction(action) => {
                self.context_menu = None;
                match action {
                    ContextAction::Copy => return self.update(Message::Copy),
                    ContextAction::Cut => return self.update(Message::Cut),
                    ContextAction::Paste => return self.update(Message::Paste),
                    ContextAction::Delete => return self.update(Message::DeleteSelected),
                    ContextAction::SelectAll => {
                        return self.update(Message::Selection(
                            selection_message::SelectionMessage::SelectAll,
                        ));
                    }
                    ContextAction::ZoomFit => {
                        return self.update(Message::CanvasEvent(CanvasEvent::FitAll));
                    }
                    ContextAction::RotateSelected => return self.update(Message::RotateSelected),
                    ContextAction::MirrorX => return self.update(Message::MirrorSelectedY), // X = horizontal flip = mirror_y
                    ContextAction::MirrorY => return self.update(Message::MirrorSelectedX), // Y = vertical flip = mirror_x
                }
            }
            // Idle events — return early to avoid triggering panel sync/re-render
            Message::Noop => {
                return Task::none();
            }
        }
        if self.current_tool != Tool::Measure
            && (self.canvas.measure_start.is_some() || self.canvas.measure_end.is_some())
        {
            self.clear_measurement();
        }
        // Sync live settings to panel context for Properties panel
        self.panel_ctx.unit = self.unit;
        self.panel_ctx.grid_visible = self.grid_visible;
        self.panel_ctx.snap_enabled = self.snap_enabled;
        self.panel_ctx.grid_size_mm = self.grid_size_mm;
        self.panel_ctx.visible_grid_mm = self.visible_grid_mm;
        self.panel_ctx.snap_hotspots = self.snap_hotspots;
        Task::none()
    }

    fn handle_menu(&mut self, msg: MenuMessage) -> Task<Message> {
        // iced_aw MenuBar manages open/close/hover state — no manual control needed
        match msg {
            // ── File ──
            MenuMessage::OpenProject => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Open Project or Schematic")
                        .add_filter("Signex Project", &["snxprj"])
                        .add_filter("Signex Schematic", &["snxsch"])
                        .add_filter("KiCad Schematic", &["kicad_sch"])
                        .add_filter("KiCad Project", &["kicad_pro"])
                        .add_filter(
                            "All Supported",
                            &["snxprj", "snxsch", "kicad_sch", "kicad_pro"],
                        )
                        .add_filter("All files", &["*"])
                        .pick_file()
                        .await
                        .map(|f| f.path().to_path_buf())
                },
                Message::FileOpened,
            ),
            // ── View ──
            MenuMessage::ZoomFit => {
                if self.has_active_schematic() {
                    self.canvas.fit_to_paper();
                    self.canvas.clear_bg_cache();
                    self.canvas.clear_content_cache();
                } else if self.has_active_pcb() {
                    self.pcb_canvas.fit_to_board();
                    self.pcb_canvas.clear_bg_cache();
                    self.pcb_canvas.clear_content_cache();
                }
                Task::none()
            }
            MenuMessage::ToggleGrid => {
                self.grid_visible = !self.grid_visible;
                self.canvas.grid_visible = self.grid_visible;
                self.pcb_canvas.grid_visible = self.grid_visible;
                self.canvas.clear_bg_cache();
                self.pcb_canvas.clear_bg_cache();
                Task::none()
            }
            MenuMessage::CycleGrid => {
                self.canvas.clear_bg_cache();
                Task::none()
            }
            MenuMessage::OpenProjectsPanel => {
                self.dock
                    .add_panel(crate::dock::PanelPosition::Left, crate::panels::PanelKind::Projects);
                Task::none()
            }
            MenuMessage::OpenComponentsPanel => {
                self.dock
                    .add_panel(crate::dock::PanelPosition::Left, crate::panels::PanelKind::Components);
                Task::none()
            }
            MenuMessage::OpenNavigatorPanel => {
                self.dock
                    .add_panel(crate::dock::PanelPosition::Right, crate::panels::PanelKind::Navigator);
                Task::none()
            }
            MenuMessage::OpenPropertiesPanel => {
                self.dock
                    .add_panel(crate::dock::PanelPosition::Right, crate::panels::PanelKind::Properties);
                Task::none()
            }
            MenuMessage::OpenMessagesPanel => {
                self.dock
                    .add_panel(crate::dock::PanelPosition::Bottom, crate::panels::PanelKind::Messages);
                Task::none()
            }
            MenuMessage::OpenSignalPanel => {
                self.dock
                    .add_panel(crate::dock::PanelPosition::Bottom, crate::panels::PanelKind::Signal);
                Task::none()
            }
            // ── Place ──
            MenuMessage::PlaceWire => {
                self.current_tool = Tool::Wire;
                self.clear_measurement();
                Task::none()
            }
            MenuMessage::PlaceBus => {
                self.current_tool = Tool::Bus;
                self.clear_measurement();
                Task::none()
            }
            MenuMessage::PlaceLabel => {
                self.current_tool = Tool::Label;
                self.clear_measurement();
                Task::none()
            }
            MenuMessage::PlaceComponent => {
                self.current_tool = Tool::Component;
                self.clear_measurement();
                Task::none()
            }
            MenuMessage::Undo => self.update(Message::Undo),
            MenuMessage::Redo => self.update(Message::Redo),
            MenuMessage::Cut => self.update(Message::Cut),
            MenuMessage::Copy => self.update(Message::Copy),
            MenuMessage::Paste => self.update(Message::Paste),
            MenuMessage::Delete => self.update(Message::DeleteSelected),
            MenuMessage::SelectAll => {
                self.update(Message::Selection(selection_message::SelectionMessage::SelectAll))
            }
            MenuMessage::Duplicate => self.update(Message::Duplicate),
            MenuMessage::Save => self.update(Message::SaveFile),
            MenuMessage::SaveAs => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Save Schematic As")
                        .add_filter("Signex Schematic", &["snxsch"])
                        .add_filter("KiCad Schematic", &["kicad_sch"])
                        .save_file()
                        .await
                        .map(|f| f.path().to_path_buf())
                },
                |path| {
                    if let Some(p) = path {
                        Message::SaveFileAs(p)
                    } else {
                        Message::Noop
                    }
                },
            ),
            // ── Stubs (not yet implemented) ──
            MenuMessage::NewProject
            | MenuMessage::ZoomIn
            | MenuMessage::ZoomOut
            | MenuMessage::Annotate
            | MenuMessage::Erc
            | MenuMessage::GenerateBom => Task::none(),
            // ── Preferences ──
            MenuMessage::OpenPreferences => self.update(Message::OpenPreferences),
        }
    }

    fn handle_tab(&mut self, msg: TabMessage) {
        match msg {
            TabMessage::Select(idx) => {
                if idx < self.tabs.len() && idx != self.active_tab {
                    self.park_active_schematic_session();
                    self.active_tab = idx;
                    self.sync_active_tab();
                }
            }
            TabMessage::Close(idx) => {
                if idx < self.tabs.len() {
                    // Skip close if tab has unsaved changes
                    // TODO: show confirmation dialog instead of silently blocking
                    if self.tabs[idx].dirty {
                        eprintln!(
                            "[tab] Close blocked: tab '{}' has unsaved changes",
                            self.tabs[idx].title
                        );
                        return;
                    }
                    if idx == self.active_tab {
                        self.engine = None;
                    }
                    self.tabs.remove(idx);
                    if self.active_tab >= self.tabs.len() && self.active_tab > 0 {
                        self.active_tab -= 1;
                    }
                    self.sync_active_tab();
                }
            }
        }
    }

    fn refresh_panel_ctx(&mut self) {
        // Build sheet info from ProjectData if available
        let sheets: Vec<crate::panels::SheetInfo> = self
            .project_data
            .as_ref()
            .map(|proj| {
                proj.sheets
                    .iter()
                    .map(|s| crate::panels::SheetInfo {
                        name: s.name.clone(),
                        filename: s.filename.clone(),
                        sym_count: s.symbols_count,
                        wire_count: s.wires_count,
                        label_count: s.labels_count,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let project_name = self
            .project_data
            .as_ref()
            .map(|p| p.name.clone())
            .or_else(|| {
                self.project_path
                    .as_ref()
                    .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            });

        let active_schematic_snapshot = self.active_render_snapshot();
        let active_pcb_snapshot = self.active_pcb_snapshot();

        self.panel_ctx = crate::panels::PanelContext {
            project_name,
            project_file: self
                .project_data
                .as_ref()
                .and_then(|p| p.schematic_root.clone())
                .or_else(|| {
                    self.project_path
                        .as_ref()
                        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
                }),
            pcb_file: self.project_data.as_ref().and_then(|p| p.pcb_file.clone()),
            sheets,
            sym_count: active_schematic_snapshot
                .map(|s| s.symbols.len())
                .or_else(|| active_pcb_snapshot.map(|s| s.footprints.len()))
                .unwrap_or(0),
            wire_count: active_schematic_snapshot
                .map(|s| s.wires.len())
                .or_else(|| active_pcb_snapshot.map(|s| s.segments.len()))
                .unwrap_or(0),
            label_count: active_schematic_snapshot
                .map(|s| s.labels.len())
                .or_else(|| active_pcb_snapshot.map(|s| s.texts.len()))
                .unwrap_or(0),
            junction_count: active_schematic_snapshot
                .map(|s| s.junctions.len())
                .or_else(|| active_pcb_snapshot.map(|s| s.vias.len()))
                .unwrap_or(0),
            child_sheets: active_schematic_snapshot
                .map(|s| s.child_sheets.iter().map(|c| c.name.clone()).collect())
                .unwrap_or_default(),
            has_schematic: self.has_active_schematic(),
            has_pcb: self.has_active_pcb(),
            paper_size: active_schematic_snapshot
                .map(|s| s.paper_size.clone())
                .or_else(|| active_pcb_snapshot.map(|s| format!("PCB • {} layers", s.layers.len())))
                .unwrap_or_else(|| "A4".to_string()),
            lib_symbol_count: active_schematic_snapshot
                .map(|s| s.lib_symbols.len())
                .unwrap_or(0),
            lib_symbol_names: active_schematic_snapshot
                .map(|s| s.lib_symbols.keys().cloned().collect())
                .unwrap_or_default(),
            placed_symbols: if let Some(snapshot) = active_schematic_snapshot {
                snapshot
                    .symbols
                    .iter()
                    .map(|sym| {
                        (
                            sym.reference.clone(),
                            sym.value.clone(),
                            sym.footprint.clone(),
                            sym.lib_id.clone(),
                        )
                    })
                    .collect()
            } else {
                active_pcb_snapshot
                    .map(|snapshot| {
                        snapshot
                            .footprints
                            .iter()
                            .map(|fp| {
                                (
                                    fp.reference.clone(),
                                    fp.value.clone(),
                                    fp.footprint_id.clone(),
                                    fp.layer.clone(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            },
            tokens: signex_types::theme::theme_tokens(self.theme_id),
            unit: self.unit,
            grid_visible: self.grid_visible,
            snap_enabled: self.snap_enabled,
            grid_size_mm: self.grid_size_mm,
            visible_grid_mm: self.visible_grid_mm,
            snap_hotspots: self.snap_hotspots,
            ui_font_name: self.ui_font_name.clone(),
            canvas_font_name: self.canvas_font_name.clone(),
            canvas_font_size: self.canvas_font_size,
            canvas_font_bold: self.canvas_font_bold,
            canvas_font_italic: self.canvas_font_italic,
            canvas_font_popup_open: self.panel_ctx.canvas_font_popup_open,
            properties_tab: self.panel_ctx.properties_tab,
            kicad_libraries: self.panel_ctx.kicad_libraries.clone(),
            active_library: self.panel_ctx.active_library.clone(),
            library_symbols: self.panel_ctx.library_symbols.clone(),
            selected_component: self.panel_ctx.selected_component.clone(),
            selected_pins: self.panel_ctx.selected_pins.clone(),
            selected_lib_symbol: self.panel_ctx.selected_lib_symbol.clone(),
            components_split: self.panel_ctx.components_split,
            project_tree: vec![], // built below
            selection_count: self.panel_ctx.selection_count,
            selected_uuid: self.panel_ctx.selected_uuid,
            selected_kind: self.panel_ctx.selected_kind,
            selection_info: self.panel_ctx.selection_info.clone(),
            component_filter: self.panel_ctx.component_filter.clone(),
            collapsed_sections: self.panel_ctx.collapsed_sections.clone(),
            pre_placement: self.panel_ctx.pre_placement.clone(),
        };
        // Build persistent project tree (toggle state preserved until next project load)
        self.panel_ctx.project_tree = crate::panels::build_project_tree(&self.panel_ctx);
    }

    fn sync_active_tab(&mut self) {
        self.sync_visible_document_from_active_tab();
    }

    /// Align selected symbols based on the alignment action.
    fn align_selected(&mut self, action: &crate::active_bar::ActiveBarAction) {
        use crate::active_bar::ActiveBarAction;

        if self.canvas.selected.len() < 2 && !matches!(action, ActiveBarAction::AlignToGrid) {
            return;
        }
        let Some(engine) = self.engine.as_ref() else {
            return;
        };

        let positions = engine.selection_anchors(&self.canvas.selected);

        if positions.is_empty() {
            return;
        }

        let min_x = positions.iter().map(|anchor| anchor.x).fold(f64::INFINITY, f64::min);
        let max_x = positions
            .iter()
            .map(|anchor| anchor.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = positions.iter().map(|anchor| anchor.y).fold(f64::INFINITY, f64::min);
        let max_y = positions
            .iter()
            .map(|anchor| anchor.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;
        let gs = self.grid_size_mm as f64;

        // Compute move delta for each item and create batch undo command
        let mut engine_commands = Vec::new();
        for anchor in &positions {
            let (target_x, target_y) = match action {
                ActiveBarAction::AlignLeft => (min_x, anchor.y),
                ActiveBarAction::AlignRight => (max_x, anchor.y),
                ActiveBarAction::AlignTop => (anchor.x, min_y),
                ActiveBarAction::AlignBottom => (anchor.x, max_y),
                ActiveBarAction::AlignHorizontalCenters => (center_x, anchor.y),
                ActiveBarAction::AlignVerticalCenters => (anchor.x, center_y),
                ActiveBarAction::AlignToGrid => {
                    ((anchor.x / gs).round() * gs, (anchor.y / gs).round() * gs)
                }
                _ => (anchor.x, anchor.y),
            };
            let dx = target_x - anchor.x;
            let dy = target_y - anchor.y;
            if dx.abs() > 0.001 || dy.abs() > 0.001 {
                let items = vec![signex_types::schematic::SelectedItem::new(anchor.uuid, anchor.kind)];
                engine_commands.push(signex_engine::Command::MoveSelection { items, dx, dy });
            }
        }

        // Handle distribute operations
        if matches!(action, ActiveBarAction::DistributeHorizontally | ActiveBarAction::DistributeVertically)
            && positions.len() > 2
        {
            engine_commands.clear();
            let mut sorted = positions.clone();
            let n = sorted.len();
            match action {
                ActiveBarAction::DistributeHorizontally => {
                    sorted.sort_by(|a, b| a.x.total_cmp(&b.x));
                    let step = (max_x - min_x) / (n - 1) as f64;
                    for (i, anchor) in sorted.iter().enumerate() {
                        let target_x = min_x + step * i as f64;
                        let dx = target_x - anchor.x;
                        if dx.abs() > 0.001 {
                            let items = vec![signex_types::schematic::SelectedItem::new(anchor.uuid, anchor.kind)];
                            engine_commands.push(signex_engine::Command::MoveSelection {
                                items,
                                dx,
                                dy: 0.0,
                            });
                        }
                    }
                }
                ActiveBarAction::DistributeVertically => {
                    sorted.sort_by(|a, b| a.y.total_cmp(&b.y));
                    let step = (max_y - min_y) / (n - 1) as f64;
                    for (i, anchor) in sorted.iter().enumerate() {
                        let target_y = min_y + step * i as f64;
                        let dy = target_y - anchor.y;
                        if dy.abs() > 0.001 {
                            let items = vec![signex_types::schematic::SelectedItem::new(anchor.uuid, anchor.kind)];
                            engine_commands.push(signex_engine::Command::MoveSelection {
                                items,
                                dx: 0.0,
                                dy,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        if !engine_commands.is_empty() {
            self.apply_engine_commands(
                engine_commands,
                true,
                false,
            );
        }
    }

    fn update_selection_info(&mut self) {
        let selected = &self.canvas.selected;
        self.panel_ctx.selection_count = selected.len();
        self.panel_ctx.selection_info.clear();
        self.panel_ctx.selected_uuid = None;
        self.panel_ctx.selected_kind = None;

        if selected.len() != 1 {
            if !selected.is_empty() {
                self.panel_ctx
                    .selection_info
                    .push(("Selected".into(), format!("{} items", selected.len())));
            }
            return;
        }

        if let Some(engine) = self.engine.as_ref()
            && let Some(details) = engine.describe_single_selection(selected)
        {
            self.panel_ctx.selected_uuid = Some(details.selected_uuid);
            self.panel_ctx.selected_kind = Some(details.selected_kind);
            self.panel_ctx.selection_info = details.info;
        }
    }

    fn update_canvas_theme(&mut self) {
        let colors = if self.theme_id == ThemeId::Custom {
            self.custom_theme
                .as_ref()
                .map(|c| c.canvas)
                .unwrap_or_else(|| signex_types::theme::canvas_colors(ThemeId::Signex))
        } else {
            signex_types::theme::canvas_colors(self.theme_id)
        };
        self.canvas.set_theme_colors(
            signex_render::colors::to_iced(&colors.background),
            signex_render::colors::to_iced(&colors.grid),
            signex_render::colors::to_iced(&colors.paper),
        );
        self.pcb_canvas.set_theme_colors(
            signex_render::colors::to_iced(&colors.background),
            signex_render::colors::to_iced(&colors.grid),
        );
        self.canvas.canvas_colors = colors;
        self.pcb_canvas.canvas_colors = colors;
        self.canvas.clear_content_cache();
        self.pcb_canvas.clear_content_cache();
    }

    /// Right-click context menu with actions based on current state.
    #[allow(clippy::vec_init_then_push)] // conditional items require push
    fn view_context_menu(&self) -> Element<'_, Message> {
        let mut items: Vec<Element<'_, Message>> = Vec::with_capacity(20);
        // Common items (both empty and selection context)
        items.push(self.ctx_menu_item_disabled("Find Similar Objects...", None));
        items.push(self.ctx_menu_item_disabled("Find Text...", Some("Ctrl+F")));
        items.push(self.ctx_menu_item_disabled("Clear Filter", Some("Shift+C")));
        items.push(self.ctx_menu_sep());
        items.push(self.ctx_menu_item_disabled("Place", Some("\u{25B6}")));
        items.push(self.ctx_menu_item_disabled("Part Actions", Some("\u{25B6}")));
        items.push(self.ctx_menu_item_disabled("Sheet Actions", Some("\u{25B6}")));

        if !self.canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_disabled("References", Some("\u{25B6}")));
            items.push(self.ctx_menu_item_disabled("Align", Some("\u{25B6}")));
            items.push(self.ctx_menu_item_disabled("Unions", Some("\u{25B6}")));
            items.push(self.ctx_menu_item_disabled("Snippets", Some("\u{25B6}")));
        }

        items.push(self.ctx_menu_item_disabled("Cross Probe", None));
        items.push(self.ctx_menu_sep());

        // Edit operations
        // TODO: implement proper Cut (copy+delete)
        items.push(self.ctx_menu_item_kb("Cut", "Ctrl+X", ContextAction::Cut));
        items.push(self.ctx_menu_item_kb("Copy", "Ctrl+C", ContextAction::Copy));
        items.push(self.ctx_menu_item_kb("Paste", "Ctrl+V", ContextAction::Paste));
        items.push(self.ctx_menu_sep());

        if !self.canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_kb("Rotate", "Space", ContextAction::RotateSelected));
            items.push(self.ctx_menu_item_kb("Mirror X", "X", ContextAction::MirrorX));
            items.push(self.ctx_menu_item_kb("Mirror Y", "Y", ContextAction::MirrorY));
            items.push(self.ctx_menu_item_kb("Delete", "Del", ContextAction::Delete));
            items.push(self.ctx_menu_sep());
        }

        items.push(self.ctx_menu_item_disabled("Comment...", None));
        items.push(self.ctx_menu_item_disabled("Pin Mapping...", None));
        items.push(self.ctx_menu_item_disabled("Project Options...", None));
        items.push(self.ctx_menu_item_disabled("Preferences...", None));

        if !self.canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_disabled("Supplier Links...", None));
            items.push(self.ctx_menu_item_disabled("Properties...", None));
        }

        container(column(items).spacing(0).width(Self::CONTEXT_MENU_WIDTH))
            .padding([4, 0])
            .style(crate::styles::context_menu(&self.panel_ctx.tokens))
            .into()
    }

    /// Context menu item with keyboard shortcut on the right.
    fn ctx_menu_item_kb<'a>(
        &self,
        label: &str,
        shortcut: &str,
        action: ContextAction,
    ) -> Element<'a, Message> {
        let text_c = crate::styles::ti(self.panel_ctx.tokens.text);
        let hover_c = crate::styles::ti(self.panel_ctx.tokens.hover);
        iced::widget::button(
            iced::widget::row![
                iced::widget::text(label.to_string())
                    .size(11)
                    .color(text_c),
                iced::widget::Space::new().width(Length::Fill),
                iced::widget::text(shortcut.to_string())
                    .size(10)
                    .color(crate::styles::ti(self.panel_ctx.tokens.text_secondary)),
            ]
            .spacing(12)
            .width(Length::Fill),
        )
        .width(Self::CONTEXT_MENU_WIDTH)
        .padding([4, 12])
        .on_press(Message::ContextAction(action))
        .style(move |_: &iced::Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => {
                    Some(iced::Background::Color(hover_c))
                }
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                border: iced::Border::default(),
                text_color: text_c,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    }

    /// Disabled/placeholder context menu item (no action).
    fn ctx_menu_item_disabled<'a>(&self, label: &str, right: Option<&str>) -> Element<'a, Message> {
        let text_secondary = crate::styles::ti(self.panel_ctx.tokens.text_secondary);
        let mut row = iced::widget::row![
            iced::widget::text(label.to_string())
                .size(11)
                .color(text_secondary),
            iced::widget::Space::new().width(Length::Fill),
        ]
        .spacing(12)
        .width(Length::Fill);

        if let Some(right_text) = right {
            row = row.push(
                iced::widget::text(right_text.to_string())
                    .size(10)
                    .color(text_secondary),
            );
        }

        container(row)
        .padding([4, 12])
        .width(Self::CONTEXT_MENU_WIDTH)
        .into()
    }

    fn ctx_menu_sep<'a>(&self) -> Element<'a, Message> {
        let border_c = crate::styles::ti(self.panel_ctx.tokens.border);
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(1)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(border_c.into()),
                ..container::Style::default()
            })
            .into()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let menu = menu_bar::view(&self.panel_ctx.tokens).map(Message::Menu);

        // Dock regions with collapse-aware sizing
        let left_has_panels = self.dock.has_panels(PanelPosition::Left);
        let right_has_panels = self.dock.has_panels(PanelPosition::Right);
        let bottom_has_panels = self.dock.has_panels(PanelPosition::Bottom);
        let left_collapsed = self.dock.is_collapsed(PanelPosition::Left);
        let right_collapsed = self.dock.is_collapsed(PanelPosition::Right);
        let bottom_collapsed = self.dock.is_collapsed(PanelPosition::Bottom);

        let left = self.view_dock_panel(
            PanelPosition::Left,
            left_has_panels,
            left_collapsed,
            self.left_width,
        );
        let left_handle = self.view_resize_handle(
            DragTarget::LeftPanel,
            left_has_panels && !left_collapsed,
            true,
        );
        let center = self.view_center();
        let right_handle = self.view_resize_handle(
            DragTarget::RightPanel,
            right_has_panels && !right_collapsed,
            true,
        );
        let right = self.view_dock_panel(
            PanelPosition::Right,
            right_has_panels,
            right_collapsed,
            self.right_width,
        );

        let center_row = row![left, left_handle, center, right_handle, right];

        let bottom_handle = self.view_resize_handle(
            DragTarget::BottomPanel,
            bottom_has_panels && !bottom_collapsed,
            false,
        );
        let bottom = self.view_dock_panel_h(
            PanelPosition::Bottom,
            bottom_has_panels,
            bottom_collapsed,
            self.bottom_height,
        );

        let status = status_bar::view(
            self.cursor_x,
            self.cursor_y,
            self.grid_visible,
            self.snap_enabled,
            self.zoom,
            self.unit,
            &self.current_tool,
            self.grid_size_mm,
            &self.panel_ctx.tokens,
        )
        .map(Message::StatusBar);

        // Assemble main column: menu → tabs → canvas row → bottom → status
        // Active Bar is overlaid ON the canvas via Stack (not in column)
        let mut main = column![menu];
        if !self.tabs.is_empty() {
            main = main.push(tab_bar::view(&self.tabs, self.active_tab, &self.panel_ctx.tokens).map(Message::Tab));
        }
        let main = main
            .push(center_row)
            .push(bottom_handle)
            .push(bottom)
            .push(status);

        // Overlay layer — Active Bar floats on canvas, plus menus/context/panels
        let has_active_bar = self.has_active_schematic();
        let needs_overlay = has_active_bar
            || self.editing_text.is_some()
            || self.context_menu.is_some()
            || self.active_bar_menu.is_some()
            || self.panel_list_open
            || self.preferences_open
            || !self.dock.floating.is_empty();

        if needs_overlay {
            let overlays = self.collect_overlays();
            let mut stack = iced::widget::Stack::new().push(main);
            for overlay in overlays {
                stack = stack.push(overlay);
            }
            stack.into()
        } else {
            main.into()
        }
    }

    // ─── View helpers (extracted per iced guide View-Helper pattern) ──

    /// Dock panel wrapped in a resizable container (vertical: width-based).
    fn view_dock_panel(
        &self,
        pos: PanelPosition,
        has_panels: bool,
        collapsed: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let panel = self.dock.view_region(pos, &self.panel_ctx).map(Message::Dock);
        let w = if !has_panels {
            0.0
        } else if collapsed {
            28.0
        } else {
            size
        };
        container(panel)
            .width(w)
            .height(Length::Fill)
            .style(crate::styles::panel_region(&self.panel_ctx.tokens))
            .into()
    }

    /// Dock panel wrapped in a resizable container (horizontal: height-based).
    fn view_dock_panel_h(
        &self,
        pos: PanelPosition,
        has_panels: bool,
        collapsed: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let panel = self.dock.view_region(pos, &self.panel_ctx).map(Message::Dock);
        let h = if !has_panels {
            0.0
        } else if collapsed {
            28.0
        } else {
            size
        };
        container(panel)
            .width(Length::Fill)
            .height(h)
            .style(crate::styles::panel_region(&self.panel_ctx.tokens))
            .into()
    }

    /// Resize handle between panels — hidden when panel is collapsed.
    fn view_resize_handle(
        &self,
        target: DragTarget,
        visible: bool,
        horizontal: bool,
    ) -> Element<'_, Message> {
        let size = if visible { 5 } else { 0 };
        let handle_container = if horizontal {
            container(iced::widget::Space::new())
                .width(size)
                .height(Length::Fill)
                .style(crate::styles::resize_handle(&self.panel_ctx.tokens))
        } else {
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(size)
                .style(crate::styles::resize_handle(&self.panel_ctx.tokens))
        };
        let interaction = if horizontal {
            iced::mouse::Interaction::ResizingHorizontally
        } else {
            iced::mouse::Interaction::ResizingVertically
        };
        iced::widget::mouse_area(handle_container)
            .interaction(interaction)
            .on_press(Message::DragStart(target))
            .into()
    }

    /// Center area — canvas when a schematic is loaded, empty placeholder otherwise.
    fn view_center(&self) -> Element<'_, Message> {
        if self.has_active_schematic() {
            canvas(&self.canvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if self.has_active_pcb() {
            canvas(&self.pcb_canvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            container(
                column![
                    iced::widget::text("No document open")
                        .size(14)
                        .color(crate::styles::ti(self.panel_ctx.tokens.text_secondary)),
                    iced::widget::text("Open a project with File > Open or Ctrl+O")
                        .size(11)
                        .color(crate::styles::ti(self.panel_ctx.tokens.text_secondary)),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(&self.panel_ctx.tokens))
            .into()
        }
    }

    /// Transparent dismiss layer that closes popups on click.
    fn dismiss_layer(on_press: Message) -> Element<'static, Message> {
        iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(on_press)
        .into()
    }

    /// Collect all overlay layers (menus, context menu, panel list, floating panels).
    fn collect_overlays(&self) -> Vec<Element<'_, Message>> {
        let mut layers = Vec::new();

        // Active Bar — floats at top-center of canvas area
        if self.has_active_schematic() {
            // Vertical offset: menu bar height + tab bar if present
            let y_offset: f32 = crate::menu_bar::MENU_BAR_HEIGHT
                + if self.tabs.is_empty() { 0.0 } else { 28.0 };
            let bar = crate::active_bar::view_bar(
                self.current_tool,
                self.draw_mode,
                &self.last_tool,
                &self.panel_ctx.tokens,
            )
            .map(Message::ActiveBar);
            layers.push(
                column![
                    iced::widget::Space::new().height(y_offset + 4.0),
                    container(bar)
                        .width(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Center),
                ]
                .into(),
            );
        }

        // In-place text editing overlay
        if self.has_active_schematic() && let Some(ref edit_state) = self.editing_text {
            let text = edit_state.text.clone();
            layers.push(
                column![
                    iced::widget::Space::new().height(edit_state.screen_y - 12.0),
                    row![
                        iced::widget::Space::new().width(edit_state.screen_x - 4.0),
                        container(
                            iced::widget::text_input("", &text)
                                .on_input(Message::TextEditChanged)
                                .on_submit(Message::TextEditSubmit)
                                .size(13)
                                .padding([4, 6])
                                .width(180),
                        )
                        .style(crate::styles::context_menu(&self.panel_ctx.tokens)),
                    ],
                ]
                .into(),
            );
        }

        // Active Bar dropdown overlay — no width constraint, Iced auto-sizes.
        if let Some(ab_menu) = self.active_bar_menu {
            let dropdown = crate::active_bar::view_dropdown(ab_menu, &self.panel_ctx.tokens, &self.selection_filters).map(Message::ActiveBar);
            let x_off = crate::active_bar::dropdown_x_offset(ab_menu);
            let ab_y: f32 =
                24.0 + 28.0 + if self.tabs.is_empty() { 0.0 } else { 28.0 } + 36.0;
            let bar_w: f32 = crate::active_bar::BAR_WIDTH_PX;
            // Position: center the active bar width, then offset inside it.
            // Use window width so the dropdown is never squeezed.
            let (ww, _) = self.window_size;
            let inner_w = ww;
            let adjusted_x = x_off + (inner_w - bar_w) / 2.0;

            layers.push(Self::dismiss_layer(Message::ActiveBar(
                crate::active_bar::ActiveBarMsg::CloseMenus,
            )));
            layers.push(
                container(column![
                    iced::widget::Space::new().height(ab_y),
                    container(row![iced::widget::Space::new().width(adjusted_x), dropdown])
                        .width(inner_w),
                ])
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .into(),
            );
        }

        // Right-click context menu overlay
        if let Some(ref ctx_menu) = self.context_menu {
            let menu = self.view_context_menu();
            layers.push(Self::dismiss_layer(Message::CloseContextMenu));
            layers.push(
                column![
                    iced::widget::Space::new().height(ctx_menu.y),
                    row![
                        iced::widget::Space::new().width(ctx_menu.x),
                        menu,
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .width(Length::Fill),
                ]
                .into(),
            );
        }

        // Panel list popup (bottom-right)
        if self.panel_list_open {
            let text_c = crate::styles::ti(self.panel_ctx.tokens.text);
            let has_sch = self.panel_ctx.has_schematic;
            let has_pcb = self.panel_ctx.has_pcb;
            let panel_items: Vec<Element<'_, Message>> = crate::panels::ALL_PANELS
                .iter()
                .filter(|&&kind| {
                    if kind.needs_schematic() && !has_sch {
                        return false;
                    }
                    if kind.needs_pcb() && !has_pcb {
                        return false;
                    }
                    true
                })
                .map(|&kind| {
                    iced::widget::button(
                        iced::widget::text(kind.label().to_string())
                            .size(11)
                            .color(text_c),
                    )
                    .padding([4, 12])
                    .width(Length::Fill)
                    .on_press(Message::OpenPanel(kind))
                    .style(crate::styles::menu_item(&self.panel_ctx.tokens))
                    .into()
                })
                .collect();

            let popup = container(
                iced::widget::scrollable(column(panel_items).spacing(0).width(180)).height(300),
            )
            .padding([6, 0])
            .style(crate::styles::context_menu(&self.panel_ctx.tokens));

            layers.push(Self::dismiss_layer(Message::TogglePanelList));
            layers.push(
                container(
                    container(popup)
                        .align_x(iced::alignment::Horizontal::Right)
                        .align_y(iced::alignment::Vertical::Bottom)
                        .padding([15, 10]),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            );
        }

        // Dock-zone highlight: show target region when dragging a floating panel near an edge
        if let Some(fp) = self.dock.floating.iter().find(|fp| fp.dragging) {
            let (ww, wh) = self.window_size;
            let zone = 120.0;
            let cx = fp.x + fp.width / 2.0;
            let cy = fp.y + fp.height / 4.0;
            let zone_style = crate::styles::dock_zone_highlight(&self.panel_ctx.tokens);
            if cx < zone {
                layers.push(
                    container(iced::widget::Space::new())
                        .width(self.left_width)
                        .height(Length::Fill)
                        .style(zone_style)
                        .into(),
                );
            } else if cx > ww - zone {
                layers.push(
                    row![
                        iced::widget::Space::new().width(Length::Fill),
                        container(iced::widget::Space::new())
                            .width(self.right_width)
                            .height(Length::Fill)
                            .style(zone_style),
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                );
            } else if cy > wh - zone {
                layers.push(
                    column![
                        iced::widget::Space::new().height(Length::Fill),
                        container(iced::widget::Space::new())
                            .width(Length::Fill)
                            .height(self.bottom_height)
                            .style(zone_style),
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                );
            }
        }

        // Floating panels — display position clamped so the panel is never
        // squeezed by the layout engine; the real fp.x/fp.y remain unclamped
        // so dock-zone detection works correctly at any drag position.
        let (ww, wh) = self.window_size;
        for i in 0..self.dock.floating.len() {
            if let Some(panel_widget) = self.dock.view_floating_panel(i, &self.panel_ctx) {
                let fp = &self.dock.floating[i];
                // Clamp display X so the panel never loses width
                let max_x = (ww - fp.width).max(0.0);
                let px = fp.x.clamp(0.0, max_x);
                let py = fp.y.clamp(0.0, wh - 40.0).max(0.0);
                layers.push(
                    column![
                        iced::widget::Space::new().height(py),
                        row![
                            iced::widget::Space::new().width(px),
                            panel_widget.map(Message::Dock),
                        ],
                    ]
                    .into(),
                );
            }
        }

        // Preferences dialog — topmost overlay
        if self.preferences_open {
            let pref_view = crate::preferences::view(
                self.preferences_nav,
                self.preferences_draft_theme,
                self.theme_id,
                &self.preferences_draft_font,
                self.preferences_draft_power_port_style,
                self.custom_theme.as_ref().map(|c| c.name.as_str()),
                self.preferences_dirty,
            )
            .map(Message::PreferencesMsg);
            layers.push(pref_view);
        }

        layers
    }
}
