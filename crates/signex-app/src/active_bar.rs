//! Altium-style Active Bar — floating toolbar centered at top of canvas.
//!
//! 12 icon buttons, each with an optional dropdown menu.
//! Matches Altium Designer's schematic editor Active Bar exactly.

use iced::widget::{Space, button, column, container, row, svg, text};
use iced::{Background, Border, Color, Element, Theme};
use signex_types::theme::ThemeTokens;

use crate::styles;

/// Theme-derived colors for Active Bar chrome (all Copy+ʼstatic).
#[derive(Clone, Copy)]
struct AbColors {
    text: Color,
    bar_bg: Color,
    bar_border: Color,
    drop_bg: Color,
    drop_border: Color,
    sep: Color,
    hover: Color,
}

impl AbColors {
    fn from_tokens(tokens: &ThemeTokens) -> Self {
        Self {
            text: styles::ti(tokens.text),
            bar_bg: styles::ti(tokens.toolbar_bg),
            bar_border: styles::ti(tokens.border),
            drop_bg: styles::ti(tokens.panel_bg),
            drop_border: styles::ti(tokens.border),
            sep: styles::ti(tokens.border),
            hover: styles::ti(tokens.hover),
        }
    }
}

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
#[allow(dead_code)]
const ICON_ADDPART: &[u8] = include_bytes!("../assets/icons/addpart.svg");
const ICON_SHEETSYM: &[u8] = include_bytes!("../assets/icons/sheetsym.svg");

/// Active Bar total width in pixels (13 btns × 29px + 4 seps × 2px + 8px padding).
/// Each button cell is 28 px + 1 px row spacing. Each separator is 1 px wide
/// + 1 px spacing on each side. Container padding adds 4 px per horizontal edge.
pub const BAR_WIDTH_PX: f32 = 393.0;

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
    SelectMode, // Lasso, Inside Area, etc.
    Select,     // Move/transform
    Align,
    Wiring,
    Power,
    Harness,
    SheetSymbol, // Sheet Symbol, Sheet Entry, Device Sheet Symbol
    Port,
    Directives,
    TextTools,
    Shapes,
    NetColor, // 10
}

#[derive(Debug, Clone)]
pub enum ActiveBarMsg {
    ToggleMenu(ActiveBarMenu),
    CloseMenus,
    Action(ActiveBarAction),
    ToggleFilter(SelectionFilter),
    ToggleAllFilters,
}

/// Selection filter categories — each can be independently toggled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionFilter {
    Components,
    Wires,
    Buses,
    SheetSymbols,
    SheetEntries,
    NetLabels,
    Parameters,
    Ports,
    PowerPorts,
    Texts,
    DrawingObjects,
    Other,
}

impl SelectionFilter {
    pub const ALL: &'static [SelectionFilter] = &[
        Self::Components,
        Self::Wires,
        Self::Buses,
        Self::SheetSymbols,
        Self::SheetEntries,
        Self::NetLabels,
        Self::Parameters,
        Self::Ports,
        Self::PowerPorts,
        Self::Texts,
        Self::DrawingObjects,
        Self::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Components => "Components",
            Self::Wires => "Wires",
            Self::Buses => "Buses",
            Self::SheetSymbols => "Sheet Symbols",
            Self::SheetEntries => "Sheet Entries",
            Self::NetLabels => "Net Labels",
            Self::Parameters => "Parameters",
            Self::Ports => "Ports",
            Self::PowerPorts => "Power Ports",
            Self::Texts => "Texts",
            Self::DrawingObjects => "Drawing Objects",
            Self::Other => "Other",
        }
    }
}

/// All actions available from Active Bar buttons and dropdown items.
#[derive(Debug, Clone)]
#[allow(dead_code)]
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

