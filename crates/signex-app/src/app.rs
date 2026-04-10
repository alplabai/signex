//! Main Iced application — Message enum, update loop, view tree.

use iced::widget::{column, container, row, text};
use iced::{Element, Length, Subscription, Task, Theme};
use signex_types::coord::Unit;
use signex_types::theme::ThemeId;

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
    ThemeChanged(ThemeId),
    UnitCycled,
    GridToggle,
    GridCycle,
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
        // Register default panels
        dock.add_panel(PanelPosition::Left, PanelKind::Projects);
        dock.add_panel(PanelPosition::Left, PanelKind::Components);
        dock.add_panel(PanelPosition::Right, PanelKind::Properties);
        dock.add_panel(PanelPosition::Bottom, PanelKind::Messages);
        dock.add_panel(PanelPosition::Bottom, PanelKind::Signal);

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
        keyboard::listen().map(|event| {
            match event {
                keyboard::Event::KeyPressed { key, modifiers: m, .. } => {
                    match (key.as_ref(), m) {
                        // Ctrl+Q — cycle units
                        (keyboard::Key::Character(c), m) if c == "q" && m.command() => {
                            Message::UnitCycled
                        }
                        // G — cycle grid
                        (keyboard::Key::Character(c), m) if c == "g" && !m.command() && !m.shift() => {
                            Message::GridCycle
                        }
                        // W — wire tool
                        (keyboard::Key::Character(c), m) if c == "w" && !m.command() => {
                            Message::Tool(ToolMessage::SelectTool(Tool::Wire))
                        }
                        // B — bus tool
                        (keyboard::Key::Character(c), m) if c == "b" && !m.command() => {
                            Message::Tool(ToolMessage::SelectTool(Tool::Bus))
                        }
                        // L — label tool
                        (keyboard::Key::Character(c), m) if c == "l" && !m.command() => {
                            Message::Tool(ToolMessage::SelectTool(Tool::Label))
                        }
                        // P — component placement
                        (keyboard::Key::Character(c), m) if c == "p" && !m.command() => {
                            Message::Tool(ToolMessage::SelectTool(Tool::Component))
                        }
                        // Escape — back to select
                        (keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
                            Message::Tool(ToolMessage::SelectTool(Tool::Select))
                        }
                        _ => Message::Noop,
                    }
                }
                _ => Message::Noop,
            }
        })
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeChanged(id) => {
                self.theme_id = id;
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
            }
            Message::GridCycle => {
                // Grid size cycling — will be implemented with canvas
            }
            Message::StatusBar(StatusBarMsg::ToggleSnap) => {
                self.snap_enabled = !self.snap_enabled;
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
            Message::Noop => {}
        }
        Task::none()
    }

    fn handle_menu(&mut self, msg: MenuMessage) -> Task<Message> {
        match msg {
            MenuMessage::ThemeSelected(id) => {
                self.theme_id = id;
            }
            MenuMessage::NewProject => {}
            MenuMessage::OpenProject => {}
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

    pub fn view(&self) -> Element<'_, Message> {
        let menu = menu_bar::view(self.theme_id).map(Message::Menu);
        let tools = toolbar::view(self.current_tool).map(Message::Tool);
        let tabs = tab_bar::view(&self.tabs, self.active_tab).map(Message::Tab);

        // Left panel
        let left_panel = self.dock.view_region(PanelPosition::Left).map(Message::Dock);
        let left = container(left_panel)
            .width(220)
            .height(Length::Fill)
            .style(container::bordered_box);

        // Center — canvas area (placeholder for now)
        let canvas_area = container(
            text("Canvas — Phase 2 (v0.3.0)").size(14),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(container::bordered_box);

        // Right panel
        let right_panel = self.dock.view_region(PanelPosition::Right).map(Message::Dock);
        let right = container(right_panel)
            .width(220)
            .height(Length::Fill)
            .style(container::bordered_box);

        // Center row: left | canvas | right
        let center_row = row![left, canvas_area, right];

        // Bottom panel
        let bottom_panel = self.dock.view_region(PanelPosition::Bottom).map(Message::Dock);
        let bottom = container(bottom_panel)
            .width(Length::Fill)
            .height(160)
            .style(container::bordered_box);

        // Status bar
        let status = status_bar::view(
            self.cursor_x,
            self.cursor_y,
            self.grid_visible,
            self.snap_enabled,
            self.zoom,
            self.unit,
            &self.current_tool,
        )
        .map(Message::StatusBar);

        column![menu, tools, tabs, center_row, bottom, status].into()
    }
}
