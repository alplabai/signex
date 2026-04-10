//! Top menu bar with dropdown menus.
//!
//! Altium-style menu structure: File, Edit, View, Place, Design, Tools, Window, Help.
//! Dropdowns appear below the clicked button using Stack overlay in app.rs.

use iced::widget::{button, container, text, Column, Row};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeId;

use crate::styles;

// ─── Messages ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum MenuMessage {
    // Menu bar control
    OpenMenu(usize),
    CloseMenus,
    HoverMenu(usize),
    // File
    NewProject,
    OpenProject,
    Save,
    SaveAs,
    // Edit
    Undo,
    Redo,
    // View
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ToggleGrid,
    CycleGrid,
    // Place
    PlaceWire,
    PlaceBus,
    PlaceLabel,
    PlaceComponent,
    // Design
    Annotate,
    Erc,
    GenerateBom,
    // Settings
    ThemeSelected(ThemeId),
}

// ─── Constants ────────────────────────────────────────────────

pub const MENU_LABELS: &[&str] = &[
    "File", "Edit", "View", "Place", "Design", "Tools", "Window", "Help",
];

pub const MENU_BAR_HEIGHT: f32 = 28.0;
pub const DROPDOWN_WIDTH: f32 = 240.0;

/// Approximate cumulative x-offset for dropdown positioning.
/// Based on label widths at 12px font + [3,10] padding + 1px spacing.
pub fn button_x_offset(idx: usize) -> f32 {
    const WIDTHS: [f32; 8] = [48.0, 46.0, 48.0, 52.0, 60.0, 52.0, 64.0, 48.0];
    WIDTHS.iter().take(idx).sum::<f32>() + 6.0 // container left padding
}

// ─── Menu Entries ─────────────────────────────────────────────

enum Entry {
    Item {
        label: &'static str,
        shortcut: Option<&'static str>,
        msg: Option<MenuMessage>,
    },
    Sep,
}

impl Entry {
    fn item(label: &'static str, shortcut: &'static str, msg: MenuMessage) -> Self {
        Self::Item {
            label,
            shortcut: Some(shortcut),
            msg: Some(msg),
        }
    }
    fn stub(label: &'static str, shortcut: Option<&'static str>) -> Self {
        Self::Item {
            label,
            shortcut,
            msg: None,
        }
    }
}

fn menu_entries(idx: usize) -> Vec<Entry> {
    match idx {
        0 => vec![
            // File
            Entry::stub("New Project", Some("Ctrl+N")),
            Entry::item("Open...", "Ctrl+O", MenuMessage::OpenProject),
            Entry::Sep,
            Entry::Item {
                label: "Save",
                shortcut: Some("Ctrl+S"),
                msg: Some(MenuMessage::Save),
            },
            Entry::stub("Save As...", Some("Ctrl+Shift+S")),
            Entry::Sep,
            Entry::stub("Exit", None),
        ],
        1 => vec![
            // Edit
            Entry::Item {
                label: "Undo",
                shortcut: Some("Ctrl+Z"),
                msg: Some(MenuMessage::Undo),
            },
            Entry::Item {
                label: "Redo",
                shortcut: Some("Ctrl+Y"),
                msg: Some(MenuMessage::Redo),
            },
            Entry::Sep,
            Entry::stub("Cut", Some("Ctrl+X")),
            Entry::stub("Copy", Some("Ctrl+C")),
            Entry::stub("Paste", Some("Ctrl+V")),
            Entry::stub("Delete", Some("Del")),
            Entry::Sep,
            Entry::stub("Select All", Some("Ctrl+A")),
        ],
        2 => vec![
            // View
            Entry::stub("Zoom In", Some("Ctrl+=")),
            Entry::stub("Zoom Out", Some("Ctrl+-")),
            Entry::item("Fit All", "Home", MenuMessage::ZoomFit),
            Entry::Sep,
            Entry::item("Toggle Grid", "Shift+Ctrl+G", MenuMessage::ToggleGrid),
            Entry::item("Cycle Grid Size", "G", MenuMessage::CycleGrid),
            Entry::Sep,
            Entry::stub("Libraries", None),
            Entry::stub("Properties", None),
            Entry::stub("Messages", None),
        ],
        3 => vec![
            // Place
            Entry::item("Wire", "W", MenuMessage::PlaceWire),
            Entry::item("Bus", "B", MenuMessage::PlaceBus),
            Entry::item("Net Label", "L", MenuMessage::PlaceLabel),
            Entry::Sep,
            Entry::item("Component...", "P", MenuMessage::PlaceComponent),
            Entry::stub("Power Port", None),
            Entry::Sep,
            Entry::stub("Text", None),
            Entry::stub("No Connect", None),
            Entry::stub("Sheet Entry", None),
        ],
        4 => vec![
            // Design
            Entry::stub("Annotate Schematics", None),
            Entry::Sep,
            Entry::stub("Electrical Rules Check", None),
            Entry::stub("Generate BOM", None),
            Entry::stub("Generate Netlist", None),
        ],
        5 => vec![
            // Tools
            Entry::stub("Assign Footprints", None),
            Entry::stub("Library Editor", None),
            Entry::Sep,
            Entry::stub("Design Rule Check", None),
            Entry::stub("Net Inspector", None),
        ],
        6 => vec![
            // Window
            Entry::stub("Tile Horizontally", None),
            Entry::stub("Tile Vertically", None),
            Entry::Sep,
            Entry::stub("Close All Documents", None),
        ],
        7 => vec![
            // Help
            Entry::stub("About Signex", None),
            Entry::Sep,
            Entry::stub("Keyboard Shortcuts", None),
        ],
        _ => vec![],
    }
}