/// Resolve the toolbar icon for the last-used action in a group.
fn action_icon(action: &ActiveBarAction) -> &'static [u8] {
    match action {
        // Wiring
        ActiveBarAction::DrawWire => DD_WIRE,
        ActiveBarAction::DrawBus => DD_BUS,
        ActiveBarAction::PlaceBusEntry => DD_BUS_ENTRY,
        ActiveBarAction::PlaceNetLabel => DD_NET_LABEL,
        // Power
        ActiveBarAction::PlacePowerGND => DD_GND,
        ActiveBarAction::PlacePowerVCC => DD_VCC,
        ActiveBarAction::PlacePowerPlus12 => DD_PWR_PLUS12,
        ActiveBarAction::PlacePowerPlus5 => DD_PWR_PLUS5,
        ActiveBarAction::PlacePowerMinus5 => DD_PWR_MINUS5,
        ActiveBarAction::PlacePowerArrow => DD_PWR_ARROW,
        ActiveBarAction::PlacePowerWave => DD_PWR_WAVE,
        ActiveBarAction::PlacePowerBar => DD_PWR_BAR,
        ActiveBarAction::PlacePowerCircle => DD_PWR_CIRCLE,
        ActiveBarAction::PlacePowerSignalGND => DD_PWR_SIG_GND,
        ActiveBarAction::PlacePowerEarth => DD_PWR_EARTH,
        // Port
        ActiveBarAction::PlacePort => DD_PORT,
        ActiveBarAction::PlaceOffSheetConnector => DD_OFF_SHEET,
        // Harness
        ActiveBarAction::PlaceSignalHarness => DD_HARNESS,
        ActiveBarAction::PlaceHarnessConnector => DD_HARNESS_CONN,
        ActiveBarAction::PlaceHarnessEntry => DD_HARNESS,
        // Sheet
        ActiveBarAction::PlaceSheetSymbol => DD_SHEET_SYM,
        ActiveBarAction::PlaceSheetEntry => DD_SHEET_ENTRY,
        ActiveBarAction::PlaceDeviceSheetSymbol => DD_DEVICE_SHEET,
        ActiveBarAction::PlaceReuseBlock => DD_REUSE_BLOCK,
        // Directives
        ActiveBarAction::PlaceParameterSet => DD_PARAM_SET,
        ActiveBarAction::PlaceNoERC => DD_NO_ERC,
        ActiveBarAction::PlaceDiffPair => DD_DIFF_PAIR,
        ActiveBarAction::PlaceBlanket => DD_BLANKET,
        ActiveBarAction::PlaceCompileMask => DD_BLANKET,
        // Text
        ActiveBarAction::PlaceTextString => DD_TEXT_STRING,
        ActiveBarAction::PlaceTextFrame => DD_TEXT_FRAME,
        ActiveBarAction::PlaceNote => DD_NOTE,
        // Shapes
        ActiveBarAction::DrawArc => DD_ARC,
        ActiveBarAction::DrawFullCircle => DD_CIRCLE,
        ActiveBarAction::DrawEllipticalArc => DD_ARC,
        ActiveBarAction::DrawEllipse => DD_ELLIPSE,
        ActiveBarAction::DrawLine => DD_LINE,
        ActiveBarAction::DrawRectangle => DD_RECT,
        ActiveBarAction::DrawRoundRectangle => DD_ROUND_RECT,
        ActiveBarAction::DrawPolygon => DD_POLYGON,
        ActiveBarAction::DrawBezier => DD_BEZIER,
        ActiveBarAction::PlaceGraphic => DD_GRAPHIC,
        // Fallback — use the group's default icon
        _ => ICON_SELECT,
    }
}

// ─── View: Active Bar ────────────────────────────────────────

