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

// ─── Dropdown item SVG icons (editable files in assets/icons/dropdown/) ──

const DD_WIRE: &[u8] = include_bytes!("../assets/icons/dropdown/wire.svg");
const DD_BUS: &[u8] = include_bytes!("../assets/icons/dropdown/bus.svg");
const DD_BUS_ENTRY: &[u8] = include_bytes!("../assets/icons/dropdown/bus_entry.svg");
const DD_NET_LABEL: &[u8] = include_bytes!("../assets/icons/dropdown/net_label.svg");
const DD_GND: &[u8] = include_bytes!("../assets/icons/dropdown/gnd.svg");
const DD_VCC: &[u8] = include_bytes!("../assets/icons/dropdown/vcc.svg");
const DD_PWR_ARROW: &[u8] = include_bytes!("../assets/icons/dropdown/pwr_arrow.svg");
const DD_PWR_BAR: &[u8] = include_bytes!("../assets/icons/dropdown/pwr_bar.svg");
const DD_PWR_CIRCLE: &[u8] = include_bytes!("../assets/icons/dropdown/pwr_circle.svg");
const DD_PWR_EARTH: &[u8] = include_bytes!("../assets/icons/dropdown/pwr_earth.svg");
const DD_PORT: &[u8] = include_bytes!("../assets/icons/dropdown/port.svg");
const DD_OFF_SHEET: &[u8] = include_bytes!("../assets/icons/dropdown/off_sheet.svg");
const DD_PARAM_SET: &[u8] = include_bytes!("../assets/icons/dropdown/param_set.svg");
const DD_NO_ERC: &[u8] = include_bytes!("../assets/icons/dropdown/no_erc.svg");
const DD_DIFF_PAIR: &[u8] = include_bytes!("../assets/icons/dropdown/diff_pair.svg");
const DD_BLANKET: &[u8] = include_bytes!("../assets/icons/dropdown/blanket.svg");
const DD_TEXT_STRING: &[u8] = include_bytes!("../assets/icons/dropdown/text_string.svg");
const DD_TEXT_FRAME: &[u8] = include_bytes!("../assets/icons/dropdown/text_frame.svg");
const DD_NOTE: &[u8] = include_bytes!("../assets/icons/dropdown/note.svg");
const DD_ARC: &[u8] = include_bytes!("../assets/icons/dropdown/arc.svg");
const DD_CIRCLE: &[u8] = include_bytes!("../assets/icons/dropdown/circle.svg");
const DD_ELLIPSE: &[u8] = include_bytes!("../assets/icons/dropdown/ellipse.svg");
const DD_LINE: &[u8] = include_bytes!("../assets/icons/dropdown/line.svg");
const DD_RECT: &[u8] = include_bytes!("../assets/icons/dropdown/rect.svg");
const DD_ROUND_RECT: &[u8] = include_bytes!("../assets/icons/dropdown/round_rect.svg");
const DD_POLYGON: &[u8] = include_bytes!("../assets/icons/dropdown/polygon.svg");
const DD_BEZIER: &[u8] = include_bytes!("../assets/icons/dropdown/bezier.svg");
const DD_HARNESS: &[u8] = include_bytes!("../assets/icons/dropdown/harness.svg");
const DD_HARNESS_CONN: &[u8] = include_bytes!("../assets/icons/dropdown/harness_conn.svg");
// Align icons
const DD_ALIGN_LEFT: &[u8] = include_bytes!("../assets/icons/dropdown/align_left.svg");
const DD_ALIGN_RIGHT: &[u8] = include_bytes!("../assets/icons/dropdown/align_right.svg");
const DD_ALIGN_HCENTER: &[u8] = include_bytes!("../assets/icons/dropdown/align_hcenter.svg");
const DD_DIST_HORIZ: &[u8] = include_bytes!("../assets/icons/dropdown/dist_horiz.svg");
const DD_ALIGN_TOP: &[u8] = include_bytes!("../assets/icons/dropdown/align_top.svg");
const DD_ALIGN_BOTTOM: &[u8] = include_bytes!("../assets/icons/dropdown/align_bottom.svg");
const DD_ALIGN_VCENTER: &[u8] = include_bytes!("../assets/icons/dropdown/align_vcenter.svg");
const DD_DIST_VERT: &[u8] = include_bytes!("../assets/icons/dropdown/dist_vert.svg");
const DD_ALIGN_GRID: &[u8] = include_bytes!("../assets/icons/dropdown/align_grid.svg");
// Move/transform icons
const DD_DRAG: &[u8] = include_bytes!("../assets/icons/dropdown/drag.svg");
const DD_MOVE_SEL: &[u8] = include_bytes!("../assets/icons/dropdown/move_sel.svg");
const DD_MOVE_XY: &[u8] = include_bytes!("../assets/icons/dropdown/move_xy.svg");
const DD_ROTATE: &[u8] = include_bytes!("../assets/icons/dropdown/rotate.svg");
const DD_ROTATE_CW: &[u8] = include_bytes!("../assets/icons/dropdown/rotate_cw.svg");
const DD_BRING_FRONT: &[u8] = include_bytes!("../assets/icons/dropdown/bring_front.svg");
const DD_SEND_BACK: &[u8] = include_bytes!("../assets/icons/dropdown/send_back.svg");
const DD_FLIP_X: &[u8] = include_bytes!("../assets/icons/dropdown/flip_x.svg");
const DD_FLIP_Y: &[u8] = include_bytes!("../assets/icons/dropdown/flip_y.svg");
// Power extras
const DD_PWR_PLUS12: &[u8] = include_bytes!("../assets/icons/dropdown/pwr_plus12.svg");
const DD_PWR_PLUS5: &[u8] = include_bytes!("../assets/icons/dropdown/pwr_plus5.svg");
const DD_PWR_MINUS5: &[u8] = include_bytes!("../assets/icons/dropdown/pwr_minus5.svg");
const DD_PWR_WAVE: &[u8] = include_bytes!("../assets/icons/dropdown/pwr_wave.svg");
const DD_PWR_SIG_GND: &[u8] = include_bytes!("../assets/icons/dropdown/pwr_signal_gnd.svg");
// Sheet symbol icons
const DD_SHEET_SYM: &[u8] = include_bytes!("../assets/icons/dropdown/sheet_symbol.svg");
const DD_SHEET_ENTRY: &[u8] = include_bytes!("../assets/icons/dropdown/sheet_entry.svg");
const DD_DEVICE_SHEET: &[u8] = include_bytes!("../assets/icons/dropdown/device_sheet.svg");
const DD_REUSE_BLOCK: &[u8] = include_bytes!("../assets/icons/dropdown/reuse_block.svg");
// Misc
const DD_GRAPHIC: &[u8] = include_bytes!("../assets/icons/dropdown/graphic.svg");

