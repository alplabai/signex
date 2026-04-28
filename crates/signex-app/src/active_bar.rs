//! Altium-style Active Bar — floating toolbar centered at top of canvas.
//!
//! 12 icon buttons, each with an optional dropdown menu.
//! Matches Altium Designer's schematic editor Active Bar exactly.

use std::cell::Cell;

use iced::widget::{Space, button, column, container, row, svg, text};
use iced::{Background, Border, Color, Element, Theme};
use signex_types::theme::{ThemeId, ThemeTokens};

use crate::styles;

thread_local! {
    /// True when the active schematic has at least one selected item.
    /// `view_bar` and `view_dropdown` install a `HasSelectionGuard`
    /// for the duration of one render so `ab_icon_btn` / `dd_item_icon`
    /// can grey out selection-dependent actions (transform, align,
    /// distribute) without threading the flag through 77+ call sites.
    static HAS_SELECTION_FOR_VIEW: Cell<bool> = const { Cell::new(true) };
    /// True when the active schematic has at least one net with a
    /// custom colour applied. Same purpose as `HAS_SELECTION_FOR_VIEW`
    /// but for the NetColor "clear" actions.
    static HAS_NET_COLORS_FOR_VIEW: Cell<bool> = const { Cell::new(true) };
}

/// RAII guard that publishes both `has_selection` and `has_net_colors`
/// to helpers for the duration of one render. Resets to `true` (the
/// "no gating, everything enabled" default) on drop so any caller that
/// reaches `dd_item_icon` outside a `view_dropdown` invocation still
/// sees a fully-enabled UI.
struct HasSelectionGuard;
impl HasSelectionGuard {
    fn enter(has_selection: bool, has_net_colors: bool) -> Self {
        HAS_SELECTION_FOR_VIEW.with(|c| c.set(has_selection));
        HAS_NET_COLORS_FOR_VIEW.with(|c| c.set(has_net_colors));
        Self
    }
}
impl Drop for HasSelectionGuard {
    fn drop(&mut self) {
        HAS_SELECTION_FOR_VIEW.with(|c| c.set(true));
        HAS_NET_COLORS_FOR_VIEW.with(|c| c.set(true));
    }
}

fn current_has_selection() -> bool {
    HAS_SELECTION_FOR_VIEW.with(|c| c.get())
}

fn current_has_net_colors() -> bool {
    HAS_NET_COLORS_FOR_VIEW.with(|c| c.get())
}

/// Whether `action` needs at least one selected item to make sense.
/// Transform / align / distribute family — Altium greys these out when
/// the selection is empty. Net-colour picks are excluded because the
/// NetColor flow is "arm-then-apply", not "act on selection".
pub fn requires_selection(action: &ActiveBarAction) -> bool {
    use ActiveBarAction::*;
    matches!(
        action,
        Drag | DragSelection
            | MoveSelection
            | MoveSelectionXY
            | MoveToFront
            | RotateSelection
            | RotateSelectionCW
            | FlipSelectedX
            | FlipSelectedY
            | BringToFront
            | BringToFrontOf
            | SendToBack
            | SendToBackOf
            | AlignLeft
            | AlignRight
            | AlignTop
            | AlignBottom
            | AlignHorizontalCenters
            | AlignVerticalCenters
            | AlignToGrid
            | DistributeHorizontally
            | DistributeVertically
    )
}

/// Whether `action` only makes sense when at least one net carries a
/// custom colour override. The Clear / Clear-All Net Color actions go
/// here; the seven NetColor pickers and Custom Color stay always-on
/// (they're the "arm" phase that paints colours onto nets).
pub fn requires_net_color(action: &ActiveBarAction) -> bool {
    use ActiveBarAction::*;
    matches!(action, ClearNetColor | ClearAllNetColors)
}

fn action_enabled(action: &ActiveBarAction) -> bool {
    if requires_selection(action) && !current_has_selection() {
        return false;
    }
    if requires_net_color(action) && !current_has_net_colors() {
        return false;
    }
    true
}

/// Muted text colour used for disabled dropdown items / bar cells.
/// Chosen to match the inactive label colour used by the chip + tab
/// styles elsewhere in the active bar.
const DISABLED_TEXT: Color = Color {
    r: 0x66 as f32 / 255.0,
    g: 0x6A as f32 / 255.0,
    b: 0x7E as f32 / 255.0,
    a: 1.0,
};

/// Theme-derived colors for Active Bar chrome (all Copy+ʼstatic).
/// `bar_bg` / `bar_border` were used by the bespoke bar container;
/// since `view_bar` now delegates to
/// `signex_widgets::active_bar::view`, those fields are unused but
/// kept on the struct so the dropdown helpers below don't have to
/// rebuild a separate palette.
#[derive(Clone, Copy)]
#[allow(dead_code)]
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

// ─── Icons ───────────────────────────────────────────────────
//
// Icon handles are resolved through `crate::icons`, which threads the
// active `ThemeId` through every lookup so the accent colour tints to
// the current theme without copying icon trees. All former `ICON_*`
// and `DD_*` const-byte declarations now live in that module.

use crate::icons as ic;

/// Active Bar total width in pixels.
///
/// Layout: 13 buttons (26 px) + 4 separators (2 px wide) = 17 cells
/// with 2 px spacing between each (16 gaps) and 2 px padding per edge.
/// 13·26 + 4·2 + 16·2 + 4 = 338 + 8 + 32 + 4 = 382 px.
pub const BAR_WIDTH_PX: f32 = 382.0;

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
    /// Replace the active filter set with the user-defined preset at
    /// `index` (0..N). No-op if the index is out of range or the slot is
    /// empty. Source: shortcut buttons in the Filter dropdown.
    ApplyCustomFilter(usize),
}

