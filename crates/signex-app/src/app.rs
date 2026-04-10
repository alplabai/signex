//! Main Iced application — Message enum, update loop, view tree.

use std::path::PathBuf;

use iced::widget::{canvas, column, container, row};
use iced::{Element, Length, Rectangle, Subscription, Task, Theme};
use signex_types::coord::Unit;
use signex_types::schematic::SchematicSheet;
use signex_types::theme::ThemeId;

use crate::canvas::{CanvasEvent, SchematicCanvas};
use crate::dock::{DockArea, DockMessage, PanelPosition};
use crate::menu_bar::{self, MenuMessage};
use crate::panels::PanelKind;
use crate::status_bar;
use crate::tab_bar::{self, TabMessage};
use crate::toolbar::{self, ToolMessage};

// ─── Message ──────────────────────────────────────────────────

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
    FileOpened(Option<PathBuf>),
    SchematicLoaded(Box<SchematicSheet>),
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
    pub panel_ctx: crate::panels::PanelContext,
}

#[derive(Debug, Clone)]
pub struct TabInfo {
    pub title: String,
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
            theme_id: ThemeId::CatppuccinMocha,
            unit: Unit::Mm,
            grid_visible: true,
            snap_enabled: true,
            cursor_x: 0.0,
            cursor_y: 0.0,
            zoom: 100.0,
            dock,
            tabs: vec![TabInfo {
                title: "Untitled".to_string(),
                dirty: false,
            }],
            active_tab: 0,
            current_tool: Tool::Select,
            canvas: sch_canvas,
            grid_size_mm,
            schematic: None,
            project_path: None,
            panel_ctx: crate::panels::PanelContext {
                project_name: None,
                project_file: None,
                sym_count: 0,
                wire_count: 0,
                label_count: 0,
                junction_count: 0,
                child_sheets: vec![],
                has_schematic: false,
                paper_size: "A4".to_string(),
                lib_symbol_count: 0,
            },
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
            ThemeId::AltiumDark => Theme::Dark,
            ThemeId::GitHubDark => Theme::Dark,
            ThemeId::SolarizedLight => Theme::Light,
            ThemeId::Nord => Theme::Nord,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        use iced::keyboard;
        keyboard::listen().map(|event| match event {
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
                // Shift+Ctrl+G — toggle grid visibility
                (keyboard::Key::Character(c), m) if c == "g" && m.command() && m.shift() => {
                    Message::GridToggle
                }
                _ => Message::Noop,
            },
            _ => Message::Noop,
        })
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
            Message::CanvasEvent(CanvasEvent::CursorMoved) => {
                // Zoom or pan changed — grid + schematic positions shifted, must redraw
                self.canvas.clear_bg_cache();
                self.canvas.clear_content_cache();
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
            }
            Message::Menu(msg) => {
                return self.handle_menu(msg);
            }
            Message::Tab(msg) => {
                self.handle_tab(msg);
            }
            Message::Dock(msg) => {
                self.dock.update(msg);
            }
            Message::FileOpened(Some(path)) => {
                // Parse the schematic file
                match kicad_parser::parse_schematic_file(&path) {
                    Ok(sheet) => {
                        self.project_path = Some(path.clone());
                        let title = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Untitled".to_string());
                        if let Some(tab) = self.tabs.first_mut() {
                            tab.title = title;
                        }
                        self.schematic = Some(sheet.clone());
                        self.canvas.schematic = Some(sheet);
                        self.canvas.clear_bg_cache();
                        self.canvas.clear_content_cache();
                        self.refresh_panel_ctx();
                    }
                    Err(e) => {
                        eprintln!("Failed to parse schematic: {e}");
                    }
                }
            }
            Message::FileOpened(None) => {
                // User cancelled file dialog
            }
            Message::SchematicLoaded(sheet) => {
                self.schematic = Some(*sheet);
                self.canvas.clear_content_cache();
            }
            Message::Noop => {}
        }
        Task::none()
    }

    fn handle_menu(&mut self, msg: MenuMessage) -> Task<Message> {
        match msg {
            MenuMessage::ThemeSelected(id) => {
                self.theme_id = id;
                self.update_canvas_theme();
            }
            MenuMessage::NewProject => {}
            MenuMessage::OpenProject => {
                return Task::perform(
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
                );
            }
            MenuMessage::Save => {}
            MenuMessage::Undo => {}
            MenuMessage::Redo => {}
            MenuMessage::ZoomFit => {
                self.zoom = 100.0;
            }
        }
        Task::none()
    }

    fn handle_tab(&mut self, msg: TabMessage) {
        match msg {
            TabMessage::Select(idx) => {
                if idx < self.tabs.len() {
                    self.active_tab = idx;
                }
            }
            TabMessage::Close(idx) => {
                if self.tabs.len() > 1 && idx < self.tabs.len() {
                    self.tabs.remove(idx);
                    if self.active_tab >= self.tabs.len() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                }
            }
        }
    }

    fn refresh_panel_ctx(&mut self) {
        self.panel_ctx = crate::panels::PanelContext {
            project_name: self.project_path.as_ref().and_then(|p| {
                p.file_stem().map(|s| s.to_string_lossy().to_string())
            }),
            project_file: self.project_path.as_ref().and_then(|p| {
                p.file_name().map(|s| s.to_string_lossy().to_string())
            }),
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
        };
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
        let menu = menu_bar::view(self.theme_id).map(Message::Menu);
        let tools = toolbar::view(self.current_tool).map(Message::Tool);
        let tabs = tab_bar::view(&self.tabs, self.active_tab).map(Message::Tab);

        // Left panel
        let left_panel = self.dock.view_region(PanelPosition::Left, &self.panel_ctx).map(Message::Dock);
        let left = container(left_panel)
            .width(220)
            .height(Length::Fill)
            .style(crate::styles::panel_region);

        // Center — live canvas
        let canvas_widget = canvas(&self.canvas)
            .width(Length::Fill)
            .height(Length::Fill);

        // Right panel
        let right_panel = self.dock.view_region(PanelPosition::Right, &self.panel_ctx).map(Message::Dock);
        let right = container(right_panel)
            .width(220)
            .height(Length::Fill)
            .style(crate::styles::panel_region);

        // Center row: left | canvas | right
        let center_row = row![left, canvas_widget, right];

        // Bottom panel
        let bottom_panel = self.dock.view_region(PanelPosition::Bottom, &self.panel_ctx).map(Message::Dock);
        let bottom = container(bottom_panel)
            .width(Length::Fill)
            .height(120)
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

        column![menu, tools, tabs, center_row, bottom, status].into()
    }
}