// ─── Messages ────────────────────────────────────────────────

/// Which Active Bar dropdown menu is open (by button index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveBarMenu {
    Filter,
    SelectMode,  // Lasso, Inside Area, etc.
    Select,      // Move/transform
    Align,
    Wiring,
    Power,
    Harness,
    SheetSymbol, // Sheet Symbol, Sheet Entry, Device Sheet Symbol
    Port,
    Directives,
    TextTools,
    Shapes,
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
    // Selection modes
    ToolSelect,
    LassoSelect,
    InsideArea,
    OutsideArea,
    TouchingRectangle,
    TouchingLine,
    SelectAll,
    SelectConnection,
    ToggleSelection,
    // Move/Transform
    Drag,
    MoveSelection,
    MoveSelectionXY,
    DragSelection,
    MoveToFront,
    RotateSelection,
    RotateSelectionCW,
    FlipSelectedX,
    FlipSelectedY,
    BringToFront,
    SendToBack,
    BringToFrontOf,
    SendToBackOf,
    // Sheet symbols
    PlaceSheetSymbol,
    PlaceSheetEntry,
    PlaceDeviceSheetSymbol,
    PlaceReuseBlock,
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
    last_tool: &std::collections::HashMap<String, ActiveBarAction>,
) -> Element<'static, ActiveBarMsg> {
    let bar = view_bar(open_menu, current_tool, draw_mode, last_tool);

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
    last_tool: &std::collections::HashMap<String, ActiveBarAction>,
) -> Element<'static, ActiveBarMsg> {
    // Helper: get last-used action for a group, or use default
    let last = |group: &str, default: ActiveBarAction| -> ActiveBarMsg {
        ActiveBarMsg::Action(last_tool.get(group).cloned().unwrap_or(default))
    };
    let mut items: Vec<Element<'_, ActiveBarMsg>> = Vec::new();

    // 1. Filter — left: toggle, right: filter dropdown
    items.push(ab_icon_btn(ICON_FILTER, false,
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Filter),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Filter)),
        "Selection Filter"));
    items.push(ab_icon_btn(ICON_ADDPART,
        current_tool == crate::app::Tool::Component,
        ActiveBarMsg::Action(ActiveBarAction::PlaceComponent),
        None, "Place Component"));
    items.push(sep());

    items.push(ab_icon_btn(ICON_SELECT,
        current_tool == crate::app::Tool::Select,
        ActiveBarMsg::Action(ActiveBarAction::ToolSelect),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::SelectMode)),
        "Select"));
    items.push(ab_icon_btn(ICON_MOVE, false,
        ActiveBarMsg::Action(ActiveBarAction::MoveSelection),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Select)),
        "Move / Transform"));
    items.push(ab_icon_btn(ICON_ALIGN, false,
        ActiveBarMsg::Action(ActiveBarAction::AlignToGrid),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align)),
        "Align"));
    items.push(sep());

    items.push(ab_icon_btn(ICON_WIRE,
        current_tool == crate::app::Tool::Wire || current_tool == crate::app::Tool::Bus,
        last("wiring", ActiveBarAction::DrawWire),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Wiring)),
        "Wiring"));
    items.push(ab_icon_btn(ICON_POWER, false,
        last("power", ActiveBarAction::PlacePowerGND),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Power)),
        "Power Port"));
    items.push(sep());

    items.push(ab_icon_btn(ICON_HARNESS, false,
        last("harness", ActiveBarAction::PlaceSignalHarness),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Harness)),
        "Harness"));
    items.push(ab_icon_btn(ICON_SHEETSYM, false,
        last("sheet", ActiveBarAction::PlaceSheetSymbol),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::SheetSymbol)),
        "Sheet Symbol"));
    items.push(ab_icon_btn(ICON_PORT, false,
        last("port", ActiveBarAction::PlacePort),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Port)),
        "Port / Connector"));
    items.push(ab_icon_btn(ICON_DIRECTIVES, false,
        ActiveBarMsg::Action(ActiveBarAction::PlaceParameterSet),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Directives)),
        "Directives"));
    items.push(sep());

    items.push(ab_icon_btn(ICON_TEXT,
        current_tool == crate::app::Tool::Text,
        last("text", ActiveBarAction::PlaceTextString),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::TextTools)),
        "Text"));
    items.push(ab_icon_btn(ICON_SHAPES,
        matches!(current_tool, crate::app::Tool::Line | crate::app::Tool::Rectangle | crate::app::Tool::Circle),
        last("shapes", ActiveBarAction::DrawLine),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Shapes)),
        "Drawing Tools"));
    items.push(ab_icon_btn(ICON_NETCOLOR, false,
        ActiveBarMsg::Action(ActiveBarAction::ToolSelect),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::NetColor)),
        "Net Color"));

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
        ActiveBarMenu::SelectMode => vec![
            dd_item("Lasso Select", ActiveBarAction::LassoSelect),
            dd_item("Inside Area", ActiveBarAction::InsideArea),
            dd_item("Outside Area", ActiveBarAction::OutsideArea),
            dd_item("Touching Rectangle", ActiveBarAction::TouchingRectangle),
            dd_item("Touching Line", ActiveBarAction::TouchingLine),
            dd_item("All", ActiveBarAction::SelectAll),
            dd_item("Connection", ActiveBarAction::SelectConnection),
            dd_sep(),
            dd_item("Toggle Selection", ActiveBarAction::ToggleSelection),
        ],
        ActiveBarMenu::Select => vec![
            dd_item_svg(DD_DRAG, "Drag", ActiveBarAction::Drag),
            dd_item_svg(DD_DRAG, "Move", ActiveBarAction::MoveSelection),
            dd_item_svg(DD_MOVE_SEL, "Move Selection", ActiveBarAction::MoveSelection),
            dd_item_svg(DD_MOVE_XY, "Move Selection by X, Y...", ActiveBarAction::MoveSelectionXY),
            dd_item_svg(DD_DRAG, "Drag Selection", ActiveBarAction::DragSelection),
            dd_item("Move To Front", ActiveBarAction::MoveToFront),
            dd_item_svg(DD_ROTATE, "Rotate Selection", ActiveBarAction::RotateSelection),
            dd_item_svg(DD_ROTATE_CW, "Rotate Selection Clockwise", ActiveBarAction::RotateSelectionCW),
            dd_sep(),
            dd_item_svg(DD_BRING_FRONT, "Bring To Front", ActiveBarAction::BringToFront),
            dd_item_svg(DD_SEND_BACK, "Send To Back", ActiveBarAction::SendToBack),
            dd_item_svg(DD_BRING_FRONT, "Bring To Front Of", ActiveBarAction::BringToFrontOf),
            dd_item_svg(DD_SEND_BACK, "Send To Back Of", ActiveBarAction::SendToBackOf),
            dd_sep(),
            dd_item_svg(DD_FLIP_X, "Flip Selected Sheet Symbols Along X", ActiveBarAction::FlipSelectedX),
            dd_item_svg(DD_FLIP_Y, "Flip Selected Sheet Symbols Along Y", ActiveBarAction::FlipSelectedY),
        ],
        ActiveBarMenu::Align => vec![
            dd_item_svg(DD_ALIGN_LEFT, "Align Left", ActiveBarAction::AlignLeft),
            dd_item_svg(DD_ALIGN_RIGHT, "Align Right", ActiveBarAction::AlignRight),
            dd_item_svg(DD_ALIGN_HCENTER, "Align Horizontal Centers", ActiveBarAction::AlignHorizontalCenters),
            dd_item_svg(DD_DIST_HORIZ, "Distribute Horizontally", ActiveBarAction::DistributeHorizontally),
            dd_sep(),
            dd_item_svg(DD_ALIGN_TOP, "Align Top", ActiveBarAction::AlignTop),
            dd_item_svg(DD_ALIGN_BOTTOM, "Align Bottom", ActiveBarAction::AlignBottom),
            dd_item_svg(DD_ALIGN_VCENTER, "Align Vertical Centers", ActiveBarAction::AlignVerticalCenters),
            dd_item_svg(DD_DIST_VERT, "Distribute Vertically", ActiveBarAction::DistributeVertically),
            dd_sep(),
            dd_item_svg(DD_ALIGN_GRID, "Align To Grid", ActiveBarAction::AlignToGrid),
        ],
        ActiveBarMenu::Wiring => vec![
            dd_item_svg(DD_WIRE, "Wire", ActiveBarAction::DrawWire),
            dd_item_svg(DD_BUS, "Bus", ActiveBarAction::DrawBus),
            dd_item_svg(DD_BUS_ENTRY, "Bus Entry", ActiveBarAction::PlaceBusEntry),
            dd_item_svg(DD_NET_LABEL, "Net Label", ActiveBarAction::PlaceNetLabel),
        ],
        ActiveBarMenu::Power => vec![
            dd_item_svg(DD_GND, "Place GND power port", ActiveBarAction::PlacePowerGND),
            dd_item_svg(DD_VCC, "Place VCC power port", ActiveBarAction::PlacePowerVCC),
            dd_item_svg(DD_PWR_PLUS12, "Place +12 power port", ActiveBarAction::PlacePowerPlus12),
            dd_item_svg(DD_PWR_PLUS5, "Place +5 power port", ActiveBarAction::PlacePowerPlus5),
            dd_item_svg(DD_PWR_MINUS5, "Place -5 power port", ActiveBarAction::PlacePowerMinus5),
            dd_sep(),
            dd_item_svg(DD_PWR_ARROW, "Place Arrow style power port", ActiveBarAction::PlacePowerArrow),
            dd_item_svg(DD_PWR_WAVE, "Place Wave style power port", ActiveBarAction::PlacePowerWave),
            dd_item_svg(DD_PWR_BAR, "Place Bar style power port", ActiveBarAction::PlacePowerBar),
            dd_item_svg(DD_PWR_CIRCLE, "Place Circle style power port", ActiveBarAction::PlacePowerCircle),
            dd_sep(),
            dd_item_svg(DD_PWR_SIG_GND, "Place Signal Ground power port", ActiveBarAction::PlacePowerSignalGND),
            dd_item_svg(DD_PWR_EARTH, "Place Earth power port", ActiveBarAction::PlacePowerEarth),
        ],
        ActiveBarMenu::Harness => vec![
            dd_item_svg(DD_HARNESS, "Signal Harness", ActiveBarAction::PlaceSignalHarness),
            dd_item_svg(DD_HARNESS_CONN, "Harness Connector", ActiveBarAction::PlaceHarnessConnector),
            dd_item_svg(DD_HARNESS, "Harness Entry", ActiveBarAction::PlaceHarnessEntry),
        ],
        ActiveBarMenu::SheetSymbol => vec![
            dd_item_svg(DD_SHEET_SYM, "Sheet Symbol", ActiveBarAction::PlaceSheetSymbol),
            dd_item_svg(DD_SHEET_ENTRY, "Sheet Entry", ActiveBarAction::PlaceSheetEntry),
            dd_item_svg(DD_DEVICE_SHEET, "Device Sheet Symbol", ActiveBarAction::PlaceDeviceSheetSymbol),
            dd_item_svg(DD_REUSE_BLOCK, "Reuse Block...", ActiveBarAction::PlaceReuseBlock),
        ],
        ActiveBarMenu::Port => vec![
            dd_item_svg(DD_PORT, "Port", ActiveBarAction::PlacePort),
            dd_item_svg(DD_OFF_SHEET, "Off Sheet Connector", ActiveBarAction::PlaceOffSheetConnector),
        ],
        ActiveBarMenu::Directives => vec![
            dd_item_svg(DD_PARAM_SET, "Parameter Set", ActiveBarAction::PlaceParameterSet),
            dd_item_svg(DD_NO_ERC, "Generic No ERC", ActiveBarAction::PlaceNoERC),
            dd_item_svg(DD_DIFF_PAIR, "Differential Pair", ActiveBarAction::PlaceDiffPair),
            dd_item_svg(DD_BLANKET, "Blanket", ActiveBarAction::PlaceBlanket),
            dd_item_svg(DD_BLANKET, "Compile Mask", ActiveBarAction::PlaceCompileMask),
        ],
        ActiveBarMenu::TextTools => vec![
            dd_item_svg(DD_TEXT_STRING, "Text String", ActiveBarAction::PlaceTextString),
            dd_item_svg(DD_TEXT_FRAME, "Text Frame", ActiveBarAction::PlaceTextFrame),
            dd_item_svg(DD_NOTE, "Note", ActiveBarAction::PlaceNote),
        ],
        ActiveBarMenu::Shapes => vec![
            dd_item_svg(DD_ARC, "Arc", ActiveBarAction::DrawArc),
            dd_item_svg(DD_CIRCLE, "Full Circle", ActiveBarAction::DrawFullCircle),
            dd_item_svg(DD_ARC, "Elliptical Arc", ActiveBarAction::DrawEllipticalArc),
            dd_item_svg(DD_ELLIPSE, "Ellipse", ActiveBarAction::DrawEllipse),
            dd_sep(),
            dd_item_svg(DD_LINE, "Line", ActiveBarAction::DrawLine),
            dd_item_svg(DD_RECT, "Rectangle", ActiveBarAction::DrawRectangle),
            dd_item_svg(DD_ROUND_RECT, "Round Rectangle", ActiveBarAction::DrawRoundRectangle),
            dd_item_svg(DD_POLYGON, "Polygon", ActiveBarAction::DrawPolygon),
            dd_item_svg(DD_BEZIER, "Bezier", ActiveBarAction::DrawBezier),
            dd_sep(),
            dd_item_svg(DD_GRAPHIC, "Graphic...", ActiveBarAction::PlaceGraphic),
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

    container(column(items).spacing(0))
        .padding([6, 0])
        .style(|_: &Theme| container::Style {
            background: Some(Color::from_rgb(0.11, 0.12, 0.15).into()),
            text_color: Some(styles::TEXT_PRIMARY),
            border: Border {
                width: 1.0,
                radius: 6.0.into(),
                color: Color::from_rgb(0.20, 0.21, 0.27),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
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
    // Layout: [Filter][+] | [Select][Move][Align] | [Wire][Power] | [Harness][Sheet][Port][Dir] | [Text][Shapes][NetColor]
    //  btn:     0      1  s    2      3      4    s   5      6    s    7      8     9    10    s  11     12     13
    let btn = 23.0_f32;
    let s = 2.0_f32;
    let pad = 4.0_f32;
    let px = pad + match menu {
        ActiveBarMenu::Filter => 0.0,
        ActiveBarMenu::SelectMode => 2.0 * btn + s,
        ActiveBarMenu::Select => 3.0 * btn + s,
        ActiveBarMenu::Align => 4.0 * btn + s,
        ActiveBarMenu::Wiring => 5.0 * btn + 2.0 * s,
        ActiveBarMenu::Power => 6.0 * btn + 2.0 * s,
        ActiveBarMenu::Harness => 7.0 * btn + 3.0 * s,
        ActiveBarMenu::SheetSymbol => 8.0 * btn + 3.0 * s,
        ActiveBarMenu::Port => 9.0 * btn + 3.0 * s,
        ActiveBarMenu::Directives => 10.0 * btn + 3.0 * s,
        ActiveBarMenu::TextTools => 11.0 * btn + 4.0 * s,
        ActiveBarMenu::Shapes => 12.0 * btn + 4.0 * s,
        ActiveBarMenu::NetColor => 13.0 * btn + 4.0 * s,
    };
    px
}

// ─── Helpers ─────────────────────────────────────────────────

// Small 45° chevron SVG for dropdown indicator (bottom-right corner)
const CHEVRON_45: &[u8] = include_bytes!("../assets/icons/chevron_45.svg");

/// Active Bar button: left-click activates tool, right-click opens dropdown.
/// Shows a small 45° chevron at bottom-right if button has a dropdown.
fn ab_icon_btn(
    icon_bytes: &'static [u8],
    active: bool,
    left_click: ActiveBarMsg,
    right_click: Option<ActiveBarMsg>,
    tooltip_text: &'static str,
) -> Element<'static, ActiveBarMsg> {
    let handle = svg::Handle::from_memory(icon_bytes);
    let has_dropdown = right_click.is_some();

    // Icon with optional chevron indicator
    let icon_content: Element<'static, ActiveBarMsg> = if has_dropdown {
        let chevron = svg(svg::Handle::from_memory(CHEVRON_45))
            .width(6)
            .height(6);
        iced::widget::Stack::new()
            .push(
                container(svg(handle).width(16).height(16))
                    .width(22)
                    .height(22)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .push(
                container(chevron)
                    .width(22)
                    .height(22)
                    .align_x(iced::alignment::Horizontal::Right)
                    .align_y(iced::alignment::Vertical::Bottom),
            )
            .into()
    } else {
        container(svg(handle).width(16).height(16))
            .width(22)
            .height(22)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    };

    let icon_widget = container(icon_content)
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

    let mut area = iced::widget::mouse_area(icon_widget)
        .on_press(left_click);

    if let Some(rc) = right_click {
        area = area.on_right_press(rc);
    }

    let tip = container(
        text(tooltip_text).size(11).color(styles::TEXT_PRIMARY),
    )
    .padding([4, 8])
    .style(|_: &Theme| container::Style {
        background: Some(Color::from_rgb(0.14, 0.14, 0.18).into()),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: Color::from_rgb(0.24, 0.25, 0.30),
        },
        ..container::Style::default()
    });

    iced::widget::tooltip(area, tip, iced::widget::tooltip::Position::Bottom)
        .gap(4)
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

/// Dropdown item with optional inline SVG icon (Altium-style).
fn dd_item(label: &str, action: ActiveBarAction) -> Element<'static, ActiveBarMsg> {
    dd_item_icon(None, label, styles::TEXT_PRIMARY, action)
}

/// Dropdown item with colored icon SVG bytes.
fn dd_item_svg(
    icon: &'static [u8],
    label: &str,
    action: ActiveBarAction,
) -> Element<'static, ActiveBarMsg> {
    dd_item_icon(Some(icon), label, styles::TEXT_PRIMARY, action)
}