/// Maximum number of user-defined custom filter presets exposed in the
/// Active Bar's filter dropdown and managed in the Properties panel.
pub const CUSTOM_FILTER_PRESET_LIMIT: usize = 4;

/// A user-defined named selection-filter preset. Persisted to
/// `~/.config/signex/prefs.json` under `custom_filter_presets` and
/// surfaced as a shortcut button in the Active Bar's filter dropdown.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CustomFilterPreset {
    pub name: String,
    /// Stored as `Vec` (not `HashSet`) so the on-disk representation
    /// has a stable order and a sensible JSON shape.
    pub filters: Vec<SelectionFilter>,
}

impl CustomFilterPreset {
    /// Snapshot the active filter set into a new preset with a default name.
    pub fn capture(name: String, filters: &std::collections::HashSet<SelectionFilter>) -> Self {
        // Keep the canonical order of `SelectionFilter::ALL` so two
        // captures of the same set are byte-identical on disk.
        let filters: Vec<SelectionFilter> = SelectionFilter::ALL
            .iter()
            .copied()
            .filter(|f| filters.contains(f))
            .collect();
        Self { name, filters }
    }

    /// Realize the preset's `Vec` into a `HashSet` for assignment back
    /// into `InteractionState::selection_filters`.
    pub fn as_set(&self) -> std::collections::HashSet<SelectionFilter> {
        self.filters.iter().copied().collect()
    }
}

/// Selection filter categories — each can be independently toggled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    /// Open the custom-colour picker modal for net-colour arming.
    NetColorCustom,
    ClearNetColor,
    ClearAllNetColors,
    // Component
    PlaceComponent,
}

