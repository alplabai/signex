//! Altium-style Active Bar — floating toolbar centered at top of canvas.
//!
//! 12 icon buttons, each with an optional dropdown menu.
//! Matches Altium Designer's schematic editor Active Bar exactly.

use iced::widget::{button, column, container, row, svg, text, Space};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::app::Message;
use crate::styles;

// ─── Icon paths (embedded at compile time) ──────────────────

const ICON_FILTER: &[u8] = include_bytes!("../assets/icons/filter.svg");
const ICON_SELECT: &[u8] = include_bytes!("../assets/icons/select.svg");
const ICON_MOVE: &[u8] = include_bytes!("../assets/icons/move.svg");
const ICON_ALIGN: &[u8] = include_bytes!("../assets/icons/align.svg");
const ICON_WIRE: &[u8] = include_bytes!("../assets/icons/wire.svg");
const ICON_POWER: &[u8] = include_bytes!("../assets/icons/power.svg");
const ICON_HARNESS: &[u8] = include_bytes!("../assets/icons/harness.svg");
const ICON_PORT: &[u8] = include_bytes!("../assets/icons/port.svg");
const ICON_DIRECTIVES: &[u8] = include_bytes!("../assets/icons/directives.svg");
const ICON_TEXT: &[u8] = include_bytes!("../assets/icons/text.svg");
const ICON_SHAPES: &[u8] = include_bytes!("../assets/icons/shapes.svg");
const ICON_NETCOLOR: &[u8] = include_bytes!("../assets/icons/netcolor.svg");
const ICON_ADDPART: &[u8] = include_bytes!("../assets/icons/addpart.svg");
const ICON_NOCONNECT: &[u8] = include_bytes!("../assets/icons/noconnect.svg");
const ICON_COMPONENT: &[u8] = include_bytes!("../assets/icons/component.svg");
const ICON_SHEETSYM: &[u8] = include_bytes!("../assets/icons/sheetsym.svg");

// ─── Messages ────────────────────────────────────────────────

/// Which Active Bar dropdown menu is open (by button index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveBarMenu {
    Filter,     // 0
    Select,     // 1 (move/transform)
    Align,      // 2
    Wiring,     // 3
    Power,      // 4
    Harness,    // 5
    Port,       // 6
    Directives, // 7
    TextTools,  // 8
    Shapes,     // 9
    NetColor,   // 10
}

#[derive(Debug, Clone)]
pub enum ActiveBarMsg {
    ToggleMenu(ActiveBarMenu),
    CloseMenus,
    Action(ActiveBarAction),
}

/// All actions available from Active Bar buttons and dropdown items.
#[derive(Debug, Clone)]
pub enum ActiveBarAction {
    // Selection/Move
    ToolSelect,
    Drag,
    MoveSelection,
    RotateSelection,
    RotateSelectionCW,
    FlipSelectedX,
    FlipSelectedY,
    BringToFront,
    SendToBack,
    // Align
    AlignLeft,
    AlignRight,
    AlignHorizontalCenters,
    DistributeHorizontally,
    AlignTop,
    AlignBottom,
    AlignVerticalCenters,
    DistributeVertically,
    AlignToGrid,
    // Wiring
    DrawWire,
    DrawBus,
    PlaceBusEntry,
    PlaceNetLabel,
    // Power
    PlacePowerGND,
    PlacePowerVCC,
    PlacePowerPlus12,
    PlacePowerPlus5,
    PlacePowerMinus5,
    PlacePowerArrow,
    PlacePowerWave,
    PlacePowerBar,
    PlacePowerCircle,
    PlacePowerSignalGND,
    PlacePowerEarth,
    // Harness
    PlaceSignalHarness,
    PlaceHarnessConnector,
    PlaceHarnessEntry,
    // Port
    PlacePort,
    PlaceOffSheetConnector,
    // Directives
    PlaceParameterSet,
    PlaceNoERC,
    PlaceDiffPair,
    PlaceBlanket,
    PlaceCompileMask,
    // Text
    PlaceTextString,
    PlaceTextFrame,
    PlaceNote,
    // Shapes
    DrawArc,
    DrawFullCircle,
    DrawEllipticalArc,
    DrawEllipse,
    DrawLine,
    DrawRectangle,
    DrawRoundRectangle,
    DrawPolygon,
    DrawBezier,
    PlaceGraphic,
    // Net Color
    NetColorBlue,
    NetColorLightGreen,
    NetColorLightBlue,
    NetColorRed,
    NetColorFuchsia,
    NetColorYellow,
    NetColorDarkGreen,
    ClearNetColor,
    ClearAllNetColors,
    // Component
    PlaceComponent,
}

