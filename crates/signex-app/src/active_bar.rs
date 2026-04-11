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

// ─── View: Active Bar ────────────────────────────────────────

/// Render the Active Bar (the floating toolbar strip).
pub fn view_bar(
    open_menu: Option<ActiveBarMenu>,
    current_tool: crate::app::Tool,
    draw_mode: crate::app::DrawMode,
) -> Element<'static, ActiveBarMsg> {
    let mut items: Vec<Element<'_, ActiveBarMsg>> = Vec::new();

    // 1. Filter
    items.push(ab_icon_btn(
        ICON_FILTER,
        open_menu == Some(ActiveBarMenu::Filter),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Filter),
    ));
    items.push(sep());

    // 2. Select (cursor)
    items.push(ab_icon_btn(
        ICON_SELECT,
        current_tool == crate::app::Tool::Select && open_menu != Some(ActiveBarMenu::Select),
        ActiveBarMsg::Action(ActiveBarAction::ToolSelect),
    ));
    // 3. Move/Transform
    items.push(ab_icon_btn(
        ICON_MOVE,
        open_menu == Some(ActiveBarMenu::Select),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Select),
    ));
    // 4. Align
    items.push(ab_icon_btn(
        ICON_ALIGN,
        open_menu == Some(ActiveBarMenu::Align),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align),
    ));
    items.push(sep());

    // 5. Wiring
    items.push(ab_icon_btn(
        ICON_WIRE,
        current_tool == crate::app::Tool::Wire
            || current_tool == crate::app::Tool::Bus
            || open_menu == Some(ActiveBarMenu::Wiring),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Wiring),
    ));
    // 6. Power
    items.push(ab_icon_btn(
        ICON_POWER,
        open_menu == Some(ActiveBarMenu::Power),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Power),
    ));
    items.push(sep());

    // 7. Harness
    items.push(ab_icon_btn(
        ICON_HARNESS,
        open_menu == Some(ActiveBarMenu::Harness),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Harness),
    ));
    // 8. Port
    items.push(ab_icon_btn(
        ICON_PORT,
        open_menu == Some(ActiveBarMenu::Port),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Port),
    ));
    // 9. Directives
    items.push(ab_icon_btn(
        ICON_DIRECTIVES,
        open_menu == Some(ActiveBarMenu::Directives),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Directives),
    ));
    items.push(sep());

    // 10. Text
    items.push(ab_icon_btn(
        ICON_TEXT,
        current_tool == crate::app::Tool::Text || open_menu == Some(ActiveBarMenu::TextTools),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::TextTools),
    ));
    // 11. Shapes
    items.push(ab_icon_btn(
        ICON_SHAPES,
        matches!(
            current_tool,
            crate::app::Tool::Line | crate::app::Tool::Rectangle | crate::app::Tool::Circle
        ) || open_menu == Some(ActiveBarMenu::Shapes),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Shapes),
    ));
    // 12. Net Color
    items.push(ab_icon_btn(
        ICON_NETCOLOR,
        open_menu == Some(ActiveBarMenu::NetColor),
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::NetColor),
    ));

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

    container(column(items).spacing(0).width(Length::Shrink))
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
/// Each icon button is ~26px wide, plus 1px spacing, plus separators.
pub fn dropdown_x_offset(menu: ActiveBarMenu) -> f32 {
    // Button widths: icon=26px, sep=1px+margins, spacing=1px
    // Group: [Filter] | [Select][Move][Align] | [Wire][Power] | [Harness][Port][Dir] | [Text][Shapes][NetColor]
    let idx = match menu {
        ActiveBarMenu::Filter => 0,
        ActiveBarMenu::Select => 2,   // after filter + sep
        ActiveBarMenu::Align => 4,    // select + move + align
        ActiveBarMenu::Wiring => 6,   // after align + sep
        ActiveBarMenu::Power => 7,
        ActiveBarMenu::Harness => 9,  // after power + sep
        ActiveBarMenu::Port => 10,
        ActiveBarMenu::Directives => 11,
        ActiveBarMenu::TextTools => 13, // after dir + sep
        ActiveBarMenu::Shapes => 14,
        ActiveBarMenu::NetColor => 15,
    };
    idx as f32 * 27.0
}

// ─── Helpers ─────────────────────────────────────────────────

fn ab_icon_btn(
    icon_bytes: &'static [u8],
    active: bool,
    msg: ActiveBarMsg,
) -> Element<'static, ActiveBarMsg> {
    let handle = svg::Handle::from_memory(icon_bytes);
    button(
        container(svg(handle).width(16).height(16))
            .width(22)
            .height(22)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .padding(2)
    .on_press(msg)
    .style(move |_: &Theme, status: button::Status| {
        let bg = match (active, status) {
            (true, _) => Some(Background::Color(Color::from_rgb(0.22, 0.23, 0.30))),
            (false, button::Status::Hovered) => {
                Some(Background::Color(Color::from_rgb(0.20, 0.21, 0.27)))
            }
            _ => None,
        };
        button::Style {
            background: bg,
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                color: Color::TRANSPARENT,
            },
            ..button::Style::default()
        }
    })
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