/// Resolve the toolbar icon for the last-used action in a group.
fn action_icon(action: &ActiveBarAction, tid: ThemeId) -> svg::Handle {
    match action {
        // Base select cursor
        ActiveBarAction::ToolSelect => ic::icon_select(tid),
        // Align / Distribute
        ActiveBarAction::AlignLeft => ic::icon_dd_align_left(tid),
        ActiveBarAction::AlignRight => ic::icon_dd_align_right(tid),
        ActiveBarAction::AlignTop => ic::icon_dd_align_top(tid),
        ActiveBarAction::AlignBottom => ic::icon_dd_align_bottom(tid),
        ActiveBarAction::AlignHorizontalCenters => ic::icon_dd_align_hcenter(tid),
        ActiveBarAction::AlignVerticalCenters => ic::icon_dd_align_vcenter(tid),
        ActiveBarAction::AlignToGrid => ic::icon_dd_align_grid(tid),
        ActiveBarAction::DistributeHorizontally => ic::icon_dd_dist_horiz(tid),
        ActiveBarAction::DistributeVertically => ic::icon_dd_dist_vert(tid),
        // Net colours — fall back to the 4-quadrant palette glyph since
        // a per-colour icon would just be a coloured square.
        ActiveBarAction::NetColorBlue
        | ActiveBarAction::NetColorRed
        | ActiveBarAction::NetColorLightGreen
        | ActiveBarAction::NetColorLightBlue
        | ActiveBarAction::NetColorFuchsia
        | ActiveBarAction::NetColorYellow
        | ActiveBarAction::NetColorDarkGreen => ic::icon_netcolor(tid),
        // Selection modes
        ActiveBarAction::LassoSelect => ic::icon_dd_select_lasso(tid),
        ActiveBarAction::InsideArea => ic::icon_dd_select_inside(tid),
        ActiveBarAction::OutsideArea => ic::icon_dd_select_outside(tid),
        ActiveBarAction::TouchingRectangle => ic::icon_dd_select_touching_rect(tid),
        ActiveBarAction::TouchingLine => ic::icon_dd_select_touching_line(tid),
        ActiveBarAction::SelectAll => ic::icon_dd_select_all(tid),
        ActiveBarAction::SelectConnection => ic::icon_dd_select_connection(tid),
        ActiveBarAction::ToggleSelection => ic::icon_dd_select_toggle(tid),
        // Move / Transform
        ActiveBarAction::Drag => ic::icon_dd_drag(tid),
        ActiveBarAction::DragSelection => ic::icon_dd_drag_sel(tid),
        ActiveBarAction::MoveSelection => ic::icon_dd_move(tid),
        ActiveBarAction::MoveSelectionXY => ic::icon_dd_move_xy(tid),
        ActiveBarAction::MoveToFront => ic::icon_dd_move_to_front(tid),
        ActiveBarAction::RotateSelection => ic::icon_dd_rotate(tid),
        ActiveBarAction::RotateSelectionCW => ic::icon_dd_rotate_cw(tid),
        ActiveBarAction::FlipSelectedX => ic::icon_dd_flip_x(tid),
        ActiveBarAction::FlipSelectedY => ic::icon_dd_flip_y(tid),
        ActiveBarAction::BringToFront => ic::icon_dd_bring_front(tid),
        ActiveBarAction::BringToFrontOf => ic::icon_dd_bring_front_of(tid),
        ActiveBarAction::SendToBack => ic::icon_dd_send_back(tid),
        ActiveBarAction::SendToBackOf => ic::icon_dd_send_back_of(tid),
        // Net colour
        ActiveBarAction::NetColorCustom => ic::icon_dd_net_color_custom(tid),
        ActiveBarAction::ClearNetColor => ic::icon_dd_net_color_clear(tid),
        ActiveBarAction::ClearAllNetColors => ic::icon_dd_net_color_clear_all(tid),
        // Wiring
        ActiveBarAction::DrawWire => ic::icon_dd_wire(tid),
        ActiveBarAction::DrawBus => ic::icon_dd_bus(tid),
        ActiveBarAction::PlaceBusEntry => ic::icon_dd_bus_entry(tid),
        ActiveBarAction::PlaceNetLabel => ic::icon_dd_net_label(tid),
        // Power
        ActiveBarAction::PlacePowerGND => ic::icon_dd_gnd(tid),
        ActiveBarAction::PlacePowerVCC => ic::icon_dd_vcc(tid),
        ActiveBarAction::PlacePowerPlus12 => ic::icon_dd_pwr_plus12(tid),
        ActiveBarAction::PlacePowerPlus5 => ic::icon_dd_pwr_plus5(tid),
        ActiveBarAction::PlacePowerMinus5 => ic::icon_dd_pwr_minus5(tid),
        ActiveBarAction::PlacePowerArrow => ic::icon_dd_pwr_arrow(tid),
        ActiveBarAction::PlacePowerWave => ic::icon_dd_pwr_wave(tid),
        ActiveBarAction::PlacePowerBar => ic::icon_dd_pwr_bar(tid),
        ActiveBarAction::PlacePowerCircle => ic::icon_dd_pwr_circle(tid),
        ActiveBarAction::PlacePowerSignalGND => ic::icon_dd_pwr_signal_gnd(tid),
        ActiveBarAction::PlacePowerEarth => ic::icon_dd_pwr_earth(tid),
        // Port
        ActiveBarAction::PlacePort => ic::icon_dd_port(tid),
        ActiveBarAction::PlaceOffSheetConnector => ic::icon_dd_off_sheet(tid),
        // Harness
        ActiveBarAction::PlaceSignalHarness => ic::icon_dd_harness(tid),
        ActiveBarAction::PlaceHarnessConnector => ic::icon_dd_harness_conn(tid),
        ActiveBarAction::PlaceHarnessEntry => ic::icon_dd_harness_entry(tid),
        // Sheet
        ActiveBarAction::PlaceSheetSymbol => ic::icon_dd_sheet_symbol(tid),
        ActiveBarAction::PlaceSheetEntry => ic::icon_dd_sheet_entry(tid),
        ActiveBarAction::PlaceDeviceSheetSymbol => ic::icon_dd_device_sheet(tid),
        ActiveBarAction::PlaceReuseBlock => ic::icon_dd_reuse_block(tid),
        // Directives
        ActiveBarAction::PlaceParameterSet => ic::icon_dd_param_set(tid),
        ActiveBarAction::PlaceNoERC => ic::icon_dd_no_erc(tid),
        ActiveBarAction::PlaceDiffPair => ic::icon_dd_diff_pair(tid),
        ActiveBarAction::PlaceBlanket => ic::icon_dd_blanket(tid),
        ActiveBarAction::PlaceCompileMask => ic::icon_dd_blanket(tid),
        // Text
        ActiveBarAction::PlaceTextString => ic::icon_dd_text_string(tid),
        ActiveBarAction::PlaceTextFrame => ic::icon_dd_text_frame(tid),
        ActiveBarAction::PlaceNote => ic::icon_dd_note(tid),
        // Shapes
        ActiveBarAction::DrawArc => ic::icon_dd_arc(tid),
        ActiveBarAction::DrawFullCircle => ic::icon_dd_circle(tid),
        ActiveBarAction::DrawEllipticalArc => ic::icon_dd_arc(tid),
        ActiveBarAction::DrawEllipse => ic::icon_dd_ellipse(tid),
        ActiveBarAction::DrawLine => ic::icon_dd_line(tid),
        ActiveBarAction::DrawRectangle => ic::icon_dd_rect(tid),
        ActiveBarAction::DrawRoundRectangle => ic::icon_dd_round_rect(tid),
        ActiveBarAction::DrawPolygon => ic::icon_dd_polygon(tid),
        ActiveBarAction::DrawBezier => ic::icon_dd_bezier(tid),
        ActiveBarAction::PlaceGraphic => ic::icon_dd_graphic(tid),
        // Fallback — use the generic select icon
        _ => ic::icon_select(tid),
    }
}

// ─── View: Active Bar ────────────────────────────────────────

