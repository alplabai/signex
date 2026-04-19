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
    AnnotateQuietly,
    AnnotateReset,
    AnnotateResetDuplicates,
    AnnotateForceAll,
    AnnotateBack,
    AnnotateSheets,
    Erc,
    ToggleAutoFocus,
    GenerateBom,
    // Tools
    /// Open the Preferences dialog.
    OpenPreferences,
}

/// Context passed into `view` so each menu leaf can decide whether to
/// render as an active link or a disabled item. Keeps the menu
/// context-aware — e.g. Annotate / ERC / Save are unclickable when no
/// schematic is open.
#[derive(Debug, Clone, Copy, Default)]
pub struct MenuContext {
    pub has_schematic: bool,
    pub has_pcb: bool,
    /// Reserved for guarding project-wide items (e.g. multi-sheet
    /// navigator, BOM across project) when those land. Currently no
    /// menu entry reads this but the field stays so callers don't
    /// need to update their struct literal when we wire it up.
    #[allow(dead_code)]
    pub has_project: bool,
    pub has_selection: bool,
    pub can_undo: bool,
    pub can_redo: bool,
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

pub fn view(tokens: &ThemeTokens, ctx: MenuContext) -> Element<'static, MenuMessage> {
    let mc = MenuColors::from_tokens(tokens);
    // `leaf_if(enabled, ..)` wraps `leaf`/`leaf_stub` — enabled items
    // dispatch their message, disabled items render greyed-out like
    // the stub entries so Annotate / ERC / Save can't fire when no
    // schematic is loaded.
    let leaf_if = |label: &str,
                   shortcut: Option<&str>,
                   msg: MenuMessage,
                   enabled: bool|
     -> Item<'static, MenuMessage, Theme, iced::Renderer> {
        if enabled {
            leaf(label, shortcut, msg, mc)
        } else {
            leaf_stub(label, shortcut, mc)
        }
    };

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
            leaf_if("Save", Some("Ctrl+S"), MenuMessage::Save, ctx.has_schematic),
            leaf_if(
                "Save As...",
                Some("Ctrl+Shift+S"),
                MenuMessage::SaveAs,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_stub("Exit", None, mc),
        ]),
    );

    let edit_menu = Item::with_menu(
        root_btn("Edit", mc),
        menu_template(vec![
            leaf_if("Undo", Some("Ctrl+Z"), MenuMessage::Undo, ctx.can_undo),
            leaf_if("Redo", Some("Ctrl+Y"), MenuMessage::Redo, ctx.can_redo),
            separator(mc),
            leaf_if("Cut", Some("Ctrl+X"), MenuMessage::Cut, ctx.has_selection),
            leaf_if("Copy", Some("Ctrl+C"), MenuMessage::Copy, ctx.has_selection),
            leaf_if(
                "Paste",
                Some("Ctrl+V"),
                MenuMessage::Paste,
                ctx.has_schematic,
            ),
            leaf_if(
                "Smart Paste",
                Some("Shift+Ctrl+V"),
                MenuMessage::SmartPaste,
                ctx.has_schematic,
            ),
            leaf_if(
                "Duplicate",
                Some("Ctrl+D"),
                MenuMessage::Duplicate,
                ctx.has_selection,
            ),
            leaf_if(
                "Delete",
                Some("Del"),
                MenuMessage::Delete,
                ctx.has_selection,
            ),
            separator(mc),
            leaf_if(
                "Select All",
                Some("Ctrl+A"),
                MenuMessage::SelectAll,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_if("Find", Some("Ctrl+F"), MenuMessage::Find, ctx.has_schematic),
            leaf_if(
                "Find and Replace",
                Some("Ctrl+H"),
                MenuMessage::Replace,
                ctx.has_schematic,
            ),
        ]),
    );

    let view_menu = Item::with_menu(
        root_btn("View", mc),
        menu_template(vec![
            leaf_stub("Zoom In", Some("Ctrl+="), mc),
            leaf_stub("Zoom Out", Some("Ctrl+-"), mc),
            leaf_if(
                "Fit All",
                Some("Home"),
                MenuMessage::ZoomFit,
                ctx.has_schematic || ctx.has_pcb,
            ),
            separator(mc),
            leaf_if(
                "Toggle Grid",
                Some("Shift+Ctrl+G"),
                MenuMessage::ToggleGrid,
                ctx.has_schematic || ctx.has_pcb,
            ),
            leaf_if(
                "Cycle Grid Size",
                Some("G"),
                MenuMessage::CycleGrid,
                ctx.has_schematic || ctx.has_pcb,
            ),
            leaf_if(
                "AutoFocus (dim unselected)",
                Some("F9"),
                MenuMessage::ToggleAutoFocus,
                ctx.has_schematic,
            ),
            separator(mc),
            // Panel-open entries are always available — panels work
            // without an active document (show empty state).
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
            leaf_if("Wire", Some("W"), MenuMessage::PlaceWire, ctx.has_schematic),
            leaf_if("Bus", Some("B"), MenuMessage::PlaceBus, ctx.has_schematic),
            leaf_if(
                "Net Label",
                Some("L"),
                MenuMessage::PlaceLabel,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_if(
                "Component...",
                Some("P"),
                MenuMessage::PlaceComponent,
                ctx.has_schematic,
            ),
            leaf_stub("Power Port", None, mc),
            separator(mc),
            leaf_stub("Text", None, mc),
            leaf_stub("No Connect", None, mc),
            leaf_stub("Sheet Entry", None, mc),
        ]),
    );

    // Design → Annotation submenu mirrors Altium's Annotation cascade.
    // Every entry gated on `has_schematic` — annotating without a
    // project open is nonsense.
    let annotation_submenu: Item<'static, MenuMessage, Theme, iced::Renderer> = Item::with_menu(
        submenu_item_btn("Annotation", mc),
        menu_template(vec![
            leaf_if(
                "Annotate Schematics...",
                None,
                MenuMessage::Annotate,
                ctx.has_schematic,
            ),
            leaf_if(
                "Reset Schematic Designators...",
                None,
                MenuMessage::AnnotateReset,
                ctx.has_schematic,
            ),
            leaf_if(
                "Reset Duplicate Schematic Designators...",
                None,
                MenuMessage::AnnotateResetDuplicates,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_if(
                "Annotate Schematics Quietly",
                Some("Alt+A"),
                MenuMessage::AnnotateQuietly,
                ctx.has_schematic,
            ),
            leaf_if(
                "Force Annotate All Schematics",
                Some("Shift+Alt+A"),
                MenuMessage::AnnotateForceAll,
                ctx.has_schematic,
            ),
            separator(mc),
            leaf_stub("Back Annotate Schematics...", None, mc),
            leaf_stub("Number Schematic Sheets...", None, mc),
        ]),
    );

    let design_menu = Item::with_menu(
        root_btn("Design", mc),
        menu_template(vec![
            annotation_submenu,
            separator(mc),
            leaf_if(
                "Electrical Rules Check",
                Some("F8"),
                MenuMessage::Erc,
                ctx.has_schematic,
            ),
            separator(mc),
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

/// Menu row that acts as a submenu header — label on the left, right
/// chevron on the right, no shortcut. Does not dispatch on click; the
/// menu framework opens the nested submenu on hover.
fn submenu_item_btn(label: &str, mc: MenuColors) -> Element<'static, MenuMessage> {
    let label = label.to_owned();
    let r = row![
        text(label).size(12).color(mc.text),
        iced::widget::Space::new().width(Length::Fill),
        text("›".to_string()).size(14).color(mc.text_muted),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    button(r)
        .padding([4, 10])
        .width(Length::Fill)
        .style(move |_: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered => {
                    Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06)))
                }
                _ => None,
            };
            button::Style {
                background: bg,
                border: Border::default(),
                text_color: mc.text,
                ..button::Style::default()
            }
        })
        .into()
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
