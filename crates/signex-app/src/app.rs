//! Main Iced application — Message enum, update loop, view tree.

use std::path::PathBuf;

use iced::widget::{canvas, column, container, row};
use iced::{Element, Length, Subscription, Task, Theme};
use signex_types::coord::Unit;
use signex_types::project::ProjectData;
use signex_types::schematic::SchematicSheet;
use signex_types::theme::ThemeId;

use crate::canvas::{CanvasEvent, SchematicCanvas};
use crate::dock::{DockArea, DockMessage, PanelPosition};
use crate::menu_bar::{self, MenuMessage};
use crate::panels::PanelKind;
use crate::status_bar;
use crate::tab_bar::{self, TabMessage};
use crate::toolbar::{self, ToolMessage};

/// Find the KiCad symbol library directory.
fn find_kicad_symbols_dir() -> Option<PathBuf> {
    for ver in &["9.0", "8.0", "7.0"] {
        let p = PathBuf::from(format!("C:/Program Files/KiCad/{ver}/share/kicad/symbols"));
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// List .kicad_sym filenames in a directory.
fn list_kicad_libraries(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .ok()
        .map(|entries| {
            let mut names: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "kicad_sym"))
                .map(|e| {
                    e.path()
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                })
                .collect();
            names.sort();
            names
        })
        .unwrap_or_default()
}

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
    SelectAll,
    Copy,
    Paste,
    SaveFile,
    SaveFileAs(PathBuf),
    CycleDrawMode,
    CancelDrawing,
    // Panel list
    TogglePanelList,
    OpenPanel(crate::panels::PanelKind),
    // Active Bar
    ActiveBar(crate::active_bar::ActiveBarMsg),
    // Context menu
    ShowContextMenu(f32, f32),
    CloseContextMenu,
    ContextAction(ContextAction),
    Noop,
}

/// Actions available in the right-click context menu.
#[derive(Debug, Clone)]
#[allow(dead_code)] // variants used in match arms, constructed via dropdown actions
pub enum ContextAction {
    Copy,
    Paste,
    Delete,
    SelectAll,
    ZoomFit,
    RotateSelected,
    MirrorX,
    MirrorY,
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
    pub grid_size_mm: f32,
    pub schematic: Option<SchematicSheet>,
    pub project_path: Option<PathBuf>,
    pub project_data: Option<ProjectData>,
    pub panel_ctx: crate::panels::PanelContext,
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_height: f32,
    pub active_menu: Option<usize>,
    pub kicad_lib_dir: Option<PathBuf>,
    pub loaded_lib: std::collections::HashMap<String, signex_types::schematic::LibSymbol>,
    pub dragging: Option<DragTarget>,
    pub drag_start_pos: Option<f32>,
    pub drag_start_size: f32,
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
    /// Context menu state (shown on right-click).
    pub context_menu: Option<ContextMenuState>,
    /// Last known global mouse position (for context menu positioning).
    pub last_mouse_pos: (f32, f32),
    /// Active Bar open dropdown menu.
    pub active_bar_menu: Option<crate::active_bar::ActiveBarMenu>,
    /// Last-used tool per Active Bar group (Altium behavior: left-click re-activates).
    pub last_tool: std::collections::HashMap<String, crate::active_bar::ActiveBarAction>,
    /// Pending power port placement (net name, lib_id).
    pub pending_power: Option<(String, String)>,
    /// Panel list popup visible.
    pub panel_list_open: bool,
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

#[derive(Debug, Clone)]
pub struct TabInfo {
    pub title: String,
    pub path: PathBuf,
    pub schematic: Option<SchematicSheet>,
    pub dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Wire,
    Bus,
    Label,
    Component,
    Text,
    Line,
    Rectangle,
    Circle,
}

impl std::fmt::Display for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tool::Select => write!(f, "Select"),
            Tool::Wire => write!(f, "Draw Wire"),
            Tool::Bus => write!(f, "Draw Bus"),
            Tool::Label => write!(f, "Place Label"),
            Tool::Component => write!(f, "Place Component"),
            Tool::Text => write!(f, "Place Text"),
            Tool::Line => write!(f, "Draw Line"),
            Tool::Rectangle => write!(f, "Draw Rectangle"),
            Tool::Circle => write!(f, "Draw Circle"),
        }
    }
}

// ─── Draw mode constraint ────────────────────────────────────

/// Given a start and end point, produce wire segments constrained by the draw mode.
/// - Ortho90: horizontal then vertical (two segments forming a 90° corner)
/// - Angle45: snap to nearest 45° angle (may produce one or two segments)
/// - FreeAngle: single straight segment
fn constrain_segments(
    start: signex_types::schematic::Point,
    end: signex_types::schematic::Point,
    mode: DrawMode,
) -> Vec<(
    signex_types::schematic::Point,
    signex_types::schematic::Point,
)> {
    use signex_types::schematic::Point;

    let dx = end.x - start.x;
    let dy = end.y - start.y;

    if dx.abs() < 0.01 && dy.abs() < 0.01 {
        return vec![];
    }

    match mode {
        DrawMode::FreeAngle => {
            vec![(start, end)]
        }
        DrawMode::Ortho90 => {
            // Horizontal first, then vertical (like Altium default)
            if dx.abs() < 0.01 {
                // Pure vertical
                vec![(start, end)]
            } else if dy.abs() < 0.01 {
                // Pure horizontal
                vec![(start, end)]
            } else {
                let corner = Point::new(end.x, start.y);
                vec![(start, corner), (corner, end)]
            }
        }
        DrawMode::Angle45 => {
            // Snap to nearest 45° increment
            let adx = dx.abs();
            let ady = dy.abs();
            if adx < 0.01 || ady < 0.01 {
                // Already axis-aligned
                vec![(start, end)]
            } else if (adx - ady).abs() < adx * 0.4 {
                // Close to 45° — make it exactly 45°
                let d = adx.min(ady);
                let sx = if dx > 0.0 { 1.0 } else { -1.0 };
                let sy = if dy > 0.0 { 1.0 } else { -1.0 };
                let diag_end = Point::new(start.x + d * sx, start.y + d * sy);
                if (adx - ady).abs() < 0.01 {
                    // Exactly 45°
                    vec![(start, diag_end)]
                } else if adx > ady {
                    // Diagonal then horizontal
                    vec![(start, diag_end), (diag_end, Point::new(end.x, diag_end.y))]
                } else {
                    // Diagonal then vertical
                    vec![(start, diag_end), (diag_end, Point::new(diag_end.x, end.y))]
                }
            } else {
                // Mostly axis-aligned — use ortho
                let corner = Point::new(end.x, start.y);
                vec![(start, corner), (corner, end)]
            }
        }
    }
}

// ─── Iced Application ─────────────────────────────────────────

impl Signex {
    pub fn new() -> (Self, Task<Message>) {
        let mut dock = DockArea::new();
        dock.add_panel(PanelPosition::Left, PanelKind::Projects);
        dock.add_panel(PanelPosition::Left, PanelKind::Components);
        dock.add_panel(PanelPosition::Right, PanelKind::Properties);
        dock.add_panel(PanelPosition::Bottom, PanelKind::Messages);
        dock.add_panel(PanelPosition::Bottom, PanelKind::Signal);

        let sch_canvas = SchematicCanvas::new();
        let grid_size_mm = crate::canvas::grid::GRID_SIZES_MM[2]; // 2.54mm
        let kicad_lib_dir = find_kicad_symbols_dir();
        let kicad_libraries = kicad_lib_dir
            .as_deref()
            .map(list_kicad_libraries)
            .unwrap_or_default();

        let app = Self {
            theme_id: ThemeId::AltiumDark,
            unit: Unit::Mm,
            grid_visible: true,
            snap_enabled: true,
            cursor_x: 0.0,
            cursor_y: 0.0,
            zoom: 100.0,
            dock,
            tabs: vec![],
            active_tab: 0,
            current_tool: Tool::Select,
            canvas: sch_canvas,
            grid_size_mm,
            schematic: None,
            project_path: None,
            project_data: None,
            panel_ctx: crate::panels::PanelContext {
                project_name: None,
                project_file: None,
                pcb_file: None,
                sheets: vec![],
                sym_count: 0,
                wire_count: 0,
                label_count: 0,
                junction_count: 0,
                child_sheets: vec![],
                has_schematic: false,
                paper_size: "A4".to_string(),
                lib_symbol_count: 0,
                lib_symbol_names: vec![],
                placed_symbols: vec![],
                tokens: signex_types::theme::theme_tokens(ThemeId::AltiumDark),
                unit: Unit::Mm,
                grid_visible: true,
                snap_enabled: true,
                grid_size_mm: 2.54,
                properties_tab: 0,
                kicad_libraries,
                active_library: None,
                library_symbols: vec![],
                selected_component: None,
                selected_pins: vec![],
                selected_lib_symbol: None,
                components_split: 250.0,
                project_tree: vec![],
                selection_count: 0,
                selection_info: vec![],
                component_filter: String::new(),
                collapsed_sections: std::collections::HashSet::new(),
            },
            left_width: 240.0,
            right_width: 220.0,
            bottom_height: 120.0,
            active_menu: None,
            kicad_lib_dir,
            loaded_lib: std::collections::HashMap::new(),
            dragging: None,
            drag_start_pos: None,
            drag_start_size: 0.0,
            undo_stack: crate::undo::UndoStack::new(100),
            wire_points: Vec::new(),
            wire_drawing: false,
            clipboard_wires: Vec::new(),
            clipboard_buses: Vec::new(),
            clipboard_labels: Vec::new(),
            clipboard_symbols: Vec::new(),
            clipboard_junctions: Vec::new(),
            clipboard_no_connects: Vec::new(),
            clipboard_text_notes: Vec::new(),
            draw_mode: DrawMode::default(),
            context_menu: None,
            last_mouse_pos: (0.0, 0.0),
            active_bar_menu: None,
            last_tool: std::collections::HashMap::new(),
            pending_power: None,
            panel_list_open: false,
        };
        (app, Task::none())
    }

