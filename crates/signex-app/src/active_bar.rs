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

// ─── Inline SVG icons for dropdown items ─────────────────────

const DD_WIRE: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#4ade80" stroke-width="2.5" stroke-linecap="round"><path d="M4 12h8v-8"/></svg>"##;
const DD_BUS: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#60a5fa" stroke-width="3" stroke-linecap="round"><path d="M4 12h16"/></svg>"##;
const DD_BUS_ENTRY: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#60a5fa" stroke-width="2" stroke-linecap="round"><path d="M6 18l12-12"/></svg>"##;
const DD_NET_LABEL: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#4ade80" stroke-width="2"><path d="M4 7h11l5 5-5 5H4V7z"/></svg>"##;
const DD_GND: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="2.5" stroke-linecap="round"><path d="M12 4v8"/><path d="M6 12h12"/><path d="M8 15h8"/><path d="M10 18h4"/></svg>"##;
const DD_VCC: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="2.5" stroke-linecap="round"><path d="M12 20v-8"/><path d="M6 12h12"/></svg>"##;
const DD_PWR_ARROW: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="2" stroke-linecap="round"><path d="M12 20v-14"/><path d="M7 10l5-5 5 5"/></svg>"##;
const DD_PWR_BAR: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="2.5" stroke-linecap="round"><path d="M12 20v-8"/><path d="M6 12h12"/></svg>"##;
const DD_PWR_CIRCLE: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="2"><circle cx="12" cy="8" r="4"/><path d="M12 12v8"/></svg>"##;
const DD_PWR_EARTH: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="2" stroke-linecap="round"><path d="M12 4v8"/><path d="M4 12h16"/><path d="M6 16h12"/><path d="M9 20h6"/></svg>"##;
const DD_PORT: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#eab308" stroke-width="2"><path d="M4 7h11l5 5-5 5H4V7z"/><line x1="4" y1="12" x2="1" y2="12"/></svg>"##;
const DD_OFF_SHEET: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#eab308" stroke-width="2"><path d="M3 7h8l5 5-5 5H3V7z"/><path d="M16 12h5"/><path d="M18 9l3 3-3 3"/></svg>"##;
const DD_PARAM_SET: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="2"><circle cx="12" cy="12" r="8"/><path d="M12 8v4"/><circle cx="12" cy="16" r="1" fill="#ef4444"/></svg>"##;
const DD_NO_ERC: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="2.5" stroke-linecap="round"><path d="M6 6l12 12"/><path d="M18 6L6 18"/></svg>"##;
const DD_DIFF_PAIR: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#60a5fa" stroke-width="2" stroke-linecap="round"><path d="M4 8h16"/><path d="M4 16h16"/></svg>"##;
const DD_BLANKET: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#4ade80" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2" stroke-dasharray="4 2"/></svg>"##;
const DD_TEXT_STRING: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2" stroke-linecap="round"><path d="M4 7V4h16v3"/><path d="M12 4v16"/><path d="M8 20h8"/></svg>"##;
const DD_TEXT_FRAME: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="1"/><path d="M7 8h10"/><path d="M7 12h7"/></svg>"##;
const DD_NOTE: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2"><path d="M3 3h18v14l-4 4H3V3z"/><path d="M14 17v4"/><path d="M14 17h4"/></svg>"##;
const DD_ARC: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2"><path d="M4 20A16 16 0 0120 4"/></svg>"##;
const DD_CIRCLE: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2"><circle cx="12" cy="12" r="9"/></svg>"##;
const DD_ELLIPSE: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2"><ellipse cx="12" cy="12" rx="10" ry="6"/></svg>"##;
const DD_LINE: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2" stroke-linecap="round"><path d="M4 20L20 4"/></svg>"##;
const DD_RECT: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2"><rect x="3" y="5" width="18" height="14"/></svg>"##;
const DD_ROUND_RECT: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2"><rect x="3" y="5" width="18" height="14" rx="4"/></svg>"##;
const DD_POLYGON: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2"><path d="M12 3l9 7-3 11H6L3 10z"/></svg>"##;
const DD_BEZIER: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2" stroke-linecap="round"><path d="M4 20C4 10 20 14 20 4"/></svg>"##;
const DD_HARNESS: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2" stroke-linecap="round"><path d="M4 8h6l4 4h6"/><path d="M4 16h6l4-4"/></svg>"##;
const DD_HARNESS_CONN: &[u8] = br##"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c8cad0" stroke-width="2"><rect x="6" y="4" width="12" height="16" rx="1"/><path d="M10 8h4"/><path d="M10 12h4"/><path d="M10 16h4"/></svg>"##;

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

    // 1. Filter — left: toggle, right: filter dropdown
    items.push(ab_icon_btn(ICON_FILTER, false,
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Filter),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Filter))));
    // 2. Add Component (+) — left: place component
    items.push(ab_icon_btn(ICON_ADDPART,
        current_tool == crate::app::Tool::Component,
        ActiveBarMsg::Action(ActiveBarAction::PlaceComponent),
        None));
    items.push(sep());

    // 3. Select — left: select tool, right: select modes
    items.push(ab_icon_btn(ICON_SELECT,
        current_tool == crate::app::Tool::Select,
        ActiveBarMsg::Action(ActiveBarAction::ToolSelect),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Select))));
    // 4. Move — left: move, right: move/transform dropdown
    items.push(ab_icon_btn(ICON_MOVE, false,
        ActiveBarMsg::Action(ActiveBarAction::MoveSelection),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Select))));
    // 5. Align — left: align to grid, right: align dropdown
    items.push(ab_icon_btn(ICON_ALIGN, false,
        ActiveBarMsg::Action(ActiveBarAction::AlignToGrid),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align))));
    items.push(sep());

    // 6. Wire — left: draw wire, right: wiring dropdown
    items.push(ab_icon_btn(ICON_WIRE,
        current_tool == crate::app::Tool::Wire || current_tool == crate::app::Tool::Bus,
        ActiveBarMsg::Action(ActiveBarAction::DrawWire),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Wiring))));
    // 7. Power — left: place GND, right: power dropdown
    items.push(ab_icon_btn(ICON_POWER, false,
        ActiveBarMsg::Action(ActiveBarAction::PlacePowerGND),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Power))));
    items.push(sep());

    // 8. Harness — left: signal harness, right: harness dropdown
    items.push(ab_icon_btn(ICON_HARNESS, false,
        ActiveBarMsg::Action(ActiveBarAction::PlaceSignalHarness),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Harness))));
    // 9. Port — left: place port, right: port dropdown
    items.push(ab_icon_btn(ICON_PORT, false,
        ActiveBarMsg::Action(ActiveBarAction::PlacePort),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Port))));
    // 10. Directives — left: parameter set, right: directives dropdown
    items.push(ab_icon_btn(ICON_DIRECTIVES, false,
        ActiveBarMsg::Action(ActiveBarAction::PlaceParameterSet),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Directives))));
    items.push(sep());

    // 11. Text — left: place text, right: text dropdown
    items.push(ab_icon_btn(ICON_TEXT,
        current_tool == crate::app::Tool::Text,
        ActiveBarMsg::Action(ActiveBarAction::PlaceTextString),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::TextTools))));
    // 12. Shapes — left: draw line, right: shapes dropdown
    items.push(ab_icon_btn(ICON_SHAPES,
        matches!(current_tool, crate::app::Tool::Line | crate::app::Tool::Rectangle | crate::app::Tool::Circle),
        ActiveBarMsg::Action(ActiveBarAction::DrawLine),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Shapes))));
    // 13. Net Color — left: no-op, right: color dropdown
    items.push(ab_icon_btn(ICON_NETCOLOR, false,
        ActiveBarMsg::Action(ActiveBarAction::ToolSelect),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::NetColor))));

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
            dd_item_svg(DD_WIRE, "Wire", ActiveBarAction::DrawWire),
            dd_item_svg(DD_BUS, "Bus", ActiveBarAction::DrawBus),
            dd_item_svg(DD_BUS_ENTRY, "Bus Entry", ActiveBarAction::PlaceBusEntry),
            dd_item_svg(DD_NET_LABEL, "Net Label", ActiveBarAction::PlaceNetLabel),
        ],
        ActiveBarMenu::Power => vec![
            dd_item_svg(DD_GND, "Place GND power port", ActiveBarAction::PlacePowerGND),
            dd_item_svg(DD_VCC, "Place VCC power port", ActiveBarAction::PlacePowerVCC),
            dd_item_svg(DD_PWR_ARROW, "Place +12 power port", ActiveBarAction::PlacePowerPlus12),
            dd_item_svg(DD_PWR_ARROW, "Place +5 power port", ActiveBarAction::PlacePowerPlus5),
            dd_item_svg(DD_PWR_ARROW, "Place -5 power port", ActiveBarAction::PlacePowerMinus5),
            dd_sep(),
            dd_item_svg(DD_PWR_ARROW, "Place Arrow style power port", ActiveBarAction::PlacePowerArrow),
            dd_item_svg(DD_PWR_BAR, "Place Wave style power port", ActiveBarAction::PlacePowerWave),
            dd_item_svg(DD_PWR_BAR, "Place Bar style power port", ActiveBarAction::PlacePowerBar),
            dd_item_svg(DD_PWR_CIRCLE, "Place Circle style power port", ActiveBarAction::PlacePowerCircle),
            dd_sep(),
            dd_item_svg(DD_GND, "Place Signal Ground power port", ActiveBarAction::PlacePowerSignalGND),
            dd_item_svg(DD_PWR_EARTH, "Place Earth power port", ActiveBarAction::PlacePowerEarth),
        ],
        ActiveBarMenu::Harness => vec![
            dd_item_svg(DD_HARNESS, "Signal Harness", ActiveBarAction::PlaceSignalHarness),
            dd_item_svg(DD_HARNESS_CONN, "Harness Connector", ActiveBarAction::PlaceHarnessConnector),
            dd_item_svg(DD_HARNESS, "Harness Entry", ActiveBarAction::PlaceHarnessEntry),
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

    container(column(items).spacing(0))
        .width(Length::Shrink)
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

/// Active Bar button: left-click activates tool, right-click opens dropdown.
fn ab_icon_btn(
    icon_bytes: &'static [u8],
    active: bool,
    left_click: ActiveBarMsg,
    right_click: Option<ActiveBarMsg>,
) -> Element<'static, ActiveBarMsg> {
    let handle = svg::Handle::from_memory(icon_bytes);

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

    let mut area = iced::widget::mouse_area(icon_widget)
        .on_press(left_click)
        .interaction(iced::mouse::Interaction::Pointer);

    if let Some(rc) = right_click {
        area = area.on_right_press(rc);
    }

    area.into()
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
        // Empty space placeholder to keep alignment
        r = r.push(Space::new().width(14).height(14));
    }

    r = r.push(
        text(label.to_string()).size(12).color(text_c),
    );

    button(r)
        .padding([5, 12])
        .width(Length::Fill)
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
