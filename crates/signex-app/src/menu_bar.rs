//! Top menu bar using iced_aw MenuBar with proper dropdown/submenu support.
//!
//! Altium-style menu structure: File, Edit, View, Place, Design, Tools, Window, Help.
//! iced_aw handles all overlay positioning, hover-to-switch, and keyboard navigation.

use iced::widget::{button, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use iced_aw::menu::{Item, Menu, MenuBar};
use iced_aw::style::menu_bar as menu_style;
use signex_types::theme::ThemeTokens;

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
    Cut,
    Copy,
    Paste,
    SmartPaste,
    Delete,
    SelectAll,
    Duplicate,
    Find,
    Replace,
    // View
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ToggleGrid,
    CycleGrid,
    OpenProjectsPanel,
    OpenComponentsPanel,
    OpenNavigatorPanel,
    OpenPropertiesPanel,
    OpenMessagesPanel,
    OpenSignalPanel,
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

/// Extracted theme colors (all Copy+ʼstatic so closures remain ʼstatic).
#[derive(Clone, Copy)]
struct MenuColors {
    text: Color,
    text_muted: Color,
    text_disabled: Color,
    toolbar_bg: Color,
    panel_bg: Color,
    border: Color,
    hover: Color,
}

impl MenuColors {
    fn from_tokens(tokens: &ThemeTokens) -> Self {
        Self {
            text: styles::ti(tokens.text),
            text_muted: styles::ti(tokens.text_secondary),
            text_disabled: {
                let t = styles::ti(tokens.text_secondary);
                Color { a: t.a * 0.6, ..t }
            },
            toolbar_bg: styles::ti(tokens.toolbar_bg),
            panel_bg: styles::ti(tokens.paper),
            border: styles::ti(tokens.border),
            hover: styles::ti(tokens.hover),
        }
    }
}

// ─── View: Menu Bar ──────────────────────────────────────────

pub fn view(tokens: &ThemeTokens) -> Element<'static, MenuMessage> {
    let mc = MenuColors::from_tokens(tokens);

    let menu_template = |items| {
        Menu::new(items)
            .max_width(DROPDOWN_WIDTH)
            .offset(4.0)
            .spacing(2.0)
    };

    let file_menu = Item::with_menu(
        root_btn("File", mc),
        menu_template(vec![
            leaf_stub("New Project", Some("Ctrl+N"), mc),
            leaf("Open...", Some("Ctrl+O"), MenuMessage::OpenProject, mc),
            separator(mc),
            leaf("Save", Some("Ctrl+S"), MenuMessage::Save, mc),
            leaf("Save As...", Some("Ctrl+Shift+S"), MenuMessage::SaveAs, mc),
            separator(mc),
            leaf_stub("Exit", None, mc),
        ]),
    );

    let edit_menu = Item::with_menu(
        root_btn("Edit", mc),
        menu_template(vec![
            leaf("Undo", Some("Ctrl+Z"), MenuMessage::Undo, mc),
            leaf("Redo", Some("Ctrl+Y"), MenuMessage::Redo, mc),
            separator(mc),
            leaf("Cut", Some("Ctrl+X"), MenuMessage::Cut, mc),
            leaf("Copy", Some("Ctrl+C"), MenuMessage::Copy, mc),
            leaf("Paste", Some("Ctrl+V"), MenuMessage::Paste, mc),
            leaf(
                "Smart Paste",
                Some("Shift+Ctrl+V"),
                MenuMessage::SmartPaste,
                mc,
            ),
            leaf("Duplicate", Some("Ctrl+D"), MenuMessage::Duplicate, mc),
            leaf("Delete", Some("Del"), MenuMessage::Delete, mc),
            separator(mc),
            leaf("Select All", Some("Ctrl+A"), MenuMessage::SelectAll, mc),
            separator(mc),
            leaf("Find", Some("Ctrl+F"), MenuMessage::Find, mc),
            leaf("Find and Replace", Some("Ctrl+H"), MenuMessage::Replace, mc),
        ]),
    );

    let view_menu = Item::with_menu(
        root_btn("View", mc),
        menu_template(vec![
            leaf_stub("Zoom In", Some("Ctrl+="), mc),
            leaf_stub("Zoom Out", Some("Ctrl+-"), mc),
            leaf("Fit All", Some("Home"), MenuMessage::ZoomFit, mc),
            separator(mc),
            leaf(
                "Toggle Grid",
                Some("Shift+Ctrl+G"),
                MenuMessage::ToggleGrid,
                mc,
            ),
            leaf("Cycle Grid Size", Some("G"), MenuMessage::CycleGrid, mc),
            separator(mc),
            leaf("Projects", None, MenuMessage::OpenProjectsPanel, mc),
            leaf("Components", None, MenuMessage::OpenComponentsPanel, mc),
            leaf("Navigator", None, MenuMessage::OpenNavigatorPanel, mc),
            leaf("Properties", None, MenuMessage::OpenPropertiesPanel, mc),
            leaf("Messages", None, MenuMessage::OpenMessagesPanel, mc),
            leaf("Signal", None, MenuMessage::OpenSignalPanel, mc),
        ]),
    );

    let place_menu = Item::with_menu(
        root_btn("Place", mc),
        menu_template(vec![
            leaf("Wire", Some("W"), MenuMessage::PlaceWire, mc),
            leaf("Bus", Some("B"), MenuMessage::PlaceBus, mc),
            leaf("Net Label", Some("L"), MenuMessage::PlaceLabel, mc),
            separator(mc),
            leaf("Component...", Some("P"), MenuMessage::PlaceComponent, mc),
            leaf_stub("Power Port", None, mc),
            separator(mc),
            leaf_stub("Text", None, mc),
            leaf_stub("No Connect", None, mc),
            leaf_stub("Sheet Entry", None, mc),
        ]),
    );

    let design_menu = Item::with_menu(
        root_btn("Design", mc),
        menu_template(vec![
            leaf_stub("Annotate Schematics", None, mc),
            separator(mc),
            leaf_stub("Electrical Rules Check", None, mc),
            leaf_stub("Generate BOM", None, mc),
            leaf_stub("Generate Netlist", None, mc),
        ]),
    );

    let tools_menu = Item::with_menu(
        root_btn("Tools", mc),
        menu_template(vec![
            leaf_stub("Assign Footprints", None, mc),
            leaf_stub("Library Editor", None, mc),
            separator(mc),
            leaf_stub("Design Rule Check", None, mc),
            leaf_stub("Net Inspector", None, mc),
            separator(mc),
            leaf(
                "Preferences...",
                Some("Ctrl+,"),
                MenuMessage::OpenPreferences,
                mc,
            ),
        ]),
    );

    let window_menu = Item::with_menu(
        root_btn("Window", mc),
        menu_template(vec![
            leaf_stub("Tile Horizontally", None, mc),
            leaf_stub("Tile Vertically", None, mc),
            separator(mc),
            leaf_stub("Close All Documents", None, mc),
        ]),
    );

    let help_menu = Item::with_menu(
        root_btn("Help", mc),
        menu_template(vec![
            leaf_stub("About Signex", None, mc),
            separator(mc),
            leaf_stub("Keyboard Shortcuts", None, mc),
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
    .style(move |_theme: &Theme, _status| menu_style::Style {
        bar_background: Background::Color(mc.toolbar_bg),
        bar_border: Border::default(),
        bar_shadow: iced::Shadow::default(),
        menu_background: Background::Color(mc.panel_bg),
        menu_border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: mc.border,
        },
        menu_shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: iced::Vector::new(2.0, 4.0),
            blur_radius: 8.0,
        },
        path: Background::Color(mc.hover),
        path_border: Border::default(),
    });

    container(row![mb].align_y(iced::Alignment::Center))
        .width(Length::Fill)
        .style(styles::toolbar_strip(tokens))
        .into()
}

// ─── Private helpers ─────────────────────────────────────────

/// Root-level menu button (top bar).
fn root_btn(label: &str, mc: MenuColors) -> Element<'static, MenuMessage> {
    let label = label.to_owned();
    button(text(label).size(12).color(mc.text))
        .padding([3, 10])
        .style(button::text)
        .into()
}