fn dd_item_icon(
    icon: Option<&'static [u8]>,
    label: &str,
    text_c: Color,
    action: ActiveBarAction,
) -> Element<'static, ActiveBarMsg> {
    let mut r = iced::widget::Row::new()
        .spacing(8)
        .align_y(iced::Alignment::Center);

    if let Some(icon_bytes) = icon {
        let handle = svg::Handle::from_memory(icon_bytes);
        r = r.push(
            svg(handle).width(14).height(14),
        );
    } else {
        r = r.push(Space::new().width(14).height(14));
    }

    r = r.push(
        text(label.to_string())
            .size(12)
            .color(text_c)
            .wrapping(iced::widget::text::Wrapping::None),
    );

    button(r)
        .padding([5, 12])
        .on_press(ActiveBarMsg::Action(action))
        .style(dd_btn_style)
        .into()
}

fn dd_sep() -> Element<'static, ActiveBarMsg> {
    container(
        container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(|_: &Theme| container::Style {
                background: Some(Color::from_rgb(0.24, 0.25, 0.33).into()),
                ..container::Style::default()
            }),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into()
}

fn dd_btn_style(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Some(Background::Color(Color::from_rgb(0.20, 0.22, 0.30))),
        _ => None,
    };
    button::Style {
        background: bg,
        border: Border::default(),
        text_color: styles::TEXT_PRIMARY,
        ..button::Style::default()
    }
}
