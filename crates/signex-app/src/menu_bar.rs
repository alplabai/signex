//! Top menu bar using iced_aw MenuBar with proper dropdown/submenu support.
//!
//! Altium-style menu structure: File, Edit, View, Place, Design, Tools, Window, Help.
//! iced_aw handles all overlay positioning, hover-to-switch, and keyboard navigation.

use iced::widget::{button, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use iced_aw::menu::{Item, Menu, MenuBar};
use iced_aw::style::menu_bar as menu_style;

use crate::styles;

// ─── Messages ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum MenuMessage {
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
    // Tools
    /// Open the Preferences dialog.
    OpenPreferences,
}

// ─── Constants ────────────────────────────────────────────────

pub const MENU_BAR_HEIGHT: f32 = 28.0;
const DROPDOWN_WIDTH: f32 = 240.0;

// ─── View: Menu Bar ──────────────────────────────────────────

pub fn view() -> Element<'static, MenuMessage> {
    let menu_template =
        |items| Menu::new(items).max_width(DROPDOWN_WIDTH).offset(4.0).spacing(2.0);

    let file_menu = Item::with_menu(
        root_btn("File"),
        menu_template(vec![
            leaf_stub("New Project", Some("Ctrl+N")),
            leaf("Open...", Some("Ctrl+O"), MenuMessage::OpenProject),
            separator(),
            leaf("Save", Some("Ctrl+S"), MenuMessage::Save),
            leaf_stub("Save As...", Some("Ctrl+Shift+S")),
            separator(),
            leaf_stub("Exit", None),
        ]),
    );

    let edit_menu = Item::with_menu(
        root_btn("Edit"),
        menu_template(vec![
            leaf("Undo", Some("Ctrl+Z"), MenuMessage::Undo),
            leaf("Redo", Some("Ctrl+Y"), MenuMessage::Redo),
            separator(),
            leaf_stub("Cut", Some("Ctrl+X")),
            leaf_stub("Copy", Some("Ctrl+C")),
            leaf_stub("Paste", Some("Ctrl+V")),
            leaf_stub("Delete", Some("Del")),
            separator(),
            leaf_stub("Select All", Some("Ctrl+A")),
        ]),
    );

    let view_menu = Item::with_menu(
        root_btn("View"),
        menu_template(vec![
            leaf_stub("Zoom In", Some("Ctrl+=")),
            leaf_stub("Zoom Out", Some("Ctrl+-")),
            leaf("Fit All", Some("Home"), MenuMessage::ZoomFit),
            separator(),
            leaf("Toggle Grid", Some("Shift+Ctrl+G"), MenuMessage::ToggleGrid),
            leaf("Cycle Grid Size", Some("G"), MenuMessage::CycleGrid),
            separator(),
            leaf_stub("Libraries", None),
            leaf_stub("Properties", None),
            leaf_stub("Messages", None),
        ]),
    );

    let place_menu = Item::with_menu(
        root_btn("Place"),
        menu_template(vec![
            leaf("Wire", Some("W"), MenuMessage::PlaceWire),
            leaf("Bus", Some("B"), MenuMessage::PlaceBus),
            leaf("Net Label", Some("L"), MenuMessage::PlaceLabel),
            separator(),
            leaf("Component...", Some("P"), MenuMessage::PlaceComponent),
            leaf_stub("Power Port", None),
            separator(),
            leaf_stub("Text", None),
            leaf_stub("No Connect", None),
            leaf_stub("Sheet Entry", None),
        ]),
    );

    let design_menu = Item::with_menu(
        root_btn("Design"),
        menu_template(vec![
            leaf_stub("Annotate Schematics", None),
            separator(),
            leaf_stub("Electrical Rules Check", None),
            leaf_stub("Generate BOM", None),
            leaf_stub("Generate Netlist", None),
        ]),
    );

    let tools_menu = Item::with_menu(
        root_btn("Tools"),
        menu_template(vec![
            leaf_stub("Assign Footprints", None),
            leaf_stub("Library Editor", None),
            separator(),
            leaf_stub("Design Rule Check", None),
            leaf_stub("Net Inspector", None),
            separator(),
            leaf("Preferences...", Some("Ctrl+,"), MenuMessage::OpenPreferences),
        ]),
    );

    let window_menu = Item::with_menu(
        root_btn("Window"),
        menu_template(vec![
            leaf_stub("Tile Horizontally", None),
            leaf_stub("Tile Vertically", None),
            separator(),
            leaf_stub("Close All Documents", None),
        ]),
    );

    let help_menu = Item::with_menu(
        root_btn("Help"),
        menu_template(vec![
            leaf_stub("About Signex", None),
            separator(),
            leaf_stub("Keyboard Shortcuts", None),
        ]),
    );

    let mb: MenuBar<'static, MenuMessage, Theme, iced::Renderer> = MenuBar::new(vec![
        file_menu,
        edit_menu,
        view_menu,
        place_menu,
        design_menu,
        tools_menu,
        window_menu,
        help_menu,
    ])
    .spacing(1.0)
    .padding([1, 4])
    .close_on_item_click_global(true)
    .close_on_background_click_global(true)
    .style(|_theme: &Theme, _status| menu_style::Style {
        bar_background: Background::Color(styles::TOOLBAR_BG),
        bar_border: Border::default(),
        bar_shadow: iced::Shadow::default(),
        menu_background: Background::Color(Color::from_rgb(0.12, 0.12, 0.14)),
        menu_border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: styles::BORDER_COLOR,
        },
        menu_shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: iced::Vector::new(2.0, 4.0),
            blur_radius: 8.0,
        },
        path: Background::Color(styles::TAB_ACTIVE_BG),
        path_border: Border::default(),
    });

    container(
        row![mb]
            .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .style(styles::toolbar_strip)
    .into()
}