// ─── View: Menu Bar Row ───────────────────────────────────────

pub fn view(current_theme: ThemeId, active_menu: Option<usize>) -> Element<'static, MenuMessage> {
    let mut bar: Row<'static, MenuMessage> = Row::new()
        .spacing(1.0)
        .align_y(iced::Alignment::Center);

    for (i, label) in MENU_LABELS.iter().enumerate() {
        let is_active = active_menu == Some(i);
        let btn = button(
            text(*label)
                .size(12)
                .color(if is_active {
                    Color::WHITE
                } else {
                    styles::TEXT_PRIMARY
                }),
        )
        .padding([3, 10])
        .on_press(MenuMessage::OpenMenu(i));

        let btn = if is_active {
            btn.style(button::primary)
        } else {
            btn.style(button::text)
        };

        // Hover: when a menu is already open, hovering switches menus
        let area =
            iced::widget::mouse_area(btn).on_enter(MenuMessage::HoverMenu(i));
        bar = bar.push(area);
    }

    // Right-aligned theme selector
    bar = bar.push(iced::widget::space::horizontal());
    bar = bar.push(text("Theme:").size(10).color(styles::TEXT_MUTED));

    let theme_btn = |id: ThemeId, label: &'static str| {
        let is_active = id == current_theme;
        let text_c = if is_active {
            Color::WHITE
        } else {
            styles::TEXT_MUTED
        };
        button(text(label).size(10).color(text_c))
            .padding([2, 5])
            .on_press(MenuMessage::ThemeSelected(id))
            .style(move |_: &Theme, status: button::Status| {
                let bg = match (is_active, status) {
                    (true, _) => Some(Background::Color(styles::TAB_ACTIVE_BG)),
                    (false, button::Status::Hovered) => {
                        Some(Background::Color(styles::TAB_ACTIVE_BG))
                    }
                    _ => None,
                };
                button::Style {
                    background: bg,
                    border: Border::default(),
                    ..button::Style::default()
                }
            })
    };

    bar = bar.push(theme_btn(ThemeId::CatppuccinMocha, "Mocha"));
    bar = bar.push(theme_btn(ThemeId::VsCodeDark, "VS Code"));
    bar = bar.push(theme_btn(ThemeId::GitHubDark, "GitHub"));
    bar = bar.push(theme_btn(ThemeId::AltiumDark, "Altium"));
    bar = bar.push(theme_btn(ThemeId::SolarizedLight, "Solarized"));
    bar = bar.push(theme_btn(ThemeId::Nord, "Nord"));

    container(bar)
        .width(Length::Fill)
        .padding([1, 6])
        .style(styles::toolbar_strip)
        .into()
}

// ─── View: Dropdown Panel ─────────────────────────────────────

/// Render the dropdown column for the given menu index.
pub fn view_dropdown(menu_idx: usize) -> Element<'static, MenuMessage> {
    let entries = menu_entries(menu_idx);

    let mut col: Column<'static, MenuMessage> = Column::new()
        .spacing(2.0)
        .width(DROPDOWN_WIDTH as f32);

    for entry in &entries {
        match entry {
            Entry::Item {
                label,
                shortcut,
                msg,
            } => {
                col = col.push(view_menu_item(label, *shortcut, msg.clone()));
            }
            Entry::Sep => {
                col = col.push(view_separator());
            }
        }
    }

    container(col)
        .padding([4, 2])
        .style(dropdown_style)
        .into()
}

// ─── Private helpers ──────────────────────────────────────────

fn view_menu_item(
    label: &str,
    shortcut: Option<&str>,
    msg: Option<MenuMessage>,
) -> Element<'static, MenuMessage> {
    let enabled = msg.is_some();
    let text_c = if enabled {
        styles::TEXT_PRIMARY
    } else {
        Color::from_rgb(0.35, 0.35, 0.38)
    };

    let mut r: Row<'static, MenuMessage> = Row::new()
        .spacing(8.0)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

    r = r.push(
        text(label.to_string())
            .size(12)
            .color(text_c)
            .wrapping(iced::widget::text::Wrapping::None),
    );

    if let Some(sc) = shortcut {
        r = r.push(iced::widget::space::horizontal());
        r = r.push(
            text(sc.to_string())
                .size(11)
                .color(styles::TEXT_MUTED)
                .wrapping(iced::widget::text::Wrapping::None),
        );
    }

    let hover_bg = styles::TAB_ACTIVE_BG;
    let btn = button(r)
        .padding([4, 12])
        .width(Length::Fill)
        .style(move |_theme: &Theme, status: button::Status| {
            let bg = if enabled {
                match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Some(Background::Color(hover_bg))
                    }
                    _ => None,
                }
            } else {
                None
            };
            button::Style {
                background: bg,
                text_color: text_c,
                border: Border {
                    radius: 2.0.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        });

    if let Some(m) = msg {
        btn.on_press(m).into()
    } else {
        btn.into()
    }
}

fn view_separator() -> Element<'static, MenuMessage> {
    container(text(""))
        .height(1.0)
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(styles::BORDER_SUBTLE)),
            ..container::Style::default()
        })
        .into()
}

fn dropdown_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb(0.12, 0.12, 0.14))),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: styles::BORDER_COLOR,
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: iced::Vector::new(2.0, 4.0),
            blur_radius: 8.0,
        },
        ..container::Style::default()
    }
}