// ─── View: Active Bar + Dropdown ─────────────────────────────

/// Render the Active Bar with an optional dropdown below it, aligned in one container.
pub fn view_bar_with_dropdown(
    open_menu: Option<ActiveBarMenu>,
    current_tool: crate::app::Tool,
    draw_mode: crate::app::DrawMode,
) -> Element<'static, ActiveBarMsg> {
    let bar = view_bar(open_menu, current_tool, draw_mode);

    if let Some(menu) = open_menu {
        let dropdown = view_dropdown(menu);
        let x_off = dropdown_x_offset(menu);

        // Bar on top, dropdown below with horizontal offset to align under the button
        column![
            bar,
            row![Space::new().width(x_off), dropdown,].spacing(0),
        ]
        .spacing(2)
        .into()
    } else {
        bar
    }
}

// ─── View: Active Bar ────────────────────────────────────────

/// Render the Active Bar (the floating toolbar strip).
pub fn view_bar(
    open_menu: Option<ActiveBarMenu>,
    current_tool: crate::app::Tool,
    draw_mode: crate::app::DrawMode,
) -> Element<'static, ActiveBarMsg> {
    let mut items: Vec<Element<'_, ActiveBarMsg>> = Vec::new();

    // 1. Filter — click opens filter dropdown
    items.push(ab_icon_btn(ICON_FILTER, open_menu == Some(ActiveBarMenu::Filter),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Filter)));
    // 2. Add Component (+)
    items.push(ab_icon_btn(ICON_ADDPART,
        current_tool == crate::app::Tool::Component,
        ActiveBarMsg::Action(ActiveBarAction::PlaceComponent)));
    items.push(sep());

    // 3. Select
    items.push(ab_icon_btn(ICON_SELECT,
        current_tool == crate::app::Tool::Select,
        ActiveBarMsg::Action(ActiveBarAction::ToolSelect)));
    // 4. Move/Transform — click opens dropdown
    items.push(ab_icon_btn(ICON_MOVE, open_menu == Some(ActiveBarMenu::Select),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Select)));
    // 5. Align — click opens dropdown
    items.push(ab_icon_btn(ICON_ALIGN, open_menu == Some(ActiveBarMenu::Align),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align)));
    items.push(sep());

    // 6. Wiring — click opens dropdown
    items.push(ab_icon_btn(ICON_WIRE,
        current_tool == crate::app::Tool::Wire || current_tool == crate::app::Tool::Bus
            || open_menu == Some(ActiveBarMenu::Wiring),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Wiring)));
    // 7. Power — click opens dropdown
    items.push(ab_icon_btn(ICON_POWER, open_menu == Some(ActiveBarMenu::Power),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Power)));
    items.push(sep());

    // 8. Harness — click opens dropdown
    items.push(ab_icon_btn(ICON_HARNESS, open_menu == Some(ActiveBarMenu::Harness),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Harness)));
    // 9. Port — click opens dropdown
    items.push(ab_icon_btn(ICON_PORT, open_menu == Some(ActiveBarMenu::Port),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Port)));
    // 10. Directives — click opens dropdown
    items.push(ab_icon_btn(ICON_DIRECTIVES, open_menu == Some(ActiveBarMenu::Directives),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Directives)));
    items.push(sep());

    // 11. Text — click opens dropdown
    items.push(ab_icon_btn(ICON_TEXT,
        current_tool == crate::app::Tool::Text || open_menu == Some(ActiveBarMenu::TextTools),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::TextTools)));
    // 12. Shapes — click opens dropdown
    items.push(ab_icon_btn(ICON_SHAPES,
        matches!(current_tool, crate::app::Tool::Line | crate::app::Tool::Rectangle | crate::app::Tool::Circle)
            || open_menu == Some(ActiveBarMenu::Shapes),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Shapes)));
    // 13. Net Color — click opens dropdown
    items.push(ab_icon_btn(ICON_NETCOLOR, open_menu == Some(ActiveBarMenu::NetColor),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::NetColor)));

    // Draw mode indicator
    if matches!(
        current_tool,
        crate::app::Tool::Wire | crate::app::Tool::Bus
    ) {
        items.push(sep());
        let mode_label = match draw_mode {
            crate::app::DrawMode::Ortho90 => "90\u{00B0}",
            crate::app::DrawMode::Angle45 => "45\u{00B0}",
            crate::app::DrawMode::FreeAngle => "Any",
        };
        items.push(
            button(
                text(mode_label.to_string())
                    .size(10)
                    .color(Color::WHITE),
            )
            .padding([3, 5])
            .on_press(ActiveBarMsg::Action(ActiveBarAction::DrawWire)) // cycles draw mode
            .style(|_: &Theme, _| button::Style {
                background: Some(Background::Color(Color::from_rgb(0.22, 0.23, 0.30))),
                border: Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    color: Color::TRANSPARENT,
                },
                ..button::Style::default()
            })
            .into(),
        );
    }

    container(row(items).spacing(1).align_y(iced::Alignment::Center))
        .padding([3, 4])
        .style(|_: &Theme| container::Style {
            background: Some(Color::from_rgb(0.165, 0.176, 0.239).into()),
            text_color: Some(styles::TEXT_PRIMARY),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: Color::from_rgb(0.24, 0.25, 0.33),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 8.0,
            },
            ..container::Style::default()
        })
        .into()
}