/// Render the Active Bar (the floating toolbar strip).
pub fn view_bar(
    current_tool: crate::app::Tool,
    draw_mode: crate::app::DrawMode,
    last_tool: &std::collections::HashMap<String, ActiveBarAction>,
    tokens: &ThemeTokens,
) -> Element<'static, ActiveBarMsg> {
    let ac = AbColors::from_tokens(tokens);
    // Helper: get last-used action for a group, or use default
    let last = |group: &str, default: ActiveBarAction| -> ActiveBarMsg {
        ActiveBarMsg::Action(last_tool.get(group).cloned().unwrap_or(default))
    };
    // Helper: get the icon for the last-used action in a group, or fall back to default
    let last_icon = |group: &str, default_icon: &'static [u8]| -> &'static [u8] {
        last_tool
            .get(group)
            .map(|a| action_icon(a))
            .unwrap_or(default_icon)
    };
    let mut items: Vec<Element<'_, ActiveBarMsg>> = Vec::with_capacity(20);
    // 1. Filter — left: toggle, right: filter dropdown
    items.push(ab_icon_btn(
        ICON_FILTER,
        false,
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Filter),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Filter)),
        "Selection Filter",
    ));
    items.push(ab_icon_btn(
        ICON_MOVE,
        false,
        ActiveBarMsg::Action(ActiveBarAction::MoveSelection),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Select)),
        "Move / Transform",
    ));
    items.push(sep(ac.sep));

    items.push(ab_icon_btn(
        ICON_SELECT,
        current_tool == crate::app::Tool::Select,
        ActiveBarMsg::Action(ActiveBarAction::ToolSelect),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::SelectMode)),
        "Select",
    ));
    items.push(ab_icon_btn(
        ICON_ALIGN,
        false,
        ActiveBarMsg::Action(ActiveBarAction::AlignToGrid),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align)),
        "Align",
    ));
    items.push(sep(ac.sep));

    items.push(ab_icon_btn(
        last_icon("wiring", ICON_WIRE),
        current_tool == crate::app::Tool::Wire || current_tool == crate::app::Tool::Bus,
        last("wiring", ActiveBarAction::DrawWire),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Wiring)),
        "Wiring",
    ));
    items.push(ab_icon_btn(
        last_icon("power", ICON_POWER),
        false,
        last("power", ActiveBarAction::PlacePowerGND),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Power)),
        "Power Port",
    ));
    items.push(sep(ac.sep));

    items.push(ab_icon_btn(
        last_icon("harness", ICON_HARNESS),
        false,
        last("harness", ActiveBarAction::PlaceSignalHarness),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Harness)),
        "Harness",
    ));
    items.push(ab_icon_btn(
        last_icon("sheet", ICON_SHEETSYM),
        false,
        last("sheet", ActiveBarAction::PlaceSheetSymbol),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::SheetSymbol)),
        "Sheet Symbol",
    ));
    items.push(ab_icon_btn(
        last_icon("port", ICON_PORT),
        false,
        last("port", ActiveBarAction::PlacePort),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Port)),
        "Port / Connector",
    ));
    items.push(ab_icon_btn(
        last_icon("directives", ICON_DIRECTIVES),
        false,
        last("directives", ActiveBarAction::PlaceParameterSet),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Directives)),
        "Directives",
    ));
    items.push(sep(ac.sep));

    items.push(ab_icon_btn(
        last_icon("text", ICON_TEXT),
        current_tool == crate::app::Tool::Text,
        last("text", ActiveBarAction::PlaceTextString),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::TextTools)),
        "Text",
    ));
    items.push(ab_icon_btn(
        last_icon("shapes", ICON_SHAPES),
        matches!(
            current_tool,
            crate::app::Tool::Line | crate::app::Tool::Rectangle | crate::app::Tool::Circle
        ),
        last("shapes", ActiveBarAction::DrawLine),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Shapes)),
        "Drawing Tools",
    ));
    items.push(ab_icon_btn(
        ICON_NETCOLOR,
        false,
        ActiveBarMsg::Action(ActiveBarAction::ToolSelect),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::NetColor)),
        "Net Color",
    ));

    // Draw mode indicator
    if matches!(current_tool, crate::app::Tool::Wire | crate::app::Tool::Bus) {
        items.push(sep(ac.sep));
        let mode_label = match draw_mode {
            crate::app::DrawMode::Ortho90 => "90\u{00B0}",
            crate::app::DrawMode::Angle45 => "45\u{00B0}",
            crate::app::DrawMode::FreeAngle => "Any",
        };
        items.push(
            button(text(mode_label.to_string()).size(12).color(Color::WHITE))
                .padding([5, 7])
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
        .style(move |_: &Theme| container::Style {
            background: Some(ac.bar_bg.into()),
            text_color: Some(ac.text),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: ac.bar_border,
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
pub fn view_dropdown(
    menu: ActiveBarMenu,
    tokens: &ThemeTokens,
    filters: &std::collections::HashSet<SelectionFilter>,
) -> Element<'static, ActiveBarMsg> {
    let ac = AbColors::from_tokens(tokens);
    let items: Vec<Element<'_, ActiveBarMsg>> = match menu {
        ActiveBarMenu::Filter => {
            // Altium-style tag buttons for selection filter
            let text_primary = ac.text;
            let hover_c = ac.hover;
            let all_on = filters.len() == SelectionFilter::ALL.len();
            let tag = |filter: SelectionFilter, enabled: bool| -> Element<'static, ActiveBarMsg> {
                let label = filter.label();
                let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
                let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
                let active_border = Color::from_rgba8(0x4D, 0x52, 0x66, 1.0);
                let inactive_border = Color::from_rgba8(0x33, 0x36, 0x44, 1.0);
                let text_on = text_primary;
                let text_off = Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0);
                button(text(label.to_string()).size(11).color(if enabled {
                    text_on
                } else {
                    text_off
                }))
                .padding([4, 10])
                .on_press(ActiveBarMsg::ToggleFilter(filter))
                .style(move |_: &Theme, status: button::Status| {
                    let bg = match status {
                        button::Status::Hovered => Background::Color(hover_c),
                        _ => Background::Color(if enabled { active_bg } else { inactive_bg }),
                    };
                    button::Style {
                        background: Some(bg),
                        border: Border {
                            width: 1.0,
                            radius: 12.0.into(),
                            color: if enabled {
                                active_border
                            } else {
                                inactive_border
                            },
                        },
                        text_color: if enabled { text_on } else { text_off },
                        ..button::Style::default()
                    }
                })
                .into()
            };
            let all_label = if all_on { "All - On" } else { "All - Off" };
            // All-On/Off as a real toggle button (pill styling matches tag row).
            let all_active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
            let all_inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
            let all_active_border = Color::from_rgba8(0x4D, 0x52, 0x66, 1.0);
            let all_inactive_border = Color::from_rgba8(0x33, 0x36, 0x44, 1.0);
            let all_text_off = Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0);
            let all_toggle = button(text(all_label.to_string()).size(11).color(if all_on {
                text_primary
            } else {
                all_text_off
            }))
            .padding([4, 12])
            .on_press(ActiveBarMsg::ToggleAllFilters)
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered => Background::Color(hover_c),
                    _ => Background::Color(if all_on {
                        all_active_bg
                    } else {
                        all_inactive_bg
                    }),
                };
                button::Style {
                    background: Some(bg),
                    border: Border {
                        width: 1.0,
                        radius: 12.0.into(),
                        color: if all_on {
                            all_active_border
                        } else {
                            all_inactive_border
                        },
                    },
                    text_color: if all_on { text_primary } else { all_text_off },
                    ..button::Style::default()
                }
            });
            // 3-row layout: row 1 = All toggle. Rows 2 & 3 split the 12 filters 6+6.
            let filter_content: Element<'static, ActiveBarMsg> = column![
                container(all_toggle).padding([4, 8]),
                container(
                    column![
                        row![
                            tag(
                                SelectionFilter::Components,
                                filters.contains(&SelectionFilter::Components)
                            ),
                            tag(
                                SelectionFilter::Wires,
                                filters.contains(&SelectionFilter::Wires)
                            ),
                            tag(
                                SelectionFilter::Buses,
                                filters.contains(&SelectionFilter::Buses)
                            ),
                            tag(
                                SelectionFilter::SheetSymbols,
                                filters.contains(&SelectionFilter::SheetSymbols)
                            ),
                            tag(
                                SelectionFilter::SheetEntries,
                                filters.contains(&SelectionFilter::SheetEntries)
                            ),
                            tag(
                                SelectionFilter::NetLabels,
                                filters.contains(&SelectionFilter::NetLabels)
                            ),
                        ]
                        .spacing(4),
                        row![
                            tag(
                                SelectionFilter::Parameters,
                                filters.contains(&SelectionFilter::Parameters)
                            ),
                            tag(
                                SelectionFilter::Ports,
                                filters.contains(&SelectionFilter::Ports)
                            ),
                            tag(
                                SelectionFilter::PowerPorts,
                                filters.contains(&SelectionFilter::PowerPorts)
                            ),
                            tag(
                                SelectionFilter::Texts,
                                filters.contains(&SelectionFilter::Texts)
                            ),
                            tag(
                                SelectionFilter::DrawingObjects,
                                filters.contains(&SelectionFilter::DrawingObjects)
                            ),
                            tag(
                                SelectionFilter::Other,
                                filters.contains(&SelectionFilter::Other)
                            ),
                        ]
                        .spacing(4),
                    ]
                    .spacing(4),
                )
                .padding([4, 8]),
            ]
            .spacing(2)
            .into();
            vec![filter_content]
        }
        ActiveBarMenu::SelectMode => vec![
            dd_item(
                "Lasso Select",
                ActiveBarAction::LassoSelect,
                ac.text,
                ac.hover,
            ),
            dd_item(
                "Inside Area",
                ActiveBarAction::InsideArea,
                ac.text,
                ac.hover,
            ),
            dd_item(
                "Outside Area",
                ActiveBarAction::OutsideArea,
                ac.text,
                ac.hover,
            ),
            dd_item(
                "Touching Rectangle",
                ActiveBarAction::TouchingRectangle,
                ac.text,
                ac.hover,
            ),
            dd_item(
                "Touching Line",
                ActiveBarAction::TouchingLine,
                ac.text,
                ac.hover,
            ),
            dd_item("All", ActiveBarAction::SelectAll, ac.text, ac.hover),
            dd_item(
                "Connection",
                ActiveBarAction::SelectConnection,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item(
                "Toggle Selection",
                ActiveBarAction::ToggleSelection,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Select => vec![
            dd_item_svg(DD_DRAG, "Drag", ActiveBarAction::Drag, ac.text, ac.hover),
            dd_item_svg(
                DD_DRAG,
                "Move",
                ActiveBarAction::MoveSelection,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_MOVE_SEL,
                "Move Selection",
                ActiveBarAction::MoveSelection,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_MOVE_XY,
                "Move Selection by X, Y...",
                ActiveBarAction::MoveSelectionXY,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_DRAG,
                "Drag Selection",
                ActiveBarAction::DragSelection,
                ac.text,
                ac.hover,
            ),
            dd_item(
                "Move To Front",
                ActiveBarAction::MoveToFront,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_ROTATE,
                "Rotate Selection",
                ActiveBarAction::RotateSelection,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_ROTATE_CW,
                "Rotate Selection Clockwise",
                ActiveBarAction::RotateSelectionCW,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                DD_BRING_FRONT,
                "Bring To Front",
                ActiveBarAction::BringToFront,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_SEND_BACK,
                "Send To Back",
                ActiveBarAction::SendToBack,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_BRING_FRONT,
                "Bring To Front Of",
                ActiveBarAction::BringToFrontOf,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_SEND_BACK,
                "Send To Back Of",
                ActiveBarAction::SendToBackOf,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                DD_FLIP_X,
                "Flip Selected Sheet Symbols Along X",
                ActiveBarAction::FlipSelectedX,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_FLIP_Y,
                "Flip Selected Sheet Symbols Along Y",
                ActiveBarAction::FlipSelectedY,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Align => vec![
            dd_item_svg(
                DD_ALIGN_LEFT,
                "Align Left",
                ActiveBarAction::AlignLeft,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_ALIGN_RIGHT,
                "Align Right",
                ActiveBarAction::AlignRight,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_ALIGN_HCENTER,
                "Align Horizontal Centers",
                ActiveBarAction::AlignHorizontalCenters,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_DIST_HORIZ,
                "Distribute Horizontally",
                ActiveBarAction::DistributeHorizontally,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                DD_ALIGN_TOP,
                "Align Top",
                ActiveBarAction::AlignTop,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_ALIGN_BOTTOM,
                "Align Bottom",
                ActiveBarAction::AlignBottom,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_ALIGN_VCENTER,
                "Align Vertical Centers",
                ActiveBarAction::AlignVerticalCenters,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_DIST_VERT,
                "Distribute Vertically",
                ActiveBarAction::DistributeVertically,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                DD_ALIGN_GRID,
                "Align To Grid",
                ActiveBarAction::AlignToGrid,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Wiring => vec![
            dd_item_svg(
                DD_WIRE,
                "Wire",
                ActiveBarAction::DrawWire,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(DD_BUS, "Bus", ActiveBarAction::DrawBus, ac.text, ac.hover),
            dd_item_svg(
                DD_BUS_ENTRY,
                "Bus Entry",
                ActiveBarAction::PlaceBusEntry,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_NET_LABEL,
                "Net Label",
                ActiveBarAction::PlaceNetLabel,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Power => vec![
            dd_item_svg(
                DD_GND,
                "Place GND power port",
                ActiveBarAction::PlacePowerGND,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_VCC,
                "Place VCC power port",
                ActiveBarAction::PlacePowerVCC,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_PWR_PLUS12,
                "Place +12 power port",
                ActiveBarAction::PlacePowerPlus12,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_PWR_PLUS5,
                "Place +5 power port",
                ActiveBarAction::PlacePowerPlus5,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_PWR_MINUS5,
                "Place -5 power port",
                ActiveBarAction::PlacePowerMinus5,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                DD_PWR_ARROW,
                "Place Arrow style power port",
                ActiveBarAction::PlacePowerArrow,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_PWR_WAVE,
                "Place Wave style power port",
                ActiveBarAction::PlacePowerWave,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_PWR_BAR,
                "Place Bar style power port",
                ActiveBarAction::PlacePowerBar,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_PWR_CIRCLE,
                "Place Circle style power port",
                ActiveBarAction::PlacePowerCircle,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                DD_PWR_SIG_GND,
                "Place Signal Ground power port",
                ActiveBarAction::PlacePowerSignalGND,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_PWR_EARTH,
                "Place Earth power port",
                ActiveBarAction::PlacePowerEarth,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Harness => vec![
            dd_item_svg(
                DD_HARNESS,
                "Signal Harness",
                ActiveBarAction::PlaceSignalHarness,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_HARNESS_CONN,
                "Harness Connector",
                ActiveBarAction::PlaceHarnessConnector,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_HARNESS,
                "Harness Entry",
                ActiveBarAction::PlaceHarnessEntry,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::SheetSymbol => vec![
            dd_item_svg(
                DD_SHEET_SYM,
                "Sheet Symbol",
                ActiveBarAction::PlaceSheetSymbol,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_SHEET_ENTRY,
                "Sheet Entry",
                ActiveBarAction::PlaceSheetEntry,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_DEVICE_SHEET,
                "Device Sheet Symbol",
                ActiveBarAction::PlaceDeviceSheetSymbol,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_REUSE_BLOCK,
                "Reuse Block...",
                ActiveBarAction::PlaceReuseBlock,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Port => vec![
            dd_item_svg(
                DD_PORT,
                "Port",
                ActiveBarAction::PlacePort,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_OFF_SHEET,
                "Off Sheet Connector",
                ActiveBarAction::PlaceOffSheetConnector,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Directives => vec![
            dd_item_svg(
                DD_PARAM_SET,
                "Parameter Set",
                ActiveBarAction::PlaceParameterSet,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_NO_ERC,
                "Generic No ERC",
                ActiveBarAction::PlaceNoERC,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_DIFF_PAIR,
                "Differential Pair",
                ActiveBarAction::PlaceDiffPair,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_BLANKET,
                "Blanket",
                ActiveBarAction::PlaceBlanket,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_BLANKET,
                "Compile Mask",
                ActiveBarAction::PlaceCompileMask,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::TextTools => vec![
            dd_item_svg(
                DD_TEXT_STRING,
                "Text String",
                ActiveBarAction::PlaceTextString,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_TEXT_FRAME,
                "Text Frame",
                ActiveBarAction::PlaceTextFrame,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_NOTE,
                "Note",
                ActiveBarAction::PlaceNote,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Shapes => vec![
            dd_item_svg(DD_ARC, "Arc", ActiveBarAction::DrawArc, ac.text, ac.hover),
            dd_item_svg(
                DD_CIRCLE,
                "Full Circle",
                ActiveBarAction::DrawFullCircle,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_ARC,
                "Elliptical Arc",
                ActiveBarAction::DrawEllipticalArc,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_ELLIPSE,
                "Ellipse",
                ActiveBarAction::DrawEllipse,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                DD_LINE,
                "Line",
                ActiveBarAction::DrawLine,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_RECT,
                "Rectangle",
                ActiveBarAction::DrawRectangle,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_ROUND_RECT,
                "Round Rectangle",
                ActiveBarAction::DrawRoundRectangle,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_POLYGON,
                "Polygon",
                ActiveBarAction::DrawPolygon,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                DD_BEZIER,
                "Bezier",
                ActiveBarAction::DrawBezier,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                DD_GRAPHIC,
                "Graphic...",
                ActiveBarAction::PlaceGraphic,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::NetColor => {
            let color_item = |label: &str,
                              color: Color,
                              action: ActiveBarAction|
             -> Element<'static, ActiveBarMsg> {
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
                        text(label.to_string())
                            .size(13)
                            .color(ac.text)
                            .wrapping(iced::widget::text::Wrapping::None),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                )
                .padding([5, 12])
                .on_press(ActiveBarMsg::Action(action))
                .style(dd_btn_style_f(ac.text, ac.hover))
                .into()
            };
            vec![
                color_item(
                    "Blue",
                    Color::from_rgb(0.40, 0.40, 0.93),
                    ActiveBarAction::NetColorBlue,
                ),
                color_item(
                    "Light Green",
                    Color::from_rgb(0.40, 0.93, 0.40),
                    ActiveBarAction::NetColorLightGreen,
                ),
                color_item(
                    "Light Blue",
                    Color::from_rgb(0.40, 0.80, 0.93),
                    ActiveBarAction::NetColorLightBlue,
                ),
                color_item(
                    "Red",
                    Color::from_rgb(0.93, 0.30, 0.30),
                    ActiveBarAction::NetColorRed,
                ),
                color_item(
                    "Fuchsia",
                    Color::from_rgb(0.80, 0.30, 0.80),
                    ActiveBarAction::NetColorFuchsia,
                ),
                color_item(
                    "Yellow",
                    Color::from_rgb(0.93, 0.80, 0.20),
                    ActiveBarAction::NetColorYellow,
                ),
                color_item(
                    "Dark Green",
                    Color::from_rgb(0.13, 0.55, 0.13),
                    ActiveBarAction::NetColorDarkGreen,
                ),
                dd_sep(ac.sep),
                dd_item(
                    "Clear Net Color",
                    ActiveBarAction::ClearNetColor,
                    ac.text,
                    ac.hover,
                ),
                dd_item(
                    "Clear All Net Colors",
                    ActiveBarAction::ClearAllNetColors,
                    ac.text,
                    ac.hover,
                ),
            ]
        }
    };

    // Auto-sized: iced shrinks the column to its widest child as long
    // as we don't wrap it in a Fill / Shrink-stacked chain that clamps
    // it. The outer positioning now uses `Translate` (see view/mod.rs)
    // so this container sits free of row/Space layout interactions.
    container(column(items).spacing(0))
        .padding([6, 0])
        .style(move |_: &Theme| container::Style {
            background: Some(ac.drop_bg.into()),
            text_color: Some(ac.text),
            border: Border {
                width: 1.0,
                radius: 6.0.into(),
                color: ac.drop_border,
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
    // Each icon = 28px mouse_area + 1px spacing = 29px per button
    // Separator = 1px + 1px spacing = 2px
    // Bar padding = [3, 4] → 4px left padding
    // Layout: [Filter][Move] | [Select][Align] | [Wire][Power] | [Harness][Sheet][Port][Dir] | [Text][Shapes][NetColor]
    //  btn:     0      1    s    2      3     s   4      5    s    6      7     8    9    s  10     11     12
    let btn = 29.0_f32;
    let s = 2.0_f32;
    let pad = 4.0_f32;
    pad + match menu {
        ActiveBarMenu::Filter => 0.0,
        ActiveBarMenu::Select => btn,
        ActiveBarMenu::SelectMode => 2.0 * btn + s,
        ActiveBarMenu::Align => 3.0 * btn + s,
        ActiveBarMenu::Wiring => 4.0 * btn + 2.0 * s,
        ActiveBarMenu::Power => 5.0 * btn + 2.0 * s,
        ActiveBarMenu::Harness => 6.0 * btn + 3.0 * s,
        ActiveBarMenu::SheetSymbol => 7.0 * btn + 3.0 * s,
        ActiveBarMenu::Port => 8.0 * btn + 3.0 * s,
        ActiveBarMenu::Directives => 9.0 * btn + 3.0 * s,
        ActiveBarMenu::TextTools => 10.0 * btn + 4.0 * s,
        ActiveBarMenu::Shapes => 11.0 * btn + 4.0 * s,
        ActiveBarMenu::NetColor => 12.0 * btn + 4.0 * s,
    }
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
        let chevron = svg(svg::Handle::from_memory(CHEVRON_45)).width(8).height(8);
        iced::widget::Stack::new()
            .push(
                container(svg(handle).width(20).height(20))
                    .width(28)
                    .height(28)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .push(
                container(chevron)
                    .width(28)
                    .height(28)
                    .align_x(iced::alignment::Horizontal::Right)
                    .align_y(iced::alignment::Vertical::Bottom),
            )
            .into()
    } else {
        container(svg(handle).width(20).height(20))
            .width(28)
            .height(28)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    };

    // Use a button for left-click (reliable event delivery) and wrap
    // with mouse_area for right-click (dropdown toggle).
    let left_msg = left_click;
    let btn = button(icon_content).padding(0).on_press(left_msg).style(
        move |_: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered => Color::from_rgb(0.26, 0.27, 0.34),
                _ if active => Color::from_rgb(0.22, 0.23, 0.30),
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    color: Color::TRANSPARENT,
                },
                ..button::Style::default()
            }
        },
    );

    let widget: Element<'static, ActiveBarMsg> = if let Some(rc) = right_click {
        iced::widget::mouse_area(btn).on_right_press(rc).into()
    } else {
        btn.into()
    };

    let tip = container(
        text(tooltip_text)
            .size(11)
            .color(Color::from_rgb(0.85, 0.85, 0.88)),
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

    iced::widget::tooltip(widget, tip, iced::widget::tooltip::Position::Bottom)
        .gap(4)
        .into()
}

fn sep(sep_c: Color) -> Element<'static, ActiveBarMsg> {
    container(Space::new())
        .width(1)
        .height(22)
        .style(move |_: &Theme| container::Style {
            background: Some(sep_c.into()),
            ..container::Style::default()
        })
        .into()
}

/// Dropdown item with optional inline SVG icon (Altium-style).
fn dd_item(
    label: &str,
    action: ActiveBarAction,
    text_c: Color,
    hover_c: Color,
) -> Element<'static, ActiveBarMsg> {
    dd_item_icon(None, label, text_c, hover_c, action)
}

/// Dropdown item with colored icon SVG bytes.
fn dd_item_svg(
    icon: &'static [u8],
    label: &str,
    action: ActiveBarAction,
    text_c: Color,
    hover_c: Color,
) -> Element<'static, ActiveBarMsg> {
    dd_item_icon(Some(icon), label, text_c, hover_c, action)
}

fn dd_item_icon(
    icon: Option<&'static [u8]>,
    label: &str,
    text_c: Color,
    hover_c: Color,
    action: ActiveBarAction,
) -> Element<'static, ActiveBarMsg> {
    let mut r = iced::widget::Row::new()
        .spacing(8)
        .align_y(iced::Alignment::Center);

    if let Some(icon_bytes) = icon {
        let handle = svg::Handle::from_memory(icon_bytes);
        r = r.push(svg(handle).width(18).height(18));
    } else {
        r = r.push(Space::new().width(18).height(18));
    }

    r = r.push(
        text(label.to_string())
            .size(13)
            .color(text_c)
            .wrapping(iced::widget::text::Wrapping::None),
    );

    button(r)
        .padding([5, 12])
        .on_press(ActiveBarMsg::Action(action))
        .style(dd_btn_style_f(text_c, hover_c))
        .into()
}

fn dd_sep(sep_c: Color) -> Element<'static, ActiveBarMsg> {
    // No Fill on the outer container — that would expand the parent column.
    // The inner 1px line uses a styled container with bottom-padding trick
    // (same approach as tab underlines) to avoid Length::Fill.
    container(Space::new())
        .height(1)
        .padding(iced::Padding {
            top: 3.0,
            right: 8.0,
            bottom: 3.0,
            left: 8.0,
        })
        .style(move |_: &Theme| container::Style {
            background: Some(sep_c.into()),
            ..container::Style::default()
        })
        .into()
}

fn dd_btn_style_f(
    text_c: Color,
    hover_c: Color,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => Some(Background::Color(hover_c)),
            _ => None,
        };
        button::Style {
            background: bg,
            border: Border::default(),
            text_color: text_c,
            ..button::Style::default()
        }
    }
}