/// Render the Active Bar (the floating toolbar strip).
pub fn view_bar<'a>(
    current_tool: crate::app::Tool,
    draw_mode: crate::app::DrawMode,
    last_tool: &std::collections::HashMap<String, ActiveBarAction>,
    tokens: &'a ThemeTokens,
    tid: ThemeId,
    has_selection: bool,
    has_net_colors: bool,
) -> Element<'a, ActiveBarMsg> {
    use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};

    // Publish gating context so cells like Move grey their left-press
    // when nothing is selected. Right-clicks still open dropdowns.
    let _selection_guard = HasSelectionGuard::enter(has_selection, has_net_colors);

    // Helper: get last-used action for a group, or use default.
    let last = |group: &str, default: ActiveBarAction| -> ActiveBarMsg {
        ActiveBarMsg::Action(last_tool.get(group).cloned().unwrap_or(default))
    };
    let last_icon = |group: &str, default_icon: svg::Handle| -> svg::Handle {
        last_tool
            .get(group)
            .map(|a| action_icon(a, tid))
            .unwrap_or(default_icon)
    };

    // Helper — build a button item with the schematic editor's
    // standard pattern: left-click action + right-click dropdown +
    // chevron indicator. Selection-aware enable inferred from the
    // left action through `action_enabled`. The chevron uses the
    // themed `chevron_45.svg` so its colour follows the active
    // theme's accent and reads as a proper Altium-style triangle
    // rather than a Unicode glyph.
    let btn = |icon: svg::Handle,
               selected: bool,
               left: ActiveBarMsg,
               right: Option<ActiveBarMsg>,
               tooltip: &str|
     -> ActiveBarItem<ActiveBarMsg> {
        let enabled = match &left {
            ActiveBarMsg::Action(a) => action_enabled(a),
            _ => true,
        };
        let dropdown_indicator = if right.is_some() {
            Some(ActiveBarIcon::Svg(ic::icon_chevron_45(tid)))
        } else {
            None
        };
        ActiveBarItem::Button(ActiveBarButton {
            icon: ActiveBarIcon::Svg(icon),
            tooltip: tooltip.to_string(),
            enabled,
            selected,
            on_press: Some(left),
            on_right_press: right.clone(),
            dropdown_indicator,
        })
    };

    let mut items: Vec<ActiveBarItem<ActiveBarMsg>> = Vec::with_capacity(20);

    items.push(btn(
        ic::icon_filter(tid),
        false,
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Filter),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Filter)),
        "Selection Filter",
    ));
    items.push(btn(
        ic::icon_move(tid),
        false,
        ActiveBarMsg::Action(ActiveBarAction::MoveSelection),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Select)),
        "Move / Transform",
    ));
    items.push(ActiveBarItem::Separator);

    items.push(btn(
        ic::icon_select(tid),
        current_tool == crate::app::Tool::Select,
        ActiveBarMsg::Action(ActiveBarAction::ToolSelect),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::SelectMode)),
        "Select",
    ));
    items.push(btn(
        ic::icon_align(tid),
        false,
        ActiveBarMsg::Action(ActiveBarAction::AlignToGrid),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align)),
        "Align",
    ));
    items.push(ActiveBarItem::Separator);

    items.push(btn(
        last_icon("wiring", ic::icon_wire(tid)),
        current_tool == crate::app::Tool::Wire || current_tool == crate::app::Tool::Bus,
        last("wiring", ActiveBarAction::DrawWire),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Wiring)),
        "Wiring",
    ));
    items.push(btn(
        last_icon("power", ic::icon_power(tid)),
        false,
        last("power", ActiveBarAction::PlacePowerGND),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Power)),
        "Power Port",
    ));
    items.push(ActiveBarItem::Separator);

    items.push(btn(
        last_icon("harness", ic::icon_harness(tid)),
        false,
        last("harness", ActiveBarAction::PlaceSignalHarness),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Harness)),
        "Harness",
    ));
    items.push(btn(
        last_icon("sheet", ic::icon_sheetsym(tid)),
        false,
        last("sheet", ActiveBarAction::PlaceSheetSymbol),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::SheetSymbol)),
        "Sheet Symbol",
    ));
    items.push(btn(
        last_icon("port", ic::icon_port(tid)),
        false,
        last("port", ActiveBarAction::PlacePort),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Port)),
        "Port / Connector",
    ));
    items.push(btn(
        last_icon("directives", ic::icon_directives(tid)),
        false,
        last("directives", ActiveBarAction::PlaceParameterSet),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Directives)),
        "Directives",
    ));
    items.push(ActiveBarItem::Separator);

    items.push(btn(
        last_icon("text", ic::icon_text(tid)),
        current_tool == crate::app::Tool::Text,
        last("text", ActiveBarAction::PlaceTextString),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::TextTools)),
        "Text",
    ));
    items.push(btn(
        last_icon("shapes", ic::icon_shapes(tid)),
        matches!(
            current_tool,
            crate::app::Tool::Line | crate::app::Tool::Rectangle | crate::app::Tool::Circle
        ),
        last("shapes", ActiveBarAction::DrawLine),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::Shapes)),
        "Drawing Tools",
    ));
    items.push(btn(
        ic::icon_netcolor(tid),
        false,
        ActiveBarMsg::Action(ActiveBarAction::ToolSelect),
        Some(ActiveBarMsg::ToggleMenu(ActiveBarMenu::NetColor)),
        "Net Color",
    ));

    // Draw-mode indicator (only visible while wire/bus is the active
    // tool). Drops in as a Custom variant so the standard button
    // styling doesn't apply.
    if matches!(current_tool, crate::app::Tool::Wire | crate::app::Tool::Bus) {
        items.push(ActiveBarItem::Separator);
        let mode_label = match draw_mode {
            crate::app::DrawMode::Ortho90 => "90\u{00B0}",
            crate::app::DrawMode::Angle45 => "45\u{00B0}",
            crate::app::DrawMode::FreeAngle => "Any",
        };
        let pill: Element<'static, ActiveBarMsg> =
            button(text(mode_label.to_string()).size(12).color(Color::WHITE))
                .padding([5, 7])
                .on_press(ActiveBarMsg::Action(ActiveBarAction::DrawWire))
                .style(|_: &Theme, _| button::Style {
                    background: Some(Background::Color(Color::from_rgb(0.22, 0.23, 0.30))),
                    border: Border {
                        width: 0.0,
                        radius: 3.0.into(),
                        color: Color::TRANSPARENT,
                    },
                    ..button::Style::default()
                })
                .into();
        items.push(ActiveBarItem::Custom(pill));
    }

    signex_widgets::active_bar::view(items, tokens)
}

// ─── View: Dropdown menus ────────────────────────────────────