// ─── Private helpers ─────────────────────────────────────────

/// Root-level menu button (top bar).
fn root_btn(label: &str) -> Element<'static, MenuMessage> {
    button(text(label.to_owned()).size(12).color(styles::TEXT_PRIMARY))
        .padding([3, 10])
        .style(button::text)
        .into()
}

/// Leaf menu item with an action.
fn leaf(label: &str, shortcut: Option<&str>, msg: MenuMessage) -> Item<'static, MenuMessage, Theme, iced::Renderer> {
    Item::new(menu_item_btn(label, shortcut, Some(msg)))
}

/// Leaf menu item — disabled/stub (no action yet).
fn leaf_stub(label: &str, shortcut: Option<&str>) -> Item<'static, MenuMessage, Theme, iced::Renderer> {
    Item::new(menu_item_btn(label, shortcut, None))
}

/// Separator line between menu sections.
fn separator() -> Item<'static, MenuMessage, Theme, iced::Renderer> {
    Item::new(
        container(iced::widget::Space::new())
            .height(1)
            .width(Length::Fill)
            .padding([2, 8])
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(styles::BORDER_SUBTLE)),
                ..container::Style::default()
            }),
    )
}

/// Build a single menu item button with label + shortcut text.
fn menu_item_btn(
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

    let mut r = row![
        text(label.to_owned())
            .size(12)
            .color(text_c)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);

    if let Some(sc) = shortcut {
        r = r.push(iced::widget::Space::new().width(Length::Fill));
        r = r.push(
            text(sc.to_owned())
                .size(11)
                .color(styles::TEXT_MUTED)
                .wrapping(iced::widget::text::Wrapping::None),
        );
    }

    let hover_bg = styles::TAB_ACTIVE_BG;
    let btn = button(r).padding([4, 12]).width(Length::Fill).style(
        move |_: &Theme, status: button::Status| {
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
        },
    );

    if let Some(m) = msg {
        btn.on_press(m).into()
    } else {
        btn.into()
    }
}
