//! Main Iced application — Message enum, update loop, view tree.

use std::path::PathBuf;

use iced::widget::{canvas, column, container, row};
use iced::{Element, Length, Rectangle, Subscription, Task, Theme};
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
                .filter(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|ext| ext == "kicad_sym")
                })
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
pub enum Message {
    Menu(MenuMessage),
    Tool(ToolMessage),
    Tab(TabMessage),
    Dock(DockMessage),
    StatusBar(StatusBarMsg),
    CanvasEvent(CanvasEvent),
    ThemeChanged(ThemeId),
    UnitCycled,
    GridToggle,
    GridCycle,
    DragStart(DragTarget),
    DragMove(f32, f32),
    DragEnd,
    FileOpened(Option<PathBuf>),
    SchematicLoaded(Box<SchematicSheet>),
    // v0.5: Editing operations
    DeleteSelected,
    Undo,
    Redo,
    RotateSelected,
    MirrorSelectedX,
    MirrorSelectedY,
    CycleDrawMode,
    CancelDrawing,
    Noop,
}

#[derive(Debug, Clone)]
pub enum StatusBarMsg {
    CycleUnit,
    ToggleGrid,
    ToggleSnap,
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
    pub draw_mode: DrawMode,
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

    pub fn label(self) -> &'static str {
        match self {
            DrawMode::Ortho90 => "90°",
            DrawMode::Angle45 => "45°",
            DrawMode::FreeAngle => "Any",
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
) -> Vec<(signex_types::schematic::Point, signex_types::schematic::Point)> {
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
                kicad_libraries: find_kicad_symbols_dir()
                    .map(|d| list_kicad_libraries(&d))
                    .unwrap_or_default(),
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
            },
            left_width: 240.0,
            right_width: 220.0,
            bottom_height: 120.0,
            active_menu: None,
            kicad_lib_dir: find_kicad_symbols_dir(),
            loaded_lib: std::collections::HashMap::new(),
            dragging: None,
            drag_start_pos: None,
            drag_start_size: 0.0,
            undo_stack: crate::undo::UndoStack::new(100),
            wire_points: Vec::new(),
            wire_drawing: false,
            draw_mode: DrawMode::default(),
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
                (keyboard::Key::Character(c), m)
                    if c == "g" && !m.command() && !m.shift() =>
                {
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
                (keyboard::Key::Named(keyboard::key::Named::Delete), _) => {
                    Message::DeleteSelected
                }
                // Undo/Redo
                (keyboard::Key::Character(c), m) if c == "z" && m.command() && !m.shift() => {
                    Message::Undo
                }
                (keyboard::Key::Character(c), m) if c == "y" && m.command() => {
                    Message::Redo
                }
                (keyboard::Key::Character(c), m) if c == "z" && m.command() && m.shift() => {
                    Message::Redo
                }
                // Shift+Space: cycle draw mode (90° → 45° → Free)
                (keyboard::Key::Named(keyboard::key::Named::Space), m) if m.shift() => {
                    Message::CycleDrawMode
                }
                // Space: rotate selected symbol (Altium convention)
                (keyboard::Key::Named(keyboard::key::Named::Space), _) => {
                    Message::RotateSelected
                }
                // Mirror: X = flip horizontal (mirror about Y-axis), Y = flip vertical (mirror about X-axis)
                // Matches Altium Designer shortcuts
                (keyboard::Key::Character(c), m) if c == "x" && !m.command() => {
                    Message::MirrorSelectedY  // X key = horizontal flip = mirror Y-axis
                }
                (keyboard::Key::Character(c), m) if c == "y" && !m.command() => {
                    Message::MirrorSelectedX  // Y key = vertical flip = mirror X-axis
                }
                // Shift+Ctrl+G — toggle grid visibility
                (keyboard::Key::Character(c), m) if c == "g" && m.command() && m.shift() => {
                    Message::GridToggle
                }
                _ => Message::Noop,
            },
            _ => Message::Noop,
        });

        // Mouse events for drag-to-resize (always subscribed, filtered in update)
        let mouse_sub = iced::event::listen().map(|event| match event {
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                Message::DragMove(position.x, position.y)
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                iced::mouse::Button::Left,
            )) => Message::DragEnd,
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
            Message::DragMove(x, y) if self.dragging.is_some() => {
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
                            self.canvas.tool_preview = Some(format!("Draw Wire [{}]", self.draw_mode.label()));
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
                            self.canvas.tool_preview = Some(format!("Draw Bus [{}]", self.draw_mode.label()));
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
                        }
                        self.current_tool = Tool::Select;
                    }
                    _ => {
                        // Selection mode: hit-test
                        if let Some(ref sheet) = self.schematic {
                            let hit = signex_render::schematic::hit_test::hit_test(sheet, world_x, world_y);
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
            Message::CycleDrawMode => {
                self.draw_mode = self.draw_mode.next();
                // Update tool preview to show current mode
                if self.wire_drawing || matches!(self.current_tool, Tool::Wire | Tool::Bus) {
                    self.canvas.tool_preview = Some(format!(
                        "{} [{}]",
                        if self.current_tool == Tool::Bus { "Draw Bus" } else { "Draw Wire" },
                        self.draw_mode.label()
                    ));
                }
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
                self.canvas.pending_fit = None;
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
                self.canvas.tool_preview = match tool {
                    Tool::Label => Some("NET".into()),
                    Tool::Component => Some("Place Component".into()),
                    Tool::Wire => Some("Draw Wire".into()),
                    Tool::Bus => Some("Draw Bus".into()),
                    _ => None,
                };
                // Escape: close menus and cancel wire drawing
                if tool == Tool::Select {
                    self.active_menu = None;
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
                use signex_widgets::tree_view::{get_node, TreeIcon, TreeMsg};
                match &msg {
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::SetUnit(unit),
                    ) => {
                        self.unit = *unit;
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::ToggleGrid,
                    ) => {
                        self.grid_visible = !self.grid_visible;
                        self.canvas.grid_visible = self.grid_visible;
                        self.canvas.clear_bg_cache();
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::ToggleSnap,
                    ) => {
                        self.snap_enabled = !self.snap_enabled;
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::PropertiesTab(idx),
                    ) => {
                        self.panel_ctx.properties_tab = *idx;
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::SelectLibrary(name),
                    ) => {
                        let name = name.clone();
                        if let Some(dir) = &self.kicad_lib_dir {
                            let path = dir.join(format!("{name}.kicad_sym"));
                            match std::fs::read_to_string(&path) {
                                Ok(content) => {
                                    match kicad_parser::parse_symbol_lib(&content) {
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
                                    }
                                }
                                Err(e) => eprintln!("Failed to read {}: {e}", path.display()),
                            }
                        }
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::ComponentFilter(filter),
                    ) => {
                        self.panel_ctx.component_filter = filter.clone();
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::DragComponentsSplit,
                    ) => {
                        self.dragging = Some(DragTarget::ComponentsSplit);
                        self.drag_start_pos = None;
                        self.drag_start_size = self.panel_ctx.components_split;
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::SelectComponent(name),
                    ) => {
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
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::Tree(TreeMsg::Toggle(path)),
                    ) => {
                        let path = path.clone();
                        signex_widgets::tree_view::toggle(
                            &mut self.panel_ctx.project_tree,
                            &path,
                        );
                    }
                    crate::dock::DockMessage::Panel(
                        crate::panels::PanelMsg::Tree(TreeMsg::Select(path)),
                    ) => {
                        if let Some(node) =
                            get_node(&self.panel_ctx.project_tree, path)
                        {
                            if matches!(
                                node.icon,
                                TreeIcon::Schematic | TreeIcon::Pcb
                            ) {
                                let filename = node.label.clone();
                                if let Some(dir) = self
                                    .project_path
                                    .as_ref()
                                    .and_then(|p| p.parent())
                                {
                                    let file_path = dir.join(&filename);
                                    if file_path.exists()
                                        && filename.ends_with(".kicad_sch")
                                    {
                                        // Already open? Switch to it
                                        if let Some(idx) = self.tabs.iter().position(|t| t.path == file_path) {
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
                    }
                    _ => {}
                }
                self.dock.update(msg);
            }
            Message::FileOpened(Some(path)) => {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                match ext {
                    "kicad_pro" => {
                        // Parse project file — discovers all sheets
                        match kicad_parser::parse_project(&path) {
                            Ok(proj) => {
                                self.project_path = Some(path.clone());
                                self.project_data = Some(proj.clone());
                                // Don't auto-open schematic — user clicks on project tree to open sheets
                                // (Altium behavior: project load shows tree only)
                                self.refresh_panel_ctx();
                            }
                            Err(e) => eprintln!("Failed to parse project: {e}"),
                        }
                    }
                    "kicad_sch" => {
                        // Direct schematic open — also try to find the .kicad_pro
                        match kicad_parser::parse_schematic_file(&path) {
                            Ok(sheet) => {
                                self.project_path = Some(path.clone());
                                // Try to find and parse the .kicad_pro in the same directory
                                if let Some(dir) = path.parent() {
                                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                    let pro_path = dir.join(format!("{stem}.kicad_pro"));
                                    if pro_path.exists() {
                                        if let Ok(proj) = kicad_parser::parse_project(&pro_path) {
                                            self.project_data = Some(proj);
                                        }
                                    }
                                }
                                let title = path.file_stem()
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
                if !self.canvas.selected.is_empty() {
                    if let Some(ref mut sheet) = self.schematic {
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
                                    if let Some(j) = sheet.junctions.iter().find(|j| j.uuid == item.uuid) {
                                        cmds.push(crate::undo::EditCommand::RemoveJunction(j.clone()));
                                    }
                                }
                                SelectedKind::NoConnect => {
                                    if let Some(nc) = sheet.no_connects.iter().find(|n| n.uuid == item.uuid) {
                                        cmds.push(crate::undo::EditCommand::RemoveNoConnect(nc.clone()));
                                    }
                                }
                                SelectedKind::Symbol => {
                                    if let Some(s) = sheet.symbols.iter().find(|s| s.uuid == item.uuid) {
                                        cmds.push(crate::undo::EditCommand::RemoveSymbol(s.clone()));
                                    }
                                }
                                SelectedKind::TextNote => {
                                    if let Some(tn) = sheet.text_notes.iter().find(|t| t.uuid == item.uuid) {
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
                            self.update_selection_info();
                        }
                    }
                }
            }
            Message::Undo => {
                if let Some(ref mut sheet) = self.schematic {
                    if self.undo_stack.undo(sheet) {
                        self.canvas.schematic = Some(sheet.clone());
                        self.canvas.selected.clear();
                        self.canvas.clear_content_cache();
                        self.canvas.clear_overlay_cache();
                        self.mark_dirty();
                        self.update_selection_info();
                    }
                }
            }
            Message::Redo => {
                if let Some(ref mut sheet) = self.schematic {
                    if self.undo_stack.redo(sheet) {
                        self.canvas.schematic = Some(sheet.clone());
                        self.canvas.selected.clear();
                        self.canvas.clear_content_cache();
                        self.canvas.clear_overlay_cache();
                        self.mark_dirty();
                        self.update_selection_info();
                    }
                }
            }
            Message::RotateSelected => {
                if self.canvas.selected.len() == 1 {
                    let item = self.canvas.selected[0];
                    if item.kind == signex_types::schematic::SelectedKind::Symbol {
                        if let Some(ref mut sheet) = self.schematic {
                            if let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid) {
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
                                self.update_selection_info();
                            }
                        }
                    }
                }
            }
            Message::MirrorSelectedX => {
                if self.canvas.selected.len() == 1 {
                    let item = self.canvas.selected[0];
                    if item.kind == signex_types::schematic::SelectedKind::Symbol {
                        if let Some(ref mut sheet) = self.schematic {
                            if let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid) {
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
                            }
                        }
                    }
                }
            }
            Message::MirrorSelectedY => {
                if self.canvas.selected.len() == 1 {
                    let item = self.canvas.selected[0];
                    if item.kind == signex_types::schematic::SelectedKind::Symbol {
                        if let Some(ref mut sheet) = self.schematic {
                            if let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid) {
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
                            }
                        }
                    }
                }
            }
            Message::SchematicLoaded(sheet) => {
                self.schematic = Some(*sheet);
                self.canvas.clear_content_cache();
            }
            Message::DragMove(_, _) | Message::DragEnd => {} // not dragging, ignore
            Message::Noop => {}
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
                        .set_title("Open KiCad Schematic")
                        .add_filter("KiCad Schematic", &["kicad_sch"])
                        .add_filter("KiCad Project", &["kicad_pro"])
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
            // ── Stubs (not yet implemented) ──
            MenuMessage::NewProject
            | MenuMessage::Save
            | MenuMessage::SaveAs
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
        let sheets: Vec<crate::panels::SheetInfo> = self.project_data.as_ref()
            .map(|proj| {
                proj.sheets.iter().map(|s| crate::panels::SheetInfo {
                    name: s.name.clone(),
                    filename: s.filename.clone(),
                    sym_count: s.symbols_count,
                    wire_count: s.wires_count,
                    label_count: s.labels_count,
                }).collect()
            })
            .unwrap_or_default();

        let project_name = self.project_data.as_ref()
            .map(|p| p.name.clone())
            .or_else(|| self.project_path.as_ref().and_then(|p| {
                p.file_stem().map(|s| s.to_string_lossy().to_string())
            }));

        self.panel_ctx = crate::panels::PanelContext {
            project_name,
            project_file: self.project_data.as_ref()
                .and_then(|p| p.schematic_root.clone())
                .or_else(|| self.project_path.as_ref().and_then(|p| {
                    p.file_name().map(|s| s.to_string_lossy().to_string())
                })),
            pcb_file: self.project_data.as_ref().and_then(|p| p.pcb_file.clone()),
            sheets,
            sym_count: self.schematic.as_ref().map(|s| s.symbols.len()).unwrap_or(0),
            wire_count: self.schematic.as_ref().map(|s| s.wires.len()).unwrap_or(0),
            label_count: self.schematic.as_ref().map(|s| s.labels.len()).unwrap_or(0),
            junction_count: self.schematic.as_ref().map(|s| s.junctions.len()).unwrap_or(0),
            child_sheets: self.schematic.as_ref()
                .map(|s| s.child_sheets.iter().map(|c| c.name.clone()).collect())
                .unwrap_or_default(),
            has_schematic: self.schematic.is_some(),
            paper_size: self.schematic.as_ref()
                .map(|s| s.paper_size.clone())
                .unwrap_or_else(|| "A4".to_string()),
            lib_symbol_count: self.schematic.as_ref()
                .map(|s| s.lib_symbols.len())
                .unwrap_or(0),
            lib_symbol_names: self.schematic.as_ref()
                .map(|s| s.lib_symbols.iter().map(|(name, _)| name.clone()).collect())
                .unwrap_or_default(),
            placed_symbols: self.schematic.as_ref()
                .map(|s| {
                    s.symbols.iter().map(|sym| {
                        (sym.reference.clone(), sym.value.clone(), sym.footprint.clone(), sym.lib_id.clone())
                    }).collect()
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
        self.panel_ctx.sym_count =
            self.schematic.as_ref().map(|s| s.symbols.len()).unwrap_or(0);
        self.panel_ctx.wire_count =
            self.schematic.as_ref().map(|s| s.wires.len()).unwrap_or(0);
        self.panel_ctx.label_count =
            self.schematic.as_ref().map(|s| s.labels.len()).unwrap_or(0);
        self.panel_ctx.junction_count =
            self.schematic.as_ref().map(|s| s.junctions.len()).unwrap_or(0);
        self.panel_ctx.lib_symbol_count =
            self.schematic.as_ref().map(|s| s.lib_symbols.len()).unwrap_or(0);
        self.panel_ctx.lib_symbol_names = self
            .schematic
            .as_ref()
            .map(|s| s.lib_symbols.iter().map(|(name, _)| name.clone()).collect())
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

    fn mark_dirty(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.dirty = true;
        }
    }

    fn update_selection_info(&mut self) {
        use signex_types::schematic::SelectedKind;

        let selected = &self.canvas.selected;
        self.panel_ctx.selection_count = selected.len();
        self.panel_ctx.selection_info.clear();

        if selected.len() != 1 {
            if !selected.is_empty() {
                self.panel_ctx.selection_info.push((
                    "Selected".into(),
                    format!("{} items", selected.len()),
                ));
            }
            return;
        }

        let item = &selected[0];
        if let Some(ref sheet) = self.schematic {
            match item.kind {
                SelectedKind::Symbol => {
                    if let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid) {
                        self.panel_ctx.selection_info.push(("Type".into(), "Symbol".into()));
                        self.panel_ctx.selection_info.push(("Reference".into(), sym.reference.clone()));
                        self.panel_ctx.selection_info.push(("Value".into(), sym.value.clone()));
                        self.panel_ctx.selection_info.push(("Library ID".into(), sym.lib_id.clone()));
                        self.panel_ctx.selection_info.push(("Footprint".into(), sym.footprint.clone()));
                        self.panel_ctx.selection_info.push(("Position".into(), format!("{:.2}, {:.2} mm", sym.position.x, sym.position.y)));
                        self.panel_ctx.selection_info.push(("Rotation".into(), format!("{:.0}\u{00b0}", sym.rotation)));
                        if sym.mirror_x { self.panel_ctx.selection_info.push(("Mirror".into(), "X".into())); }
                        if sym.mirror_y { self.panel_ctx.selection_info.push(("Mirror".into(), "Y".into())); }
                        if sym.unit > 1 { self.panel_ctx.selection_info.push(("Unit".into(), format!("{}", sym.unit))); }
                    }
                }
                SelectedKind::Wire => {
                    if let Some(w) = sheet.wires.iter().find(|w| w.uuid == item.uuid) {
                        let dx = w.end.x - w.start.x;
                        let dy = w.end.y - w.start.y;
                        let len = (dx * dx + dy * dy).sqrt();
                        self.panel_ctx.selection_info.push(("Type".into(), "Wire".into()));
                        self.panel_ctx.selection_info.push(("Start".into(), format!("{:.2}, {:.2}", w.start.x, w.start.y)));
                        self.panel_ctx.selection_info.push(("End".into(), format!("{:.2}, {:.2}", w.end.x, w.end.y)));
                        self.panel_ctx.selection_info.push(("Length".into(), format!("{:.2} mm", len)));
                    }
                }
                SelectedKind::Label => {
                    if let Some(l) = sheet.labels.iter().find(|l| l.uuid == item.uuid) {
                        self.panel_ctx.selection_info.push(("Type".into(), format!("{:?} Label", l.label_type)));
                        self.panel_ctx.selection_info.push(("Net Name".into(), l.text.clone()));
                        self.panel_ctx.selection_info.push(("Position".into(), format!("{:.2}, {:.2}", l.position.x, l.position.y)));
                    }
                }
                SelectedKind::Junction => {
                    if let Some(j) = sheet.junctions.iter().find(|j| j.uuid == item.uuid) {
                        self.panel_ctx.selection_info.push(("Type".into(), "Junction".into()));
                        self.panel_ctx.selection_info.push(("Position".into(), format!("{:.2}, {:.2}", j.position.x, j.position.y)));
                    }
                }
                SelectedKind::NoConnect => {
                    if let Some(nc) = sheet.no_connects.iter().find(|n| n.uuid == item.uuid) {
                        self.panel_ctx.selection_info.push(("Type".into(), "No Connect".into()));
                        self.panel_ctx.selection_info.push(("Position".into(), format!("{:.2}, {:.2}", nc.position.x, nc.position.y)));
                    }
                }
                SelectedKind::TextNote => {
                    if let Some(tn) = sheet.text_notes.iter().find(|t| t.uuid == item.uuid) {
                        self.panel_ctx.selection_info.push(("Type".into(), "Text Note".into()));
                        self.panel_ctx.selection_info.push(("Text".into(), tn.text.clone()));
                        self.panel_ctx.selection_info.push(("Position".into(), format!("{:.2}, {:.2}", tn.position.x, tn.position.y)));
                    }
                }
                SelectedKind::ChildSheet => {
                    if let Some(cs) = sheet.child_sheets.iter().find(|c| c.uuid == item.uuid) {
                        self.panel_ctx.selection_info.push(("Type".into(), "Hierarchical Sheet".into()));
                        self.panel_ctx.selection_info.push(("Name".into(), cs.name.clone()));
                        self.panel_ctx.selection_info.push(("File".into(), cs.filename.clone()));
                        self.panel_ctx.selection_info.push(("Position".into(), format!("{:.2}, {:.2}", cs.position.x, cs.position.y)));
                        self.panel_ctx.selection_info.push(("Size".into(), format!("{:.1} x {:.1} mm", cs.size.0, cs.size.1)));
                    }
                }
                SelectedKind::Bus => {
                    if let Some(b) = sheet.buses.iter().find(|b| b.uuid == item.uuid) {
                        self.panel_ctx.selection_info.push(("Type".into(), "Bus".into()));
                        self.panel_ctx.selection_info.push(("Start".into(), format!("{:.2}, {:.2}", b.start.x, b.start.y)));
                        self.panel_ctx.selection_info.push(("End".into(), format!("{:.2}, {:.2}", b.end.x, b.end.y)));
                    }
                }
                SelectedKind::BusEntry | SelectedKind::Drawing => {
                    self.panel_ctx.selection_info.push(("Type".into(), format!("{:?}", item.kind)));
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

    pub fn view(&self) -> Element<'_, Message> {
        let menu = menu_bar::view(self.theme_id, self.active_menu).map(Message::Menu);
        let tools = toolbar::view(self.current_tool).map(Message::Tool);

        // Left panel (resizable width)
        let left_panel = self.dock.view_region(PanelPosition::Left, &self.panel_ctx).map(Message::Dock);
        let left = container(left_panel)
            .width(self.left_width)
            .height(Length::Fill)
            .style(crate::styles::panel_region);

        // Left resize handle
        let left_handle = iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(5)
                .height(Length::Fill)
                .style(crate::styles::resize_handle),
        )
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(Message::DragStart(DragTarget::LeftPanel));

        // Center — canvas when a schematic is loaded, empty otherwise
        let center: Element<'_, Message> = if self.schematic.is_some() {
            canvas(&self.canvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
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

        // Right resize handle
        let right_handle = iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(5)
                .height(Length::Fill)
                .style(crate::styles::resize_handle),
        )
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(Message::DragStart(DragTarget::RightPanel));

        // Right panel (resizable width)
        let right_panel = self.dock.view_region(PanelPosition::Right, &self.panel_ctx).map(Message::Dock);
        let right = container(right_panel)
            .width(self.right_width)
            .height(Length::Fill)
            .style(crate::styles::panel_region);

        // Center row: left | handle | center | handle | right
        let center_row = row![left, left_handle, center, right_handle, right];

        // Bottom resize handle
        let bottom_handle = iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(5)
                .style(crate::styles::resize_handle),
        )
        .interaction(iced::mouse::Interaction::ResizingVertically)
        .on_press(Message::DragStart(DragTarget::BottomPanel));

        // Bottom panel (resizable height)
        let bottom_panel = self.dock.view_region(PanelPosition::Bottom, &self.panel_ctx).map(Message::Dock);
        let bottom = container(bottom_panel)
            .width(Length::Fill)
            .height(self.bottom_height)
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
        let main = main
            .push(center_row)
            .push(bottom_handle)
            .push(bottom)
            .push(status);

        // Overlay dropdown menu when active
        if let Some(idx) = self.active_menu {
            let x_offset = menu_bar::button_x_offset(idx);
            let dropdown = menu_bar::view_dropdown(idx).map(Message::Menu);

            iced::widget::Stack::new()
                .push(main)
                // Dismiss layer — covers area below menu bar, catches outside clicks
                .push(
                    column![
                        iced::widget::Space::new().height(menu_bar::MENU_BAR_HEIGHT),
                        iced::widget::mouse_area(
                            container(iced::widget::Space::new())
                                .width(Length::Fill)
                                .height(Length::Fill),
                        )
                        .on_press(Message::Menu(menu_bar::MenuMessage::CloseMenus)),
                    ],
                )
                // Dropdown positioned below menu bar, aligned to button
                .push(column![
                    iced::widget::Space::new().height(menu_bar::MENU_BAR_HEIGHT),
                    row![iced::widget::Space::new().width(x_offset), dropdown,],
                ])
                .into()
        } else {
            main.into()
        }
    }
}