/// Render the dropdown menu for the given Active Bar button.
pub fn view_dropdown(
    menu: ActiveBarMenu,
    tokens: &ThemeTokens,
    filters: &std::collections::HashSet<SelectionFilter>,
    custom_presets: &[CustomFilterPreset],
    tid: ThemeId,
    has_selection: bool,
    has_net_colors: bool,
) -> Element<'static, ActiveBarMsg> {
    // Publish gating context to `dd_item_icon` for the duration of
    // this render: transform / align / distribute rows grey out when
    // nothing is selected; Clear / Clear-All Net Color rows grey out
    // when no net carries a custom colour. Altium parity.
    let _selection_guard = HasSelectionGuard::enter(has_selection, has_net_colors);
    // Snapshot the slice into an owned Vec for the Filter arm, since
    // each preset button's closure must own its label/index for the
    // returned `Element<'static, _>`.
    let custom_presets_owned: Vec<CustomFilterPreset> = custom_presets.to_vec();
    let ac = AbColors::from_tokens(tokens);
    let items: Vec<Element<'_, ActiveBarMsg>> = match menu {
        ActiveBarMenu::Filter => {
            // Altium-style tag buttons for selection filter
            let text_primary = ac.text;
            let hover_c = ac.hover;
            // Border colour matches the Properties-panel unit boxes
            // (`seg_btn` uses `tokens.accent`); near-square corners give
            // the chips a more "input-like" look than the old pill.
            let chip_border = styles::ti(tokens.accent);
            let chip_radius = 2.0_f32;
            let all_on = filters.len() == SelectionFilter::ALL.len();
            let tag = |filter: SelectionFilter, enabled: bool| -> Element<'static, ActiveBarMsg> {
                let label = filter.label();
                let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
                let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
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
                            radius: chip_radius.into(),
                            color: chip_border,
                        },
                        text_color: if enabled { text_on } else { text_off },
                        ..button::Style::default()
                    }
                })
                .into()
            };
            let all_label = if all_on { "All - On" } else { "All - Off" };
            // All-On/Off as a real toggle button (matches chip styling).
            let all_active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
            let all_inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
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
                        radius: chip_radius.into(),
                        color: chip_border,
                    },
                    text_color: if all_on { text_primary } else { all_text_off },
                    ..button::Style::default()
                }
            });
            // Row 1 = All toggle followed by user-defined preset
            // shortcuts (clicking one replaces the active filter set).
            let mut top_row = iced::widget::Row::new()
                .spacing(4)
                .align_y(iced::Alignment::Center)
                .push(all_toggle);
            for (idx, preset) in custom_presets_owned.iter().enumerate() {
                let label = if preset.name.trim().is_empty() {
                    format!("Filter {}", idx + 1)
                } else {
                    preset.name.clone()
                };
                let preset_btn = button(text(label).size(11).color(text_primary))
                    .padding([4, 10])
                    .on_press(ActiveBarMsg::ApplyCustomFilter(idx))
                    .style(move |_: &Theme, status: button::Status| {
                        let bg = match status {
                            button::Status::Hovered => Background::Color(hover_c),
                            _ => Background::Color(Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0)),
                        };
                        button::Style {
                            background: Some(bg),
                            border: Border {
                                width: 1.0,
                                radius: chip_radius.into(),
                                color: chip_border,
                            },
                            text_color: text_primary,
                            ..button::Style::default()
                        }
                    });
                top_row = top_row.push(preset_btn);
            }
            // 3-row layout: row 1 = All toggle + presets. Rows 2 & 3 = 6+6 chips.
            let filter_content: Element<'static, ActiveBarMsg> = column![
                container(top_row).padding([4, 8]),
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
            dd_item_svg(
                ic::icon_dd_select_lasso(tid),
                "Lasso Select",
                ActiveBarAction::LassoSelect,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_select_inside(tid),
                "Inside Area",
                ActiveBarAction::InsideArea,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_select_outside(tid),
                "Outside Area",
                ActiveBarAction::OutsideArea,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_select_touching_rect(tid),
                "Touching Rectangle",
                ActiveBarAction::TouchingRectangle,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_select_touching_line(tid),
                "Touching Line",
                ActiveBarAction::TouchingLine,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_select_all(tid),
                "All",
                ActiveBarAction::SelectAll,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_select_connection(tid),
                "Connection",
                ActiveBarAction::SelectConnection,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                ic::icon_dd_select_toggle(tid),
                "Toggle Selection",
                ActiveBarAction::ToggleSelection,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Select => vec![
            dd_item_svg(
                ic::icon_dd_drag(tid),
                "Drag",
                ActiveBarAction::Drag,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_move(tid),
                "Move",
                ActiveBarAction::MoveSelection,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_move_sel(tid),
                "Move Selection",
                ActiveBarAction::MoveSelection,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_move_xy(tid),
                "Move Selection by X, Y...",
                ActiveBarAction::MoveSelectionXY,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_drag_sel(tid),
                "Drag Selection",
                ActiveBarAction::DragSelection,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_move_to_front(tid),
                "Move To Front",
                ActiveBarAction::MoveToFront,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_rotate(tid),
                "Rotate Selection",
                ActiveBarAction::RotateSelection,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_rotate_cw(tid),
                "Rotate Selection Clockwise",
                ActiveBarAction::RotateSelectionCW,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                ic::icon_dd_bring_front(tid),
                "Bring To Front",
                ActiveBarAction::BringToFront,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_send_back(tid),
                "Send To Back",
                ActiveBarAction::SendToBack,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_bring_front_of(tid),
                "Bring To Front Of",
                ActiveBarAction::BringToFrontOf,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_send_back_of(tid),
                "Send To Back Of",
                ActiveBarAction::SendToBackOf,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                ic::icon_dd_flip_x(tid),
                "Flip Selected Sheet Symbols Along X",
                ActiveBarAction::FlipSelectedX,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_flip_y(tid),
                "Flip Selected Sheet Symbols Along Y",
                ActiveBarAction::FlipSelectedY,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Align => vec![
            dd_item_svg(
                ic::icon_dd_align_left(tid),
                "Align Left",
                ActiveBarAction::AlignLeft,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_align_right(tid),
                "Align Right",
                ActiveBarAction::AlignRight,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_align_hcenter(tid),
                "Align Horizontal Centers",
                ActiveBarAction::AlignHorizontalCenters,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_dist_horiz(tid),
                "Distribute Horizontally",
                ActiveBarAction::DistributeHorizontally,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                ic::icon_dd_align_top(tid),
                "Align Top",
                ActiveBarAction::AlignTop,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_align_bottom(tid),
                "Align Bottom",
                ActiveBarAction::AlignBottom,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_align_vcenter(tid),
                "Align Vertical Centers",
                ActiveBarAction::AlignVerticalCenters,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_dist_vert(tid),
                "Distribute Vertically",
                ActiveBarAction::DistributeVertically,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                ic::icon_dd_align_grid(tid),
                "Align To Grid",
                ActiveBarAction::AlignToGrid,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Wiring => vec![
            dd_item_svg(
                ic::icon_dd_wire(tid),
                "Wire",
                ActiveBarAction::DrawWire,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_bus(tid),
                "Bus",
                ActiveBarAction::DrawBus,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_bus_entry(tid),
                "Bus Entry",
                ActiveBarAction::PlaceBusEntry,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_net_label(tid),
                "Net Label",
                ActiveBarAction::PlaceNetLabel,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Power => vec![
            dd_item_svg(
                ic::icon_dd_gnd(tid),
                "Place GND power port",
                ActiveBarAction::PlacePowerGND,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_vcc(tid),
                "Place VCC power port",
                ActiveBarAction::PlacePowerVCC,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_pwr_plus12(tid),
                "Place +12 power port",
                ActiveBarAction::PlacePowerPlus12,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_pwr_plus5(tid),
                "Place +5 power port",
                ActiveBarAction::PlacePowerPlus5,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_pwr_minus5(tid),
                "Place -5 power port",
                ActiveBarAction::PlacePowerMinus5,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                ic::icon_dd_pwr_arrow(tid),
                "Place Arrow style power port",
                ActiveBarAction::PlacePowerArrow,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_pwr_wave(tid),
                "Place Wave style power port",
                ActiveBarAction::PlacePowerWave,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_pwr_bar(tid),
                "Place Bar style power port",
                ActiveBarAction::PlacePowerBar,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_pwr_circle(tid),
                "Place Circle style power port",
                ActiveBarAction::PlacePowerCircle,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                ic::icon_dd_pwr_signal_gnd(tid),
                "Place Signal Ground power port",
                ActiveBarAction::PlacePowerSignalGND,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_pwr_earth(tid),
                "Place Earth power port",
                ActiveBarAction::PlacePowerEarth,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Harness => vec![
            dd_item_svg(
                ic::icon_dd_harness(tid),
                "Signal Harness",
                ActiveBarAction::PlaceSignalHarness,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_harness_conn(tid),
                "Harness Connector",
                ActiveBarAction::PlaceHarnessConnector,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_harness_entry(tid),
                "Harness Entry",
                ActiveBarAction::PlaceHarnessEntry,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::SheetSymbol => vec![
            dd_item_svg(
                ic::icon_dd_sheet_symbol(tid),
                "Sheet Symbol",
                ActiveBarAction::PlaceSheetSymbol,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_sheet_entry(tid),
                "Sheet Entry",
                ActiveBarAction::PlaceSheetEntry,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_device_sheet(tid),
                "Device Sheet Symbol",
                ActiveBarAction::PlaceDeviceSheetSymbol,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_reuse_block(tid),
                "Reuse Block...",
                ActiveBarAction::PlaceReuseBlock,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Port => vec![
            dd_item_svg(
                ic::icon_dd_port(tid),
                "Port",
                ActiveBarAction::PlacePort,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_off_sheet(tid),
                "Off Sheet Connector",
                ActiveBarAction::PlaceOffSheetConnector,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Directives => vec![
            dd_item_svg(
                ic::icon_dd_param_set(tid),
                "Parameter Set",
                ActiveBarAction::PlaceParameterSet,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_no_erc(tid),
                "Generic No ERC",
                ActiveBarAction::PlaceNoERC,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_diff_pair(tid),
                "Differential Pair",
                ActiveBarAction::PlaceDiffPair,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_blanket(tid),
                "Blanket",
                ActiveBarAction::PlaceBlanket,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_blanket(tid),
                "Compile Mask",
                ActiveBarAction::PlaceCompileMask,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::TextTools => vec![
            dd_item_svg(
                ic::icon_dd_text_string(tid),
                "Text String",
                ActiveBarAction::PlaceTextString,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_text_frame(tid),
                "Text Frame",
                ActiveBarAction::PlaceTextFrame,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_note(tid),
                "Note",
                ActiveBarAction::PlaceNote,
                ac.text,
                ac.hover,
            ),
        ],
        ActiveBarMenu::Shapes => vec![
            dd_item_svg(
                ic::icon_dd_arc(tid),
                "Arc",
                ActiveBarAction::DrawArc,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_circle(tid),
                "Full Circle",
                ActiveBarAction::DrawFullCircle,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_arc(tid),
                "Elliptical Arc",
                ActiveBarAction::DrawEllipticalArc,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_ellipse(tid),
                "Ellipse",
                ActiveBarAction::DrawEllipse,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                ic::icon_dd_line(tid),
                "Line",
                ActiveBarAction::DrawLine,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_rect(tid),
                "Rectangle",
                ActiveBarAction::DrawRectangle,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_round_rect(tid),
                "Round Rectangle",
                ActiveBarAction::DrawRoundRectangle,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_polygon(tid),
                "Polygon",
                ActiveBarAction::DrawPolygon,
                ac.text,
                ac.hover,
            ),
            dd_item_svg(
                ic::icon_dd_bezier(tid),
                "Bezier",
                ActiveBarAction::DrawBezier,
                ac.text,
                ac.hover,
            ),
            dd_sep(ac.sep),
            dd_item_svg(
                ic::icon_dd_graphic(tid),
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
                // The 14×14 swatch sits inside a 20×20 slot so the
                // label column lines up with the SVG-icon rows below
                // (`dd_item_svg` uses a 20-px icon column).
                let swatch =
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
                        });
                let swatch_slot = container(swatch)
                    .width(20)
                    .height(20)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center);
                button(
                    row![
                        swatch_slot,
                        text(label.to_string())
                            .size(13)
                            .color(ac.text)
                            .wrapping(iced::widget::text::Wrapping::None),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                )
                .width(iced::Length::Fill)
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
                dd_item_svg(
                    ic::icon_dd_net_color_custom(tid),
                    "Custom Color...",
                    ActiveBarAction::NetColorCustom,
                    ac.text,
                    ac.hover,
                ),
                dd_sep(ac.sep),
                dd_item_svg(
                    ic::icon_dd_net_color_clear(tid),
                    "Clear Net Color",
                    ActiveBarAction::ClearNetColor,
                    ac.text,
                    ac.hover,
                ),
                dd_item_svg(
                    ic::icon_dd_net_color_clear_all(tid),
                    "Clear All Net Colors",
                    ActiveBarAction::ClearAllNetColors,
                    ac.text,
                    ac.hover,
                ),
            ]
        }
    };

    // For ordinary list-style dropdowns, pin the column to a per-menu
    // `Length::Fixed(W)` so each row's `Length::Fill` button paints a
    // full-row hover highlight without `Fill` propagating to the
    // viewport. `Filter` keeps its auto-sized chip wrap layout.
    let body: Element<'_, ActiveBarMsg> = if let Some(w) = dropdown_min_width(menu) {
        container(column(items).spacing(0))
            .width(iced::Length::Fixed(w))
            .into()
    } else {
        column(items).spacing(0).into()
    };
    container(body)
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

/// Pinned column width per dropdown menu.
///
/// `view_dropdown` wraps the items column in a `Length::Fixed(W)`
/// container using this value. That bound lets each item set
/// `button.width(Length::Fill)` (so the hover background covers the
/// full row) without `Fill` propagating to the viewport — which is the
/// `Length::Fill`-inside-`Length::Shrink` trap iced 0.14 falls into.
///
/// Widths are sized to the longest label in each menu (Roboto @ 13 +
/// 28 px icon column + 24 px button padding + a small safety margin).
/// `Filter` returns `None` because its chip wrap layout already drives
/// its own width.
fn dropdown_min_width(menu: ActiveBarMenu) -> Option<f32> {
    // Width formula: ~6.5 px/char × longest_label + 60 px overhead
    // (24 px button padding + 20 px icon column + 8 px spacing +
    //  small safety). Roboto @ 13 px is narrower than the 8 px/char
    // estimate I used originally — tightening removes the right-side
    // dead space the user noticed.
    Some(match menu {
        ActiveBarMenu::Filter => return None,
        // "Flip Selected Sheet Symbols Along X" (36 chars)
        ActiveBarMenu::Select => 300.0,
        // "Touching Rectangle" (18)
        ActiveBarMenu::SelectMode => 180.0,
        // "Align Horizontal Centers" (24)
        ActiveBarMenu::Align => 220.0,
        // "Net Label" (9) — keep a usable minimum.
        ActiveBarMenu::Wiring => 140.0,
        // "Place Signal Ground power port" (30)
        ActiveBarMenu::Power => 260.0,
        // "Harness Connector" (17)
        ActiveBarMenu::Harness => 180.0,
        // "Device Sheet Symbol" (19)
        ActiveBarMenu::SheetSymbol => 190.0,
        // "Off Sheet Connector" (19)
        ActiveBarMenu::Port => 190.0,
        // "Differential Pair" (17)
        ActiveBarMenu::Directives => 180.0,
        // "Text String" (11)
        ActiveBarMenu::TextTools => 140.0,
        // "Round Rectangle" (15)
        ActiveBarMenu::Shapes => 170.0,
        // "Clear All Net Colors" (20)
        ActiveBarMenu::NetColor => 200.0,
    })
}

/// Horizontal offset (in px) to align dropdown below a given button index.
pub fn dropdown_x_offset(menu: ActiveBarMenu) -> f32 {
    // Bar layout (`view_bar`): each `ab_icon_btn` is a 26 px container,
    // separators are `width(1)`, the row uses `.spacing(2)`, the bar
    // container uses `.padding([2, 2])`. So advancing past one button
    // costs 26 + 2 = 28 px and advancing past one separator costs
    // 1 + 2 = 3 px.
    // Layout: [Filter][Move] | [Select][Align] | [Wire][Power] | [Harness][Sheet][Port][Dir] | [Text][Shapes][NetColor]
    //  btn:     0      1    s    2      3     s   4      5    s    6      7     8    9    s  10     11     12
    let btn = 28.0_f32;
    let s = 3.0_f32;
    let pad = 2.0_f32;
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

/// Active Bar button: left-click activates tool, right-click opens dropdown.
/// Shows a small 45° chevron at bottom-right if button has a dropdown.
/// Legacy bespoke button builder — superseded by
/// `signex_widgets::active_bar::ActiveBarButton`. Kept here so a
/// follow-up patch can lift the chevron / mouse_area details if
/// the generic widget needs them; remove when the migration is
/// fully bedded in.
#[allow(dead_code)]
fn ab_icon_btn(
    icon: svg::Handle,
    active: bool,
    left_click: ActiveBarMsg,
    right_click: Option<ActiveBarMsg>,
    tooltip_text: &'static str,
    tid: ThemeId,
) -> Element<'static, ActiveBarMsg> {
    let handle = icon;
    let has_dropdown = right_click.is_some();
    // Pre-compute the gating decision so both the icon tint and the
    // `on_press` wiring see the same answer.
    let left_enabled = match &left_click {
        ActiveBarMsg::Action(action) => action_enabled(action),
        _ => true,
    };
    let icon_widget = {
        let s = svg(handle).width(20).height(20);
        if left_enabled {
            s
        } else {
            s.style(|_: &Theme, _| iced::widget::svg::Style {
                color: Some(DISABLED_TEXT),
            })
        }
    };

    // Icon with optional chevron indicator
    let icon_content: Element<'static, ActiveBarMsg> = if has_dropdown {
        let chevron = svg(ic::icon_chevron_45(tid)).width(14).height(14);
        iced::widget::Stack::new()
            .push(
                container(icon_widget)
                    .width(26)
                    .height(26)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .push(
                container(chevron)
                    .width(26)
                    .height(26)
                    .align_x(iced::alignment::Horizontal::Right)
                    .align_y(iced::alignment::Vertical::Bottom),
            )
            .into()
    } else {
        container(icon_widget)
            .width(26)
            .height(26)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    };

    // Use a button for left-click (reliable event delivery) and wrap
    // with mouse_area for right-click (dropdown toggle). When the
    // left-click is `Action(a)` for a selection-dependent action and
    // the canvas selection is empty, skip `on_press` so iced renders
    // the button in its `Disabled` state. The right-click (dropdown
    // toggle) still works via the surrounding `mouse_area`, so the
    // user can open the menu and discover what's greyed out.
    let left_msg = left_click;
    let mut btn =
        button(icon_content)
            .padding(0)
            .style(move |_: &Theme, status: button::Status| {
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
            });
    if left_enabled {
        btn = btn.on_press(left_msg);
    }

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

/// Legacy separator builder — superseded by
/// `ActiveBarItem::Separator`. See `ab_icon_btn` for the rationale
/// to keep this around for one more cycle.
#[allow(dead_code)]
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

/// Dropdown item with a themed icon handle.
fn dd_item_svg(
    icon: svg::Handle,
    label: &str,
    action: ActiveBarAction,
    text_c: Color,
    hover_c: Color,
) -> Element<'static, ActiveBarMsg> {
    dd_item_icon(Some(icon), label, text_c, hover_c, action)
}

fn dd_item_icon(
    icon: Option<svg::Handle>,
    label: &str,
    text_c: Color,
    hover_c: Color,
    action: ActiveBarAction,
) -> Element<'static, ActiveBarMsg> {
    // Consult the per-render selection guard so transform / align /
    // distribute rows grey out when the canvas selection is empty.
    let enabled = action_enabled(&action);
    let label_c = if enabled { text_c } else { DISABLED_TEXT };

    let mut r = iced::widget::Row::new()
        .spacing(8)
        .align_y(iced::Alignment::Center);

    // Altium renders dropdown icons at the same size as the Active Bar
    // cell icons (20×20) so group-default and dropdown-member icons look
    // consistent as the pointer crosses from bar to dropdown. Disabled
    // rows flat-tint the SVG to the muted text colour so the whole row
    // (icon + label) reads as inactive.
    if let Some(handle) = icon {
        let mut s = svg(handle).width(20).height(20);
        if !enabled {
            s = s.style(|_: &Theme, _| iced::widget::svg::Style {
                color: Some(DISABLED_TEXT),
            });
        }
        r = r.push(s);
    } else {
        r = r.push(Space::new().width(20).height(20));
    }

    r = r.push(
        text(label.to_string())
            .size(13)
            .color(label_c)
            .wrapping(iced::widget::text::Wrapping::None),
    );

    // `Length::Fill` here is bounded by the dropdown column's
    // `Length::Fixed(W)` wrapper (see `view_dropdown`), so the hover
    // background paints the full row width and Fill never propagates
    // to the viewport. Disabled rows skip `on_press` — iced's button
    // then auto-renders the `Status::Disabled` variant.
    let mut btn = button(r)
        .width(iced::Length::Fill)
        .padding([5, 12])
        .style(dd_btn_style_f(label_c, hover_c));
    if enabled {
        btn = btn.on_press(ActiveBarMsg::Action(action));
    }
    btn.into()
}

fn dd_sep(sep_c: Color) -> Element<'static, ActiveBarMsg> {
    // Width is bounded by `view_dropdown`'s `Length::Fixed(W)` wrapper
    // (per-menu, see `dropdown_min_width`), so `Length::Fill` resolves
    // to the column's pinned width without leaking to the viewport.
    container(Space::new().width(iced::Length::Fill))
        .height(1)
        .padding(iced::Padding {
            top: 3.0,
            right: 0.0,
            bottom: 3.0,
            left: 0.0,
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