// ─── View: Dropdown menus ────────────────────────────────────

/// Render the dropdown menu for the given Active Bar button.
pub fn view_dropdown(menu: ActiveBarMenu) -> Element<'static, ActiveBarMsg> {
    let items: Vec<Element<'_, ActiveBarMsg>> = match menu {
        ActiveBarMenu::Filter => vec![
            dd_item("All - On", ActiveBarAction::ToolSelect),
            dd_sep(),
            dd_item("Components", ActiveBarAction::ToolSelect),
            dd_item("Wires", ActiveBarAction::ToolSelect),
            dd_item("Buses", ActiveBarAction::ToolSelect),
            dd_item("Sheet Symbols", ActiveBarAction::ToolSelect),
            dd_item("Sheet Entries", ActiveBarAction::ToolSelect),
            dd_item("Net Labels", ActiveBarAction::ToolSelect),
            dd_item("Parameters", ActiveBarAction::ToolSelect),
            dd_item("Ports", ActiveBarAction::ToolSelect),
            dd_item("Power Ports", ActiveBarAction::ToolSelect),
            dd_item("Texts", ActiveBarAction::ToolSelect),
            dd_item("Drawing Objects", ActiveBarAction::ToolSelect),
            dd_item("Other", ActiveBarAction::ToolSelect),
        ],
        ActiveBarMenu::Select => vec![
            dd_item("Drag", ActiveBarAction::Drag),
            dd_item("Move", ActiveBarAction::MoveSelection),
            dd_item("Move Selection", ActiveBarAction::MoveSelection),
            dd_sep(),
            dd_item("Rotate Selection", ActiveBarAction::RotateSelection),
            dd_item("Rotate Selection Clockwise", ActiveBarAction::RotateSelectionCW),
            dd_sep(),
            dd_item("Bring To Front", ActiveBarAction::BringToFront),
            dd_item("Send To Back", ActiveBarAction::SendToBack),
            dd_sep(),
            dd_item("Flip Selected Sheet Symbols Along X", ActiveBarAction::FlipSelectedX),
            dd_item("Flip Selected Sheet Symbols Along Y", ActiveBarAction::FlipSelectedY),
        ],
        ActiveBarMenu::Align => vec![
            dd_item("Align Left", ActiveBarAction::AlignLeft),
            dd_item("Align Right", ActiveBarAction::AlignRight),
            dd_item("Align Horizontal Centers", ActiveBarAction::AlignHorizontalCenters),
            dd_item("Distribute Horizontally", ActiveBarAction::DistributeHorizontally),
            dd_sep(),
            dd_item("Align Top", ActiveBarAction::AlignTop),
            dd_item("Align Bottom", ActiveBarAction::AlignBottom),
            dd_item("Align Vertical Centers", ActiveBarAction::AlignVerticalCenters),
            dd_item("Distribute Vertically", ActiveBarAction::DistributeVertically),
            dd_sep(),
            dd_item("Align To Grid", ActiveBarAction::AlignToGrid),
        ],
        ActiveBarMenu::Wiring => vec![
            dd_item("Wire", ActiveBarAction::DrawWire),
            dd_item("Bus", ActiveBarAction::DrawBus),
            dd_item("Bus Entry", ActiveBarAction::PlaceBusEntry),
            dd_item("Net Label", ActiveBarAction::PlaceNetLabel),
        ],
        ActiveBarMenu::Power => vec![
            dd_item("Place GND power port", ActiveBarAction::PlacePowerGND),
            dd_item("Place VCC power port", ActiveBarAction::PlacePowerVCC),
            dd_item("Place +12 power port", ActiveBarAction::PlacePowerPlus12),
            dd_item("Place +5 power port", ActiveBarAction::PlacePowerPlus5),
            dd_item("Place -5 power port", ActiveBarAction::PlacePowerMinus5),
            dd_sep(),
            dd_item("Place Arrow style power port", ActiveBarAction::PlacePowerArrow),
            dd_item("Place Wave style power port", ActiveBarAction::PlacePowerWave),
            dd_item("Place Bar style power port", ActiveBarAction::PlacePowerBar),
            dd_item("Place Circle style power port", ActiveBarAction::PlacePowerCircle),
            dd_sep(),
            dd_item("Place Signal Ground power port", ActiveBarAction::PlacePowerSignalGND),
            dd_item("Place Earth power port", ActiveBarAction::PlacePowerEarth),
        ],
        ActiveBarMenu::Harness => vec![
            dd_item("Signal Harness", ActiveBarAction::PlaceSignalHarness),
            dd_item("Harness Connector", ActiveBarAction::PlaceHarnessConnector),
            dd_item("Harness Entry", ActiveBarAction::PlaceHarnessEntry),
        ],
        ActiveBarMenu::Port => vec![
            dd_item("Port", ActiveBarAction::PlacePort),
            dd_item("Off Sheet Connector", ActiveBarAction::PlaceOffSheetConnector),
        ],
        ActiveBarMenu::Directives => vec![
            dd_item("Parameter Set", ActiveBarAction::PlaceParameterSet),
            dd_item("Generic No ERC", ActiveBarAction::PlaceNoERC),
            dd_item("Differential Pair", ActiveBarAction::PlaceDiffPair),
            dd_item("Blanket", ActiveBarAction::PlaceBlanket),
            dd_item("Compile Mask", ActiveBarAction::PlaceCompileMask),
        ],
        ActiveBarMenu::TextTools => vec![
            dd_item("Text String", ActiveBarAction::PlaceTextString),
            dd_item("Text Frame", ActiveBarAction::PlaceTextFrame),
            dd_item("Note", ActiveBarAction::PlaceNote),
        ],
        ActiveBarMenu::Shapes => vec![
            dd_item("Arc", ActiveBarAction::DrawArc),
            dd_item("Full Circle", ActiveBarAction::DrawFullCircle),
            dd_item("Elliptical Arc", ActiveBarAction::DrawEllipticalArc),
            dd_item("Ellipse", ActiveBarAction::DrawEllipse),
            dd_sep(),
            dd_item("Line", ActiveBarAction::DrawLine),
            dd_item("Rectangle", ActiveBarAction::DrawRectangle),
            dd_item("Round Rectangle", ActiveBarAction::DrawRoundRectangle),
            dd_item("Polygon", ActiveBarAction::DrawPolygon),
            dd_item("Bezier", ActiveBarAction::DrawBezier),
            dd_sep(),
            dd_item("Graphic...", ActiveBarAction::PlaceGraphic),
        ],
        ActiveBarMenu::NetColor => {
            let color_item = |label: &str, color: Color, action: ActiveBarAction| -> Element<'static, ActiveBarMsg> {
                button(
                    row![
                        container(Space::new())
                            .width(14)
                            .height(14)
                            .style(move |_: &Theme| container::Style {
                                background: Some(Background::Color(color)),
                                border: Border {
                                    width: 1.0,
                                    radius: 2.0.into(),
                                    color: Color::from_rgb(0.3, 0.3, 0.35),
                                },
                                ..container::Style::default()
                            }),
                        text(label.to_string()).size(11).color(styles::TEXT_PRIMARY),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                )
                .padding([4, 12])
                .width(Length::Fill)
                .on_press(ActiveBarMsg::Action(action))
                .style(dd_btn_style)
                .into()
            };
            vec![
                color_item("Blue", Color::from_rgb(0.40, 0.40, 0.93), ActiveBarAction::NetColorBlue),
                color_item("Light Green", Color::from_rgb(0.40, 0.93, 0.40), ActiveBarAction::NetColorLightGreen),
                color_item("Light Blue", Color::from_rgb(0.40, 0.80, 0.93), ActiveBarAction::NetColorLightBlue),
                color_item("Red", Color::from_rgb(0.93, 0.30, 0.30), ActiveBarAction::NetColorRed),
                color_item("Fuchsia", Color::from_rgb(0.80, 0.30, 0.80), ActiveBarAction::NetColorFuchsia),
                color_item("Yellow", Color::from_rgb(0.93, 0.80, 0.20), ActiveBarAction::NetColorYellow),
                color_item("Dark Green", Color::from_rgb(0.13, 0.55, 0.13), ActiveBarAction::NetColorDarkGreen),
                dd_sep(),
                dd_item("Clear Net Color", ActiveBarAction::ClearNetColor),
                dd_item("Clear All Net Colors", ActiveBarAction::ClearAllNetColors),
            ]
        }
    };

    container(column(items).spacing(0).width(220))
        .padding([4, 0])
        .style(|_: &Theme| container::Style {
            background: Some(Color::from_rgb(0.14, 0.14, 0.16).into()),
            text_color: Some(styles::TEXT_PRIMARY),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: Color::from_rgb(0.24, 0.25, 0.33),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                offset: iced::Vector::new(2.0, 3.0),
                blur_radius: 8.0,
            },
            ..container::Style::default()
        })
        .into()
}