/// Leaf menu item with an action.
fn leaf(
    label: &str,
    shortcut: Option<&str>,
    msg: MenuMessage,
    mc: MenuColors,
) -> Item<'static, MenuMessage, Theme, iced::Renderer> {
    Item::new(menu_item_btn(label, shortcut, Some(msg), mc))
}

/// Leaf menu item — disabled/stub (no action yet).
fn leaf_stub(
    label: &str,
    shortcut: Option<&str>,
    mc: MenuColors,
) -> Item<'static, MenuMessage, Theme, iced::Renderer> {
    Item::new(menu_item_btn(label, shortcut, None, mc))
}

/// Separator line between menu sections.
fn separator(mc: MenuColors) -> Item<'static, MenuMessage, Theme, iced::Renderer> {
    Item::new(
        container(iced::widget::Space::new())
            .height(1)
            .width(Length::Fill)
            .padding([2, 8])
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(mc.border)),
                ..container::Style::default()
            }),
    )
}

/// Build a single menu item button with label + shortcut text.
fn menu_item_btn(
    label: &str,
    shortcut: Option<&str>,
    msg: Option<MenuMessage>,
    mc: MenuColors,
) -> Element<'static, MenuMessage> {
    let enabled = msg.is_some();
    let text_c = if enabled { mc.text } else { mc.text_disabled };

    let label = label.to_owned();
    let mut r = row![
        text(label)
            .size(12)
            .color(text_c)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);

    if let Some(sc) = shortcut {
        let sc = sc.to_owned();
        r = r.push(iced::widget::Space::new().width(Length::Fill));
        r = r.push(
            text(sc)
                .size(11)
                .color(mc.text_muted)
                .wrapping(iced::widget::text::Wrapping::None),
        );
    }

    let hover_bg = mc.hover;
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