    pub fn title(&self) -> String {
        "Signex".to_string()
    }

    pub fn theme(&self) -> Theme {
        match self.theme_id {
            ThemeId::CatppuccinMocha => Theme::CatppuccinMocha,
            ThemeId::VsCodeDark => Theme::Dark,
            ThemeId::AltiumDark => Theme::custom(
                "Altium Dark".to_string(),
                iced::theme::Palette {
                    background: iced::Color::from_rgb(0.18, 0.18, 0.19),
                    text: iced::Color::from_rgb(0.86, 0.86, 0.86),
                    primary: iced::Color::from_rgb(0.45, 0.45, 0.48),
                    success: iced::Color::from_rgb(0.34, 0.65, 0.29),
                    danger: iced::Color::from_rgb(0.96, 0.31, 0.31),
                    warning: iced::Color::from_rgb(0.91, 0.57, 0.18),
                },
            ),
            ThemeId::GitHubDark => Theme::Dark,
            ThemeId::SolarizedLight => Theme::Light,
            ThemeId::Nord => Theme::Nord,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        use iced::keyboard;

        let kbd = keyboard::listen().map(|event| match event {
            keyboard::Event::KeyPressed {
                key, modifiers: m, ..
            } => match (key.as_ref(), m) {
                (keyboard::Key::Character(c), m) if c == "q" && m.command() => Message::UnitCycled,
                (keyboard::Key::Character(c), m) if c == "g" && !m.command() && !m.shift() => {
                    Message::GridCycle
                }
                (keyboard::Key::Character(c), m) if c == "w" && !m.command() => {
                    Message::Tool(ToolMessage::SelectTool(Tool::Wire))
                }
                (keyboard::Key::Character(c), m) if c == "b" && !m.command() => {
                    Message::Tool(ToolMessage::SelectTool(Tool::Bus))
                }
                (keyboard::Key::Character(c), m) if c == "l" && !m.command() => {
                    Message::Tool(ToolMessage::SelectTool(Tool::Label))
                }
                (keyboard::Key::Character(c), m) if c == "p" && !m.command() => {
                    Message::Tool(ToolMessage::SelectTool(Tool::Component))
                }
                (keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
                    Message::Tool(ToolMessage::SelectTool(Tool::Select))
                }
                (keyboard::Key::Named(keyboard::key::Named::Home), _) => {
                    Message::CanvasEvent(CanvasEvent::FitAll)
                }
                // Delete selected
                (keyboard::Key::Named(keyboard::key::Named::Delete), _) => Message::DeleteSelected,
                // Undo/Redo
                (keyboard::Key::Character(c), m) if c == "z" && m.command() && !m.shift() => {
                    Message::Undo
                }
                (keyboard::Key::Character(c), m) if c == "y" && m.command() => Message::Redo,
                (keyboard::Key::Character(c), m) if c == "z" && m.command() && m.shift() => {
                    Message::Redo
                }
                // Shift+Space: cycle draw mode (90° → 45° → Free)
                (keyboard::Key::Named(keyboard::key::Named::Space), m) if m.shift() => {
                    Message::CycleDrawMode
                }
                // Space: rotate selected symbol (Altium convention)
                (keyboard::Key::Named(keyboard::key::Named::Space), _) => Message::RotateSelected,
                // Mirror: X key = horizontal flip (left-right) = KiCad mirror_y
                //         Y key = vertical flip (top-bottom) = KiCad mirror_x
                (keyboard::Key::Character(c), m) if c == "x" && !m.command() => {
                    Message::MirrorSelectedY // X key = horizontal flip = toggle mirror_y
                }
                (keyboard::Key::Character(c), m) if c == "y" && !m.command() => {
                    Message::MirrorSelectedX // Y key = vertical flip = toggle mirror_x
                }
                // Ctrl+S save
                (keyboard::Key::Character(c), m) if c == "s" && m.command() => Message::SaveFile,
                // Ctrl+A select all
                (keyboard::Key::Character(c), m) if c == "a" && m.command() => Message::SelectAll,
                // Ctrl+C copy
                (keyboard::Key::Character(c), m) if c == "c" && m.command() => Message::Copy,
                // Ctrl+V paste
                (keyboard::Key::Character(c), m) if c == "v" && m.command() => Message::Paste,
                // Shift+Ctrl+G — toggle grid visibility
                (keyboard::Key::Character(c), m) if c == "g" && m.command() && m.shift() => {
                    Message::GridToggle
                }
                _ => Message::Noop,
            },
            _ => Message::Noop,
        });

        // Mouse events for drag-to-resize and global position tracking
        let mouse_sub = iced::event::listen().map(|event| match event {
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                Message::DragMove(position.x, position.y)
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                Message::DragEnd
            }
            // Any click dismisses context menu
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                Message::CloseContextMenu
            }
            _ => Message::Noop,
        });
        Subscription::batch([kbd, mouse_sub])
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
                self.canvas.clear_bg_cache();
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
            }
            Message::DragEnd if self.dragging.is_some() => {
                #[cfg(debug_assertions)]
                eprintln!("[drag] END");
                self.dragging = None;
                self.drag_start_pos = None;
            }
            Message::GridCycle => {
                // Cycle grid and clear cache so it redraws
                self.canvas.clear_bg_cache();
            }
            Message::StatusBar(StatusBarMsg::ToggleSnap) => {
                self.snap_enabled = !self.snap_enabled;
            }
            Message::StatusBar(StatusBarMsg::TogglePanelList) => {
                return self.update(Message::TogglePanelList);
            }
            Message::CanvasEvent(CanvasEvent::CursorAt { x, y, zoom_pct }) => {
                self.cursor_x = x as f64;
                self.cursor_y = y as f64;
                self.zoom = zoom_pct;
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
                            for seg in &segments {
                                let wire = signex_types::schematic::Wire {
                                    uuid: uuid::Uuid::new_v4(),
                                    start: seg.0,
                                    end: seg.1,
                                };
                                if let Some(ref mut sheet) = self.schematic {
                                    let cmd = crate::undo::EditCommand::AddWire(wire);
                                    self.undo_stack.execute(sheet, cmd);
                                    self.canvas.schematic = Some(sheet.clone());
                                    self.canvas.clear_content_cache();
                                    self.mark_dirty();
                                    self.commit_schematic();
                                }
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
                            for seg in &segments {
                                let bus = signex_types::schematic::Bus {
                                    uuid: uuid::Uuid::new_v4(),
                                    start: seg.0,
                                    end: seg.1,
                                };
                                if let Some(ref mut sheet) = self.schematic {
                                    let cmd = crate::undo::EditCommand::AddBus(bus);
                                    self.undo_stack.execute(sheet, cmd);
                                    self.canvas.schematic = Some(sheet.clone());
                                    self.canvas.clear_content_cache();
                                    self.mark_dirty();
                                    self.commit_schematic();
                                }
                            }
                            let end_pt = segments.last().map(|s| s.1).unwrap_or(pt);
                            self.wire_points = vec![end_pt];
                            self.canvas.wire_preview = vec![end_pt];
                        }
                    }
                    Tool::Label => {
                        // Place a net label
                        let label = signex_types::schematic::Label {
                            uuid: uuid::Uuid::new_v4(),
                            text: "NET".to_string(),
                            position: signex_types::schematic::Point::new(wx, wy),
                            rotation: 0.0,
                            label_type: signex_types::schematic::LabelType::Net,
                            shape: String::new(),
                            font_size: 1.27,
                            justify: signex_types::schematic::HAlign::Left,
                        };
                        if let Some(ref mut sheet) = self.schematic {
                            let cmd = crate::undo::EditCommand::AddLabel(label);
                            self.undo_stack.execute(sheet, cmd);
                            self.canvas.schematic = Some(sheet.clone());
                            self.canvas.clear_content_cache();
                            self.mark_dirty();
                            self.commit_schematic();
                        }
                        self.current_tool = Tool::Select;
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
                            if let Some(ref mut sheet) = self.schematic {
                                let cmd = crate::undo::EditCommand::AddSymbol(sym);
                                self.undo_stack.execute(sheet, cmd);
                                self.canvas.schematic = Some(sheet.clone());
                                self.canvas.clear_content_cache();
                                self.mark_dirty();
                                self.commit_schematic();
                            }
                        }
                        // Stay in power placement mode for continuous placement
                    }
                    _ => {
                        // Selection mode: hit-test
                        if let Some(ref sheet) = self.schematic {
                            let hit = signex_render::schematic::hit_test::hit_test(
                                sheet, world_x, world_y,
                            );
                            self.canvas.selected = hit.into_iter().collect();
                            self.canvas.clear_overlay_cache();
                            self.update_selection_info();
                        }
                    }
                }
            }
            Message::CanvasEvent(CanvasEvent::DoubleClicked { .. }) => {
                // Finish wire drawing on double-click
                if self.wire_drawing {
                    self.wire_drawing = false;
                    self.wire_points.clear();
                    self.canvas.wire_preview.clear();
                    self.canvas.drawing_mode = false;
                }
            }
            Message::CanvasEvent(CanvasEvent::BoxSelect { x1, y1, x2, y2 }) => {
                // Select all items within the rectangle
                if let Some(ref sheet) = self.schematic {
                    use signex_types::schematic::{SelectedItem, SelectedKind};
                    let mut selected = Vec::new();
                    // Check symbols
                    for s in &sheet.symbols {
                        let px = s.position.x;
                        let py = s.position.y;
                        if px >= x1 && px <= x2 && py >= y1 && py <= y2 {
                            selected.push(SelectedItem::new(s.uuid, SelectedKind::Symbol));
                        }
                    }
                    // Check wires
                    for w in &sheet.wires {
                        let in_box = |p: &signex_types::schematic::Point| {
                            p.x >= x1 && p.x <= x2 && p.y >= y1 && p.y <= y2
                        };
                        if in_box(&w.start) || in_box(&w.end) {
                            selected.push(SelectedItem::new(w.uuid, SelectedKind::Wire));
                        }
                    }
                    // Check buses
                    for b in &sheet.buses {
                        let in_box = |p: &signex_types::schematic::Point| {
                            p.x >= x1 && p.x <= x2 && p.y >= y1 && p.y <= y2
                        };
                        if in_box(&b.start) || in_box(&b.end) {
                            selected.push(SelectedItem::new(b.uuid, SelectedKind::Bus));
                        }
                    }
                    // Check labels
                    for l in &sheet.labels {
                        if l.position.x >= x1
                            && l.position.x <= x2
                            && l.position.y >= y1
                            && l.position.y <= y2
                        {
                            selected.push(SelectedItem::new(l.uuid, SelectedKind::Label));
                        }
                    }
                    // Check junctions
                    for j in &sheet.junctions {
                        if j.position.x >= x1
                            && j.position.x <= x2
                            && j.position.y >= y1
                            && j.position.y <= y2
                        {
                            selected.push(SelectedItem::new(j.uuid, SelectedKind::Junction));
                        }
                    }
                    // Check no-connects
                    for nc in &sheet.no_connects {
                        if nc.position.x >= x1
                            && nc.position.x <= x2
                            && nc.position.y >= y1
                            && nc.position.y <= y2
                        {
                            selected.push(SelectedItem::new(nc.uuid, SelectedKind::NoConnect));
                        }
                    }
                    // Check text notes
                    for tn in &sheet.text_notes {
                        if tn.position.x >= x1
                            && tn.position.x <= x2
                            && tn.position.y >= y1
                            && tn.position.y <= y2
                        {
                            selected.push(SelectedItem::new(tn.uuid, SelectedKind::TextNote));
                        }
                    }
                    self.canvas.selected = selected;
                    self.canvas.clear_overlay_cache();
                    self.update_selection_info();
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
                // Zoom or pan changed — grid + schematic positions shifted, must redraw
                self.canvas.clear_bg_cache();
                self.canvas.clear_content_cache();
                self.canvas.clear_overlay_cache();
                // Clear pending fit after it's been applied
                self.canvas.pending_fit.set(None);
            }
            Message::CanvasEvent(CanvasEvent::FitAll) => {
                // Fit-all is handled in the canvas state, but we can't access it
                // from here. Instead, reset the canvas camera to default fit.
                // A proper implementation would read canvas bounds, but for now
                // we reset to a reasonable default that shows an A4 sheet.
                self.canvas.fit_to_paper();
                self.canvas.clear_bg_cache();
                self.canvas.clear_content_cache();
            }
            Message::Tool(ToolMessage::SelectTool(tool)) => {
                self.current_tool = tool;
                // Set tool preview text for placement modes
                // No cursor text — Active Bar shows the active tool
                self.canvas.tool_preview = None;
                // Escape: close menus and cancel wire drawing
                if tool == Tool::Select {
                    self.active_menu = None;
                    self.pending_power = None;
                    if self.wire_drawing {
                        self.wire_drawing = false;
                        self.wire_points.clear();
                        self.canvas.wire_preview.clear();
                        self.canvas.drawing_mode = false;
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
                        self.canvas.clear_bg_cache();
                    }
                    crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleSnap) => {
                        self.snap_enabled = !self.snap_enabled;
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
                                if file_path.exists()
                                    && (filename.ends_with(".kicad_sch")
                                        || filename.ends_with(".snxsch"))
                                {
                                    // Already open? Switch to it
                                    if let Some(idx) =
                                        self.tabs.iter().position(|t| t.path == file_path)
                                    {
                                        self.active_tab = idx;
                                        self.sync_active_tab();
                                    } else {
                                        // Open new tab
                                        match kicad_parser::parse_schematic_file(&file_path) {
                                            Ok(sheet) => {
                                                self.tabs.push(TabInfo {
                                                    title: filename.replace(".kicad_sch", ""),
                                                    path: file_path,
                                                    schematic: Some(sheet),
                                                    dirty: false,
                                                });
                                                self.active_tab = self.tabs.len() - 1;
                                                self.sync_active_tab();
                                            }
                                            Err(e) => eprintln!("Failed to parse {filename}: {e}"),
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
                self.dock.update(msg);
            }
            Message::FileOpened(Some(path)) => {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                match ext {
                    "kicad_pro" | "snxprj" => {
                        // Parse project file — discovers all sheets
                        match kicad_parser::parse_project(&path) {
                            Ok(proj) => {
                                self.project_path = Some(path.clone());
                                self.project_data = Some(proj.clone());
                                // Don't auto-load any schematic — user clicks in project tree to open
                                self.refresh_panel_ctx();
                            }
                            Err(e) => eprintln!("Failed to parse project: {e}"),
                        }
                    }
                    "kicad_sch" | "snxsch" => {
                        // Direct schematic open — also try to find the .kicad_pro
                        match kicad_parser::parse_schematic_file(&path) {
                            Ok(sheet) => {
                                self.project_path = Some(path.clone());
                                // Try to find and parse the .kicad_pro in the same directory
                                if let Some(dir) = path.parent() {
                                    let stem =
                                        path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                    let pro_path = dir.join(format!("{stem}.kicad_pro"));
                                    if pro_path.exists()
                                        && let Ok(proj) = kicad_parser::parse_project(&pro_path)
                                    {
                                        self.project_data = Some(proj);
                                    }
                                }
                                let title = path
                                    .file_stem()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "Schematic".to_string());
                                self.tabs.push(TabInfo {
                                    title,
                                    path: path.clone(),
                                    schematic: Some(sheet.clone()),
                                    dirty: false,
                                });
                                self.active_tab = self.tabs.len() - 1;
                                self.schematic = Some(sheet.clone());
                                self.canvas.schematic = Some(sheet);
                                self.canvas.fit_to_paper();
                                self.canvas.clear_bg_cache();
                                self.canvas.clear_content_cache();
                                self.refresh_panel_ctx();
                            }
                            Err(e) => eprintln!("Failed to parse schematic: {e}"),
                        }
                    }
                    _ => {
                        eprintln!("Unsupported file type: .{ext}");
                    }
                }
            }
            Message::FileOpened(None) => {
                // User cancelled file dialog
            }
            Message::DeleteSelected => {
                if !self.canvas.selected.is_empty()
                    && let Some(ref mut sheet) = self.schematic
                {
                    let mut cmds = Vec::new();
                    for item in &self.canvas.selected {
                        use signex_types::schematic::SelectedKind;
                        match item.kind {
                            SelectedKind::Wire => {
                                if let Some(w) = sheet.wires.iter().find(|w| w.uuid == item.uuid) {
                                    cmds.push(crate::undo::EditCommand::RemoveWire(w.clone()));
                                }
                            }
                            SelectedKind::Bus => {
                                if let Some(b) = sheet.buses.iter().find(|b| b.uuid == item.uuid) {
                                    cmds.push(crate::undo::EditCommand::RemoveBus(b.clone()));
                                }
                            }
                            SelectedKind::Label => {
                                if let Some(l) = sheet.labels.iter().find(|l| l.uuid == item.uuid) {
                                    cmds.push(crate::undo::EditCommand::RemoveLabel(l.clone()));
                                }
                            }
                            SelectedKind::Junction => {
                                if let Some(j) =
                                    sheet.junctions.iter().find(|j| j.uuid == item.uuid)
                                {
                                    cmds.push(crate::undo::EditCommand::RemoveJunction(j.clone()));
                                }
                            }
                            SelectedKind::NoConnect => {
                                if let Some(nc) =
                                    sheet.no_connects.iter().find(|n| n.uuid == item.uuid)
                                {
                                    cmds.push(crate::undo::EditCommand::RemoveNoConnect(
                                        nc.clone(),
                                    ));
                                }
                            }
                            SelectedKind::Symbol => {
                                if let Some(s) = sheet.symbols.iter().find(|s| s.uuid == item.uuid)
                                {
                                    cmds.push(crate::undo::EditCommand::RemoveSymbol(s.clone()));
                                }
                            }
                            SelectedKind::TextNote => {
                                if let Some(tn) =
                                    sheet.text_notes.iter().find(|t| t.uuid == item.uuid)
                                {
                                    cmds.push(crate::undo::EditCommand::RemoveTextNote(tn.clone()));
                                }
                            }
                            _ => {}
                        }
                    }
                    if !cmds.is_empty() {
                        let batch = crate::undo::EditCommand::Batch(cmds);
                        self.undo_stack.execute(sheet, batch);
                        self.canvas.schematic = Some(sheet.clone());
                        self.canvas.selected.clear();
                        self.canvas.clear_content_cache();
                        self.canvas.clear_overlay_cache();
                        self.mark_dirty();
                        self.commit_schematic();
                        self.update_selection_info();
                    }
                }
            }
            Message::Undo => {
                if let Some(ref mut sheet) = self.schematic
                    && self.undo_stack.undo(sheet)
                {
                    self.canvas.schematic = Some(sheet.clone());
                    self.canvas.selected.clear();
                    self.canvas.clear_content_cache();
                    self.canvas.clear_overlay_cache();
                    self.mark_dirty();
                    self.commit_schematic();
                    self.update_selection_info();
                }
            }
            Message::Redo => {
                if let Some(ref mut sheet) = self.schematic
                    && self.undo_stack.redo(sheet)
                {
                    self.canvas.schematic = Some(sheet.clone());
                    self.canvas.selected.clear();
                    self.canvas.clear_content_cache();
                    self.canvas.clear_overlay_cache();
                    self.mark_dirty();
                    self.commit_schematic();
                    self.update_selection_info();
                }
            }
            Message::RotateSelected => {
                if self.canvas.selected.len() == 1 {
                    let item = self.canvas.selected[0];
                    if item.kind == signex_types::schematic::SelectedKind::Symbol
                        && let Some(ref mut sheet) = self.schematic
                        && let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid)
                    {
                        let old_rotation = sym.rotation;
                        let new_rotation = (old_rotation + 90.0) % 360.0;
                        let cmd = crate::undo::EditCommand::RotateSymbol {
                            uuid: item.uuid,
                            old_rotation,
                            new_rotation,
                        };
                        self.undo_stack.execute(sheet, cmd);
                        self.canvas.schematic = Some(sheet.clone());
                        self.canvas.clear_content_cache();
                        self.canvas.clear_overlay_cache();
                        self.mark_dirty();
                        self.commit_schematic();
                        self.update_selection_info();
                    }
                }
            }
            Message::MirrorSelectedX => {
                if self.canvas.selected.len() == 1 {
                    let item = self.canvas.selected[0];
                    if item.kind == signex_types::schematic::SelectedKind::Symbol
                        && let Some(ref mut sheet) = self.schematic
                        && let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid)
                    {
                        let cmd = crate::undo::EditCommand::MirrorSymbol {
                            uuid: item.uuid,
                            axis: crate::undo::MirrorAxis::X,
                            old_mirror_x: sym.mirror_x,
                            old_mirror_y: sym.mirror_y,
                        };
                        self.undo_stack.execute(sheet, cmd);
                        self.canvas.schematic = Some(sheet.clone());
                        self.canvas.clear_content_cache();
                        self.canvas.clear_overlay_cache();
                        self.mark_dirty();
                        self.commit_schematic();
                        self.update_selection_info();
                    }
                }
            }
            Message::MirrorSelectedY => {
                if self.canvas.selected.len() == 1 {
                    let item = self.canvas.selected[0];
                    if item.kind == signex_types::schematic::SelectedKind::Symbol
                        && let Some(ref mut sheet) = self.schematic
                        && let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid)
                    {
                        let cmd = crate::undo::EditCommand::MirrorSymbol {
                            uuid: item.uuid,
                            axis: crate::undo::MirrorAxis::Y,
                            old_mirror_x: sym.mirror_x,
                            old_mirror_y: sym.mirror_y,
                        };
                        self.undo_stack.execute(sheet, cmd);
                        self.canvas.schematic = Some(sheet.clone());
                        self.canvas.clear_content_cache();
                        self.canvas.clear_overlay_cache();
                        self.mark_dirty();
                        self.commit_schematic();
                        self.update_selection_info();
                    }
                }
            }
            Message::CanvasEvent(CanvasEvent::CtrlClicked { world_x, world_y }) => {
                // Ctrl+click: toggle selection (multi-select)
                if let Some(ref sheet) = self.schematic
                    && let Some(hit) =
                        signex_render::schematic::hit_test::hit_test(sheet, world_x, world_y)
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
            Message::SelectAll => {
                if let Some(ref sheet) = self.schematic {
                    use signex_types::schematic::{SelectedItem, SelectedKind};
                    let mut all = Vec::new();
                    for s in &sheet.symbols {
                        all.push(SelectedItem::new(s.uuid, SelectedKind::Symbol));
                    }
                    for w in &sheet.wires {
                        all.push(SelectedItem::new(w.uuid, SelectedKind::Wire));
                    }
                    for b in &sheet.buses {
                        all.push(SelectedItem::new(b.uuid, SelectedKind::Bus));
                    }
                    for l in &sheet.labels {
                        all.push(SelectedItem::new(l.uuid, SelectedKind::Label));
                    }
                    for j in &sheet.junctions {
                        all.push(SelectedItem::new(j.uuid, SelectedKind::Junction));
                    }
                    for nc in &sheet.no_connects {
                        all.push(SelectedItem::new(nc.uuid, SelectedKind::NoConnect));
                    }
                    for tn in &sheet.text_notes {
                        all.push(SelectedItem::new(tn.uuid, SelectedKind::TextNote));
                    }
                    for cs in &sheet.child_sheets {
                        all.push(SelectedItem::new(cs.uuid, SelectedKind::ChildSheet));
                    }
                    self.canvas.selected = all;
                    self.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            Message::Copy => {
                if let Some(ref sheet) = self.schematic {
                    self.clipboard_wires.clear();
                    self.clipboard_buses.clear();
                    self.clipboard_labels.clear();
                    self.clipboard_symbols.clear();
                    self.clipboard_junctions.clear();
                    self.clipboard_no_connects.clear();
                    self.clipboard_text_notes.clear();
                    for item in &self.canvas.selected {
                        use signex_types::schematic::SelectedKind;
                        match item.kind {
                            SelectedKind::Wire => {
                                if let Some(w) = sheet.wires.iter().find(|w| w.uuid == item.uuid) {
                                    self.clipboard_wires.push(w.clone());
                                }
                            }
                            SelectedKind::Bus => {
                                if let Some(b) = sheet.buses.iter().find(|b| b.uuid == item.uuid) {
                                    self.clipboard_buses.push(b.clone());
                                }
                            }
                            SelectedKind::Label => {
                                if let Some(l) = sheet.labels.iter().find(|l| l.uuid == item.uuid) {
                                    self.clipboard_labels.push(l.clone());
                                }
                            }
                            SelectedKind::Symbol => {
                                if let Some(s) = sheet.symbols.iter().find(|s| s.uuid == item.uuid)
                                {
                                    self.clipboard_symbols.push(s.clone());
                                }
                            }
                            SelectedKind::Junction => {
                                if let Some(j) =
                                    sheet.junctions.iter().find(|j| j.uuid == item.uuid)
                                {
                                    self.clipboard_junctions.push(j.clone());
                                }
                            }
                            SelectedKind::NoConnect => {
                                if let Some(nc) =
                                    sheet.no_connects.iter().find(|n| n.uuid == item.uuid)
                                {
                                    self.clipboard_no_connects.push(nc.clone());
                                }
                            }
                            SelectedKind::TextNote => {
                                if let Some(tn) =
                                    sheet.text_notes.iter().find(|t| t.uuid == item.uuid)
                                {
                                    self.clipboard_text_notes.push(tn.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Message::Paste => {
                if let Some(ref mut sheet) = self.schematic {
                    let offset = 5.08; // Paste offset in mm (2 grid units)
                    let mut cmds = Vec::new();
                    for w in &self.clipboard_wires {
                        let mut nw = w.clone();
                        nw.uuid = uuid::Uuid::new_v4();
                        nw.start.x += offset;
                        nw.start.y += offset;
                        nw.end.x += offset;
                        nw.end.y += offset;
                        cmds.push(crate::undo::EditCommand::AddWire(nw));
                    }
                    for b in &self.clipboard_buses {
                        let mut nb = b.clone();
                        nb.uuid = uuid::Uuid::new_v4();
                        nb.start.x += offset;
                        nb.start.y += offset;
                        nb.end.x += offset;
                        nb.end.y += offset;
                        cmds.push(crate::undo::EditCommand::AddBus(nb));
                    }
                    for l in &self.clipboard_labels {
                        let mut nl = l.clone();
                        nl.uuid = uuid::Uuid::new_v4();
                        nl.position.x += offset;
                        nl.position.y += offset;
                        cmds.push(crate::undo::EditCommand::AddLabel(nl));
                    }
                    for s in &self.clipboard_symbols {
                        let mut ns = s.clone();
                        ns.uuid = uuid::Uuid::new_v4();
                        ns.position.x += offset;
                        ns.position.y += offset;
                        if let Some(ref mut rt) = ns.ref_text {
                            rt.position.x += offset;
                            rt.position.y += offset;
                        }
                        if let Some(ref mut vt) = ns.val_text {
                            vt.position.x += offset;
                            vt.position.y += offset;
                        }
                        cmds.push(crate::undo::EditCommand::AddSymbol(ns));
                    }
                    for j in &self.clipboard_junctions {
                        let mut nj = j.clone();
                        nj.uuid = uuid::Uuid::new_v4();
                        nj.position.x += offset;
                        nj.position.y += offset;
                        cmds.push(crate::undo::EditCommand::AddJunction(nj));
                    }
                    for nc in &self.clipboard_no_connects {
                        let mut nnc = nc.clone();
                        nnc.uuid = uuid::Uuid::new_v4();
                        nnc.position.x += offset;
                        nnc.position.y += offset;
                        cmds.push(crate::undo::EditCommand::AddNoConnect(nnc));
                    }
                    for tn in &self.clipboard_text_notes {
                        let mut ntn = tn.clone();
                        ntn.uuid = uuid::Uuid::new_v4();
                        ntn.position.x += offset;
                        ntn.position.y += offset;
                        cmds.push(crate::undo::EditCommand::AddTextNote(ntn));
                    }
                    if !cmds.is_empty() {
                        let batch = crate::undo::EditCommand::Batch(cmds);
                        self.undo_stack.execute(sheet, batch);
                        self.canvas.schematic = Some(sheet.clone());
                        self.canvas.clear_content_cache();
                        self.mark_dirty();
                        self.commit_schematic();
                    }
                }
            }
            Message::SaveFile => {
                if let Some(ref sheet) = self.schematic
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                {
                    let content = kicad_writer::write_schematic(sheet);
                    match std::fs::write(&tab.path, &content) {
                        Ok(_) => {
                            tab.dirty = false;
                            #[cfg(debug_assertions)]
                            eprintln!("[save] Wrote {}", tab.path.display());
                        }
                        Err(e) => {
                            eprintln!("[save] Error: {e}");
                            // TODO: show error in status bar / modal
                        }
                    }
                }
            }
            Message::SaveFileAs(path) => {
                if let Some(ref sheet) = self.schematic {
                    let content = kicad_writer::write_schematic(sheet);
                    match std::fs::write(&path, &content) {
                        Ok(_) => {
                            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                                tab.path = path.clone();
                                tab.title = path
                                    .file_stem()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "Schematic".to_string());
                                tab.dirty = false;
                            }
                            #[cfg(debug_assertions)]
                            eprintln!("[save-as] Wrote {}", path.display());
                        }
                        Err(e) => {
                            eprintln!("[save-as] Error: {e}");
                            // TODO: show error in status bar / modal
                        }
                    }
                }
            }
            Message::SchematicLoaded(sheet) => {
                self.schematic = Some(*sheet);
                self.canvas.schematic = self.schematic.clone();
                self.canvas.clear_content_cache();
                self.commit_schematic();
            }
            // Panel list
            Message::TogglePanelList => {
                self.panel_list_open = !self.panel_list_open;
                return Task::none();
            }
            Message::OpenPanel(kind) => {
                self.panel_list_open = false;
                // Add to right panel dock by default
                self.dock.add_panel(crate::dock::PanelPosition::Right, kind);
                return Task::none();
            }
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
                                self.canvas.tool_preview = None;
                            }
                            ActiveBarAction::PlacePowerVCC => {
                                self.pending_power = Some(("VCC".into(), "power:VCC".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = None;
                            }
                            ActiveBarAction::PlacePowerPlus12 => {
                                self.pending_power = Some(("+12V".into(), "power:+12V".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = None;
                            }
                            ActiveBarAction::PlacePowerPlus5 => {
                                self.pending_power = Some(("+5V".into(), "power:+5V".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = None;
                            }
                            ActiveBarAction::PlacePowerMinus5 => {
                                self.pending_power = Some(("-5V".into(), "power:-5V".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = None;
                            }
                            ActiveBarAction::PlacePowerArrow
                            | ActiveBarAction::PlacePowerWave
                            | ActiveBarAction::PlacePowerBar
                            | ActiveBarAction::PlacePowerCircle => {
                                self.pending_power = Some(("PWR".into(), "power:PWR_FLAG".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = None;
                            }
                            ActiveBarAction::PlacePowerSignalGND => {
                                self.pending_power = Some(("GNDREF".into(), "power:GNDREF".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = None;
                            }
                            ActiveBarAction::PlacePowerEarth => {
                                self.pending_power = Some(("Earth".into(), "power:Earth".into()));
                                self.current_tool = Tool::Component;
                                self.canvas.tool_preview = None;
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
                                return self.update(Message::SelectAll);
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
                    ContextAction::Paste => return self.update(Message::Paste),
                    ContextAction::Delete => return self.update(Message::DeleteSelected),
                    ContextAction::SelectAll => return self.update(Message::SelectAll),
                    ContextAction::ZoomFit => {
                        return self.update(Message::CanvasEvent(CanvasEvent::FitAll));
                    }
                    ContextAction::RotateSelected => return self.update(Message::RotateSelected),
                    ContextAction::MirrorX => return self.update(Message::MirrorSelectedY), // X = horizontal flip = mirror_y
                    ContextAction::MirrorY => return self.update(Message::MirrorSelectedX), // Y = vertical flip = mirror_x
                }
            }
            // Idle events — return early to avoid triggering panel sync/re-render
            Message::DragEnd | Message::Noop => {
                return Task::none();
            }
        }
        // Sync live settings to panel context for Properties panel
        self.panel_ctx.unit = self.unit;
        self.panel_ctx.grid_visible = self.grid_visible;
        self.panel_ctx.snap_enabled = self.snap_enabled;
        self.panel_ctx.grid_size_mm = self.grid_size_mm;
        Task::none()
    }

    fn handle_menu(&mut self, msg: MenuMessage) -> Task<Message> {
        // Close dropdown after any action (except menu-control and theme)
        let should_close = !matches!(
            msg,
            MenuMessage::OpenMenu(_)
                | MenuMessage::CloseMenus
                | MenuMessage::HoverMenu(_)
                | MenuMessage::ThemeSelected(_)
        );

        let task = match msg {
            // ── Menu bar control ──
            MenuMessage::OpenMenu(idx) => {
                self.active_menu = if self.active_menu == Some(idx) {
                    None
                } else {
                    Some(idx)
                };
                Task::none()
            }
            MenuMessage::CloseMenus => {
                self.active_menu = None;
                Task::none()
            }
            MenuMessage::HoverMenu(idx) => {
                if self.active_menu.is_some() {
                    self.active_menu = Some(idx);
                }
                Task::none()
            }
            // ── Theme ──
            MenuMessage::ThemeSelected(id) => {
                self.theme_id = id;
                self.update_canvas_theme();
                Task::none()
            }
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
                self.canvas.fit_to_paper();
                self.canvas.clear_bg_cache();
                self.canvas.clear_content_cache();
                Task::none()
            }
            MenuMessage::ToggleGrid => {
                self.grid_visible = !self.grid_visible;
                self.canvas.grid_visible = self.grid_visible;
                self.canvas.clear_bg_cache();
                Task::none()
            }
            MenuMessage::CycleGrid => {
                self.canvas.clear_bg_cache();
                Task::none()
            }
            // ── Place ──
            MenuMessage::PlaceWire => {
                self.current_tool = Tool::Wire;
                Task::none()
            }
            MenuMessage::PlaceBus => {
                self.current_tool = Tool::Bus;
                Task::none()
            }
            MenuMessage::PlaceLabel => {
                self.current_tool = Tool::Label;
                Task::none()
            }
            MenuMessage::PlaceComponent => {
                self.current_tool = Tool::Component;
                Task::none()
            }
            MenuMessage::Undo => {
                return self.update(Message::Undo);
            }
            MenuMessage::Redo => {
                return self.update(Message::Redo);
            }
            MenuMessage::Save => {
                return self.update(Message::SaveFile);
            }
            MenuMessage::SaveAs => {
                return Task::perform(
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
                );
            }
            // ── Stubs (not yet implemented) ──
            MenuMessage::NewProject
            | MenuMessage::ZoomIn
            | MenuMessage::ZoomOut
            | MenuMessage::Annotate
            | MenuMessage::Erc
            | MenuMessage::GenerateBom => Task::none(),
        };

        if should_close {
            self.active_menu = None;
        }
        task
    }

    fn handle_tab(&mut self, msg: TabMessage) {
        match msg {
            TabMessage::Select(idx) => {
                if idx < self.tabs.len() {
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
            sym_count: self
                .schematic
                .as_ref()
                .map(|s| s.symbols.len())
                .unwrap_or(0),
            wire_count: self.schematic.as_ref().map(|s| s.wires.len()).unwrap_or(0),
            label_count: self.schematic.as_ref().map(|s| s.labels.len()).unwrap_or(0),
            junction_count: self
                .schematic
                .as_ref()
                .map(|s| s.junctions.len())
                .unwrap_or(0),
            child_sheets: self
                .schematic
                .as_ref()
                .map(|s| s.child_sheets.iter().map(|c| c.name.clone()).collect())
                .unwrap_or_default(),
            has_schematic: self.schematic.is_some(),
            paper_size: self
                .schematic
                .as_ref()
                .map(|s| s.paper_size.clone())
                .unwrap_or_else(|| "A4".to_string()),
            lib_symbol_count: self
                .schematic
                .as_ref()
                .map(|s| s.lib_symbols.len())
                .unwrap_or(0),
            lib_symbol_names: self
                .schematic
                .as_ref()
                .map(|s| s.lib_symbols.keys().cloned().collect())
                .unwrap_or_default(),
            placed_symbols: self
                .schematic
                .as_ref()
                .map(|s| {
                    s.symbols
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
                })
                .unwrap_or_default(),
            tokens: signex_types::theme::theme_tokens(self.theme_id),
            unit: self.unit,
            grid_visible: self.grid_visible,
            snap_enabled: self.snap_enabled,
            grid_size_mm: self.grid_size_mm,
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
            selection_info: self.panel_ctx.selection_info.clone(),
            component_filter: self.panel_ctx.component_filter.clone(),
            collapsed_sections: self.panel_ctx.collapsed_sections.clone(),
        };
        // Build persistent project tree (toggle state preserved until next project load)
        self.panel_ctx.project_tree = crate::panels::build_project_tree(&self.panel_ctx);
    }

    fn sync_active_tab(&mut self) {
        if let Some(tab) = self.tabs.get(self.active_tab) {
            self.schematic = tab.schematic.clone();
            self.canvas.schematic = tab.schematic.clone();
        } else {
            self.schematic = None;
            self.canvas.schematic = None;
        }
        self.canvas.clear_bg_cache();
        self.canvas.clear_content_cache();

        // Update panel stats from current schematic (preserve tree collapse state)
        self.panel_ctx.has_schematic = self.schematic.is_some();
        self.panel_ctx.sym_count = self
            .schematic
            .as_ref()
            .map(|s| s.symbols.len())
            .unwrap_or(0);
        self.panel_ctx.wire_count = self.schematic.as_ref().map(|s| s.wires.len()).unwrap_or(0);
        self.panel_ctx.label_count = self.schematic.as_ref().map(|s| s.labels.len()).unwrap_or(0);
        self.panel_ctx.junction_count = self
            .schematic
            .as_ref()
            .map(|s| s.junctions.len())
            .unwrap_or(0);
        self.panel_ctx.lib_symbol_count = self
            .schematic
            .as_ref()
            .map(|s| s.lib_symbols.len())
            .unwrap_or(0);
        self.panel_ctx.lib_symbol_names = self
            .schematic
            .as_ref()
            .map(|s| s.lib_symbols.keys().cloned().collect())
            .unwrap_or_default();
        self.panel_ctx.placed_symbols = self
            .schematic
            .as_ref()
            .map(|s| {
                s.symbols
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
            })
            .unwrap_or_default();
        self.panel_ctx.paper_size = self
            .schematic
            .as_ref()
            .map(|s| s.paper_size.clone())
            .unwrap_or_else(|| "A4".to_string());
    }

    /// Align selected symbols based on the alignment action.
    fn align_selected(&mut self, action: &crate::active_bar::ActiveBarAction) {
        use crate::active_bar::ActiveBarAction;
        if self.canvas.selected.len() < 2 && !matches!(action, ActiveBarAction::AlignToGrid) {
            return;
        }
        let Some(ref mut sheet) = self.schematic else {
            return;
        };
        let selected_uuids: Vec<uuid::Uuid> = self.canvas.selected.iter().map(|s| s.uuid).collect();

        // Gather positions of selected symbols
        let positions: Vec<(uuid::Uuid, f64, f64)> = sheet
            .symbols
            .iter()
            .filter(|s| selected_uuids.contains(&s.uuid))
            .map(|s| (s.uuid, s.position.x, s.position.y))
            .collect();

        if positions.is_empty() {
            return;
        }

        let min_x = positions.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let max_x = positions
            .iter()
            .map(|p| p.1)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = positions.iter().map(|p| p.2).fold(f64::INFINITY, f64::min);
        let max_y = positions
            .iter()
            .map(|p| p.2)
            .fold(f64::NEG_INFINITY, f64::max);
        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;
        let gs = self.grid_size_mm as f64;

        for sym in &mut sheet.symbols {
            if !selected_uuids.contains(&sym.uuid) {
                continue;
            }
            match action {
                ActiveBarAction::AlignLeft => sym.position.x = min_x,
                ActiveBarAction::AlignRight => sym.position.x = max_x,
                ActiveBarAction::AlignTop => sym.position.y = min_y,
                ActiveBarAction::AlignBottom => sym.position.y = max_y,
                ActiveBarAction::AlignHorizontalCenters => sym.position.x = center_x,
                ActiveBarAction::AlignVerticalCenters => sym.position.y = center_y,
                ActiveBarAction::AlignToGrid => {
                    sym.position.x = (sym.position.x / gs).round() * gs;
                    sym.position.y = (sym.position.y / gs).round() * gs;
                }
                _ => {}
            }
        }

        // Distribute evenly
        if matches!(
            action,
            ActiveBarAction::DistributeHorizontally | ActiveBarAction::DistributeVertically
        ) {
            let mut sorted = positions.clone();
            let n = sorted.len();
            if n > 2 {
                match action {
                    ActiveBarAction::DistributeHorizontally => {
                        sorted.sort_by(|a, b| a.1.total_cmp(&b.1));
                        let step = (max_x - min_x) / (n - 1) as f64;
                        for (i, (uuid, _, _)) in sorted.iter().enumerate() {
                            if let Some(sym) = sheet.symbols.iter_mut().find(|s| s.uuid == *uuid) {
                                sym.position.x = min_x + step * i as f64;
                            }
                        }
                    }
                    ActiveBarAction::DistributeVertically => {
                        sorted.sort_by(|a, b| a.2.total_cmp(&b.2));
                        let step = (max_y - min_y) / (n - 1) as f64;
                        for (i, (uuid, _, _)) in sorted.iter().enumerate() {
                            if let Some(sym) = sheet.symbols.iter_mut().find(|s| s.uuid == *uuid) {
                                sym.position.y = min_y + step * i as f64;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        self.canvas.schematic = Some(sheet.clone());
        self.canvas.clear_content_cache();
        self.canvas.clear_overlay_cache();
        self.mark_dirty();
        self.commit_schematic();
    }

    fn mark_dirty(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.dirty = true;
        }
    }

    /// Write the current working schematic back to the active tab.
    /// Must be called after every mutation to keep tab state in sync.
    fn commit_schematic(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.schematic = self.schematic.clone();
        }
    }

    fn update_selection_info(&mut self) {
        use signex_types::schematic::SelectedKind;

        let selected = &self.canvas.selected;
        self.panel_ctx.selection_count = selected.len();
        self.panel_ctx.selection_info.clear();

        if selected.len() != 1 {
            if !selected.is_empty() {
                self.panel_ctx
                    .selection_info
                    .push(("Selected".into(), format!("{} items", selected.len())));
            }
            return;
        }

        let item = &selected[0];
        if let Some(ref sheet) = self.schematic {
            match item.kind {
                SelectedKind::Symbol => {
                    if let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid) {
                        self.panel_ctx
                            .selection_info
                            .push(("Type".into(), "Symbol".into()));
                        self.panel_ctx
                            .selection_info
                            .push(("Reference".into(), sym.reference.clone()));
                        self.panel_ctx
                            .selection_info
                            .push(("Value".into(), sym.value.clone()));
                        self.panel_ctx
                            .selection_info
                            .push(("Library ID".into(), sym.lib_id.clone()));
                        self.panel_ctx
                            .selection_info
                            .push(("Footprint".into(), sym.footprint.clone()));
                        self.panel_ctx.selection_info.push((
                            "Position".into(),
                            format!("{:.2}, {:.2} mm", sym.position.x, sym.position.y),
                        ));
                        self.panel_ctx
                            .selection_info
                            .push(("Rotation".into(), format!("{:.0}\u{00b0}", sym.rotation)));
                        if sym.mirror_x {
                            self.panel_ctx
                                .selection_info
                                .push(("Mirror".into(), "X".into()));
                        }
                        if sym.mirror_y {
                            self.panel_ctx
                                .selection_info
                                .push(("Mirror".into(), "Y".into()));
                        }
                        if sym.unit > 1 {
                            self.panel_ctx
                                .selection_info
                                .push(("Unit".into(), format!("{}", sym.unit)));
                        }
                    }
                }
                SelectedKind::Wire => {
                    if let Some(w) = sheet.wires.iter().find(|w| w.uuid == item.uuid) {
                        let dx = w.end.x - w.start.x;
                        let dy = w.end.y - w.start.y;
                        let len = (dx * dx + dy * dy).sqrt();
                        self.panel_ctx
                            .selection_info
                            .push(("Type".into(), "Wire".into()));
                        self.panel_ctx.selection_info.push((
                            "Start".into(),
                            format!("{:.2}, {:.2}", w.start.x, w.start.y),
                        ));
                        self.panel_ctx
                            .selection_info
                            .push(("End".into(), format!("{:.2}, {:.2}", w.end.x, w.end.y)));
                        self.panel_ctx
                            .selection_info
                            .push(("Length".into(), format!("{:.2} mm", len)));
                    }
                }
                SelectedKind::Label => {
                    if let Some(l) = sheet.labels.iter().find(|l| l.uuid == item.uuid) {
                        self.panel_ctx
                            .selection_info
                            .push(("Type".into(), format!("{:?} Label", l.label_type)));
                        self.panel_ctx
                            .selection_info
                            .push(("Net Name".into(), l.text.clone()));
                        self.panel_ctx.selection_info.push((
                            "Position".into(),
                            format!("{:.2}, {:.2}", l.position.x, l.position.y),
                        ));
                    }
                }
                SelectedKind::Junction => {
                    if let Some(j) = sheet.junctions.iter().find(|j| j.uuid == item.uuid) {
                        self.panel_ctx
                            .selection_info
                            .push(("Type".into(), "Junction".into()));
                        self.panel_ctx.selection_info.push((
                            "Position".into(),
                            format!("{:.2}, {:.2}", j.position.x, j.position.y),
                        ));
                    }
                }
                SelectedKind::NoConnect => {
                    if let Some(nc) = sheet.no_connects.iter().find(|n| n.uuid == item.uuid) {
                        self.panel_ctx
                            .selection_info
                            .push(("Type".into(), "No Connect".into()));
                        self.panel_ctx.selection_info.push((
                            "Position".into(),
                            format!("{:.2}, {:.2}", nc.position.x, nc.position.y),
                        ));
                    }
                }
                SelectedKind::TextNote => {
                    if let Some(tn) = sheet.text_notes.iter().find(|t| t.uuid == item.uuid) {
                        self.panel_ctx
                            .selection_info
                            .push(("Type".into(), "Text Note".into()));
                        self.panel_ctx
                            .selection_info
                            .push(("Text".into(), tn.text.clone()));
                        self.panel_ctx.selection_info.push((
                            "Position".into(),
                            format!("{:.2}, {:.2}", tn.position.x, tn.position.y),
                        ));
                    }
                }
                SelectedKind::ChildSheet => {
                    if let Some(cs) = sheet.child_sheets.iter().find(|c| c.uuid == item.uuid) {
                        self.panel_ctx
                            .selection_info
                            .push(("Type".into(), "Hierarchical Sheet".into()));
                        self.panel_ctx
                            .selection_info
                            .push(("Name".into(), cs.name.clone()));
                        self.panel_ctx
                            .selection_info
                            .push(("File".into(), cs.filename.clone()));
                        self.panel_ctx.selection_info.push((
                            "Position".into(),
                            format!("{:.2}, {:.2}", cs.position.x, cs.position.y),
                        ));
                        self.panel_ctx.selection_info.push((
                            "Size".into(),
                            format!("{:.1} x {:.1} mm", cs.size.0, cs.size.1),
                        ));
                    }
                }
                SelectedKind::Bus => {
                    if let Some(b) = sheet.buses.iter().find(|b| b.uuid == item.uuid) {
                        self.panel_ctx
                            .selection_info
                            .push(("Type".into(), "Bus".into()));
                        self.panel_ctx.selection_info.push((
                            "Start".into(),
                            format!("{:.2}, {:.2}", b.start.x, b.start.y),
                        ));
                        self.panel_ctx
                            .selection_info
                            .push(("End".into(), format!("{:.2}, {:.2}", b.end.x, b.end.y)));
                    }
                }
                SelectedKind::BusEntry | SelectedKind::Drawing => {
                    self.panel_ctx
                        .selection_info
                        .push(("Type".into(), format!("{:?}", item.kind)));
                }
            }
        }
    }

    fn update_canvas_theme(&mut self) {
        let colors = signex_types::theme::canvas_colors(self.theme_id);
        self.canvas.set_theme_colors(
            signex_render::colors::to_iced(&colors.background),
            signex_render::colors::to_iced(&colors.grid),
            signex_render::colors::to_iced(&colors.paper),
        );
        self.canvas.canvas_colors = colors;
        self.canvas.clear_content_cache();
    }

    /// Right-click context menu with actions based on current state.
    #[allow(clippy::vec_init_then_push)] // conditional items require push
    fn view_context_menu(&self) -> Element<'_, Message> {
        let mut items: Vec<Element<'_, Message>> = Vec::with_capacity(20);
        // Common items (both empty and selection context)
        items.push(self.ctx_menu_item_disabled("Find Similar Objects..."));
        items.push(self.ctx_menu_item_disabled("Find Text...                    Ctrl+F"));
        items.push(self.ctx_menu_item_disabled("Clear Filter                  Shift+C"));
        items.push(self.ctx_menu_sep());
        items.push(self.ctx_menu_item_disabled("Place                              \u{25B6}"));
        items.push(self.ctx_menu_item_disabled("Part Actions                       \u{25B6}"));
        items.push(self.ctx_menu_item_disabled("Sheet Actions                      \u{25B6}"));

        if !self.canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_disabled("References                         \u{25B6}"));
            items.push(self.ctx_menu_item_disabled("Align                              \u{25B6}"));
            items.push(self.ctx_menu_item_disabled("Unions                             \u{25B6}"));
            items.push(self.ctx_menu_item_disabled("Snippets                           \u{25B6}"));
        }

        items.push(self.ctx_menu_item_disabled("Cross Probe"));
        items.push(self.ctx_menu_sep());

        // Edit operations
        // TODO: implement proper Cut (copy+delete)
        items.push(self.ctx_menu_item_kb("Cut", "Ctrl+X", ContextAction::Delete));
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

        items.push(self.ctx_menu_item_disabled("Comment..."));
        items.push(self.ctx_menu_item_disabled("Pin Mapping..."));
        items.push(self.ctx_menu_item_disabled("Project Options..."));
        items.push(self.ctx_menu_item_disabled("Preferences..."));

        if !self.canvas.selected.is_empty() {
            items.push(self.ctx_menu_item_disabled("Supplier Links..."));
            items.push(self.ctx_menu_item_disabled("Properties..."));
        }

        container(column(items).spacing(0))
            .padding([4, 0])
            .style(crate::styles::context_menu)
            .into()
    }

    /// Context menu item with keyboard shortcut on the right.
    fn ctx_menu_item_kb<'a>(
        &self,
        label: &str,
        shortcut: &str,
        action: ContextAction,
    ) -> Element<'a, Message> {
        iced::widget::button(
            iced::widget::row![
                iced::widget::text(label.to_string())
                    .size(11)
                    .color(crate::styles::TEXT_PRIMARY),
                iced::widget::Space::new().width(Length::Fill),
                iced::widget::text(shortcut.to_string())
                    .size(10)
                    .color(crate::styles::TEXT_MUTED),
            ]
            .spacing(12)
            .width(220),
        )
        .padding([4, 12])
        .on_press(Message::ContextAction(action))
        .style(|_: &iced::Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                    iced::Color::from_rgb(0.22, 0.22, 0.26),
                )),
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                border: iced::Border::default(),
                text_color: crate::styles::TEXT_PRIMARY,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    }

    /// Disabled/placeholder context menu item (no action).
    fn ctx_menu_item_disabled<'a>(&self, label: &str) -> Element<'a, Message> {
        container(
            iced::widget::text(label.to_string())
                .size(11)
                .color(crate::styles::TEXT_MUTED),
        )
        .padding([4, 12])
        .width(220)
        .into()
    }

    fn ctx_menu_sep<'a>(&self) -> Element<'a, Message> {
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(1)
            .style(|_: &iced::Theme| container::Style {
                background: Some(crate::styles::BORDER_SUBTLE.into()),
                ..container::Style::default()
            })
            .into()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let menu = menu_bar::view(self.theme_id, self.active_menu).map(Message::Menu);
        let tools = toolbar::view(self.current_tool).map(Message::Tool);

        // Collapse states (needed before building panels and handles)
        let left_collapsed = self.dock.is_collapsed(PanelPosition::Left);
        let right_collapsed = self.dock.is_collapsed(PanelPosition::Right);
        let bottom_collapsed = self.dock.is_collapsed(PanelPosition::Bottom);

        // Left panel (resizable width, collapsible)
        let left_panel = self
            .dock
            .view_region(PanelPosition::Left, &self.panel_ctx)
            .map(Message::Dock);
        let left_w = if left_collapsed {
            28.0
        } else {
            self.left_width
        };
        let left = container(left_panel)
            .width(left_w)
            .height(Length::Fill)
            .style(crate::styles::panel_region);

        // Left resize handle (hidden when collapsed)
        let left_handle_w = if left_collapsed { 0 } else { 5 };
        let left_handle = iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(left_handle_w)
                .height(Length::Fill)
                .style(crate::styles::resize_handle),
        )
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(Message::DragStart(DragTarget::LeftPanel));

        // Center — canvas when a schematic is loaded, empty otherwise
        let center: Element<'_, Message> = if self.schematic.is_some() {
            let canvas_widget: Element<'_, Message> = canvas(&self.canvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into();

            canvas_widget
        } else {
            container(
                column![
                    iced::widget::text("No document open")
                        .size(14)
                        .color(crate::styles::TEXT_MUTED),
                    iced::widget::text("Open a project with File > Open or Ctrl+O")
                        .size(11)
                        .color(crate::styles::TEXT_MUTED),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(crate::styles::panel_region)
            .into()
        };

        // Right resize handle (hidden when collapsed)
        let right_handle_w = if right_collapsed { 0 } else { 5 };
        let right_handle = iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(right_handle_w)
                .height(Length::Fill)
                .style(crate::styles::resize_handle),
        )
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(Message::DragStart(DragTarget::RightPanel));

        // Right panel (resizable width, collapsible)
        let right_panel = self
            .dock
            .view_region(PanelPosition::Right, &self.panel_ctx)
            .map(Message::Dock);
        let right_w = if right_collapsed {
            28.0
        } else {
            self.right_width
        };
        let right = container(right_panel)
            .width(right_w)
            .height(Length::Fill)
            .style(crate::styles::panel_region);

        // Center row: left | handle | center | handle | right
        let center_row = row![left, left_handle, center, right_handle, right];

        // Bottom resize handle (hidden when collapsed)
        let bottom_handle_h = if bottom_collapsed { 0 } else { 5 };
        let bottom_handle = iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(bottom_handle_h)
                .style(crate::styles::resize_handle),
        )
        .interaction(iced::mouse::Interaction::ResizingVertically)
        .on_press(Message::DragStart(DragTarget::BottomPanel));

        // Bottom panel (resizable height, collapsible)
        let bottom_panel = self
            .dock
            .view_region(PanelPosition::Bottom, &self.panel_ctx)
            .map(Message::Dock);
        let bottom_h = if bottom_collapsed {
            28.0
        } else {
            self.bottom_height
        };
        let bottom = container(bottom_panel)
            .width(Length::Fill)
            .height(bottom_h)
            .style(crate::styles::panel_region);

        // Status bar
        let status = status_bar::view(
            self.cursor_x,
            self.cursor_y,
            self.grid_visible,
            self.snap_enabled,
            self.zoom,
            self.unit,
            &self.current_tool,
            self.grid_size_mm,
        )
        .map(Message::StatusBar);

        let mut main = column![menu, tools];
        if !self.tabs.is_empty() {
            main = main.push(tab_bar::view(&self.tabs, self.active_tab).map(Message::Tab));
        }
        // Active Bar (centered above canvas)
        if self.schematic.is_some() {
            let bar = crate::active_bar::view_bar(
                self.current_tool,
                self.draw_mode,
                &self.last_tool,
            )
            .map(Message::ActiveBar);
            main = main.push(
                container(bar)
                    .width(Length::Fill)
                    .padding([3, 0])
                    .align_x(iced::alignment::Horizontal::Center)
                    .style(|_: &iced::Theme| container::Style {
                        background: Some(iced::Color::from_rgb(0.09, 0.09, 0.10).into()),
                        ..container::Style::default()
                    }),
            );
        }
        let main = main
            .push(center_row)
            .push(bottom_handle)
            .push(bottom)
            .push(status);

        // Overlay: dropdown menus, Active Bar dropdowns, and context menus via Stack
        let has_menu = self.active_menu.is_some();
        let has_context = self.context_menu.is_some();
        let has_ab_menu = self.active_bar_menu.is_some();

        if has_menu || has_context || has_ab_menu || self.panel_list_open {
            let mut stack = iced::widget::Stack::new().push(main);

            // Dropdown menu overlay
            if let Some(idx) = self.active_menu {
                let x_offset = menu_bar::button_x_offset(idx);
                let dropdown = menu_bar::view_dropdown(idx).map(Message::Menu);

                // Dismiss layer
                stack = stack.push(column![
                    iced::widget::Space::new().height(menu_bar::MENU_BAR_HEIGHT),
                    iced::widget::mouse_area(
                        container(iced::widget::Space::new())
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .on_press(Message::Menu(menu_bar::MenuMessage::CloseMenus)),
                ]);
                // Dropdown positioned below menu bar
                stack = stack.push(column![
                    iced::widget::Space::new().height(menu_bar::MENU_BAR_HEIGHT),
                    row![iced::widget::Space::new().width(x_offset), dropdown,],
                ]);
            }

            // Active Bar dropdown overlay
            if let Some(ab_menu) = self.active_bar_menu {
                let dropdown = crate::active_bar::view_dropdown(ab_menu).map(Message::ActiveBar);
                let x_off = crate::active_bar::dropdown_x_offset(ab_menu);
                // Vertical: menu(24) + toolbar(28) + tabs(~28) + bar on canvas(~36)
                let ab_y: f32 = 24.0 + 28.0 + if self.tabs.is_empty() { 0.0 } else { 28.0 } + 36.0;
                // Dismiss layer
                stack = stack.push(
                    iced::widget::mouse_area(
                        container(iced::widget::Space::new())
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .on_press(Message::ActiveBar(
                        crate::active_bar::ActiveBarMsg::CloseMenus,
                    )),
                );
                // Dropdown — aligned under the clicked button
                let bar_w: f32 = crate::active_bar::BAR_WIDTH_PX;
                stack = stack.push(
                    container(column![
                        iced::widget::Space::new().height(ab_y),
                        container(row![iced::widget::Space::new().width(x_off), dropdown,],)
                            .width(bar_w),
                    ])
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
                );
            }

            // Context menu overlay (right-click)
            if let Some(ref ctx_menu) = self.context_menu {
                let menu = self.view_context_menu();
                // Dismiss layer
                stack = stack.push(
                    iced::widget::mouse_area(
                        container(iced::widget::Space::new())
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .on_press(Message::CloseContextMenu),
                );
                // Menu positioned at click point
                stack = stack.push(column![
                    iced::widget::Space::new().height(ctx_menu.y),
                    row![iced::widget::Space::new().width(ctx_menu.x), menu,],
                ]);
            }

            // Panel list popup (bottom-right)
            if self.panel_list_open {
                // Dismiss layer
                stack = stack.push(
                    iced::widget::mouse_area(
                        container(iced::widget::Space::new())
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .on_press(Message::TogglePanelList),
                );
                // Panel list popup at bottom-right
                let mut panel_items: Vec<Element<'_, Message>> = Vec::new();
                for &kind in crate::panels::ALL_PANELS {
                    panel_items.push(
                        iced::widget::button(
                            iced::widget::text(kind.label().to_string())
                                .size(11)
                                .color(crate::styles::TEXT_PRIMARY),
                        )
                        .padding([4, 12])
                        .width(Length::Fill)
                        .on_press(Message::OpenPanel(kind))
                        .style(|_: &iced::Theme, status: iced::widget::button::Status| {
                            let bg = match status {
                                iced::widget::button::Status::Hovered => {
                                    Some(iced::Background::Color(iced::Color::from_rgb(
                                        0.20, 0.22, 0.30,
                                    )))
                                }
                                _ => None,
                            };
                            iced::widget::button::Style {
                                background: bg,
                                border: iced::Border::default(),
                                text_color: crate::styles::TEXT_PRIMARY,
                                ..iced::widget::button::Style::default()
                            }
                        })
                        .into(),
                    );
                }
                let popup = container(
                    iced::widget::scrollable(column(panel_items).spacing(0).width(180)).height(300),
                )
                .padding([6, 0])
                .style(crate::styles::context_menu);

                stack = stack.push(
                    container(
                        container(popup)
                            .align_x(iced::alignment::Horizontal::Right)
                            .align_y(iced::alignment::Vertical::Bottom)
                            .padding([15, 10]),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill),
                );
            }

            stack.into()
        } else {
            main.into()
        }
    }
}