/// Horizontal offset (in px) to align dropdown below a given button index.
pub fn dropdown_x_offset(menu: ActiveBarMenu) -> f32 {
    // Each icon = 22px mouse_area + 1px spacing = 23px per button
    // Separator = 1px + 1px spacing = 2px
    // Bar padding = [3, 4] → 4px left padding
    // Layout: [Filter][+] | [Select][Move][Align] | [Wire][Power] | [Harness][Port][Dir] | [Text][Shapes][NetColor]
    //  btn:     0      1  s    2      3      4    s   5      6    s    7      8      9    s  10     11     12
    let btn = 23.0_f32; // button width + spacing
    let s = 2.0_f32;    // separator width + spacing
    let pad = 4.0_f32;  // left padding of bar container
    let px = pad + match menu {
        ActiveBarMenu::Filter => 0.0,                          // btn 0
        ActiveBarMenu::Select => 2.0 * btn + s,                // btn 2
        ActiveBarMenu::Align => 4.0 * btn + s,                 // btn 4
        ActiveBarMenu::Wiring => 5.0 * btn + 2.0 * s,          // btn 5
        ActiveBarMenu::Power => 6.0 * btn + 2.0 * s,           // btn 6
        ActiveBarMenu::Harness => 7.0 * btn + 3.0 * s,         // btn 7
        ActiveBarMenu::Port => 8.0 * btn + 3.0 * s,            // btn 8
        ActiveBarMenu::Directives => 9.0 * btn + 3.0 * s,      // btn 9
        ActiveBarMenu::TextTools => 10.0 * btn + 4.0 * s,      // btn 10
        ActiveBarMenu::Shapes => 11.0 * btn + 4.0 * s,         // btn 11
        ActiveBarMenu::NetColor => 12.0 * btn + 4.0 * s,       // btn 12
    };
    px
}

// ─── Helpers ─────────────────────────────────────────────────

/// Active Bar button: click opens dropdown or activates tool.
/// Uses SVG icon with an opaque background to ensure clickability.
fn ab_icon_btn(
    icon_bytes: &'static [u8],
    active: bool,
    msg: ActiveBarMsg,
) -> Element<'static, ActiveBarMsg> {
    let handle = svg::Handle::from_memory(icon_bytes);

    // Use mouse_area wrapping the icon for guaranteed click detection
    let icon_widget = container(
        svg(handle).width(16).height(16),
    )
    .width(22)
    .height(22)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .style(move |_: &Theme| {
        let bg = if active {
            Some(Background::Color(Color::from_rgb(0.22, 0.23, 0.30)))
        } else {
            Some(Background::Color(Color::TRANSPARENT))
        };
        container::Style {
            background: bg,
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                color: Color::TRANSPARENT,
            },
            ..container::Style::default()
        }
    });

    iced::widget::mouse_area(icon_widget)
        .on_press(msg)
        .interaction(iced::mouse::Interaction::Pointer)
        .into()
}

fn sep() -> Element<'static, ActiveBarMsg> {
    container(Space::new())
        .width(1)
        .height(18)
        .style(|_: &Theme| container::Style {
            background: Some(Color::from_rgb(0.24, 0.25, 0.33).into()),
            ..container::Style::default()
        })
        .into()
}

fn dd_item(label: &str, action: ActiveBarAction) -> Element<'static, ActiveBarMsg> {
    button(
        text(label.to_string())
            .size(11)
            .color(styles::TEXT_PRIMARY),
    )
    .padding([4, 12])
    .width(Length::Fill)
    .on_press(ActiveBarMsg::Action(action))
    .style(dd_btn_style)
    .into()
}

fn dd_sep() -> Element<'static, ActiveBarMsg> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(|_: &Theme| container::Style {
            background: Some(Color::from_rgb(0.24, 0.25, 0.33).into()),
            ..container::Style::default()
        })
        .into()
}

fn dd_btn_style(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Some(Background::Color(Color::from_rgb(0.22, 0.22, 0.26))),
        _ => None,
    };
    button::Style {
        background: bg,
        border: Border::default(),
        text_color: styles::TEXT_PRIMARY,
        ..button::Style::default()
    }
}
