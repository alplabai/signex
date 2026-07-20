//! Altium-style Active Bar — floating toolbar centered at top of canvas.
//!
//! 12 icon buttons, each with an optional dropdown menu.
//! Matches Altium Designer's schematic editor Active Bar exactly.

use std::cell::Cell;

use iced::widget::{button, svg, text};
use iced::{Background, Border, Color, Element, Theme};
use signex_types::theme::{ThemeId, ThemeTokens};

use crate::styles;

mod dropdown;
mod legacy;

// Re-exported so `crate::active_bar::view_dropdown` / `dropdown_x_offset`
// paths (overlays/mod.rs) resolve unchanged after the folder split.
pub use dropdown::{dropdown_x_offset, view_dropdown};

thread_local! {
    /// True when the active schematic has at least one selected item.
    /// `view_bar` installs a `HasSelectionGuard` for the duration of one
    /// render so `ab_icon_btn` can grey out selection-dependent actions
    /// (transform, align, distribute) on the bar's group-default buttons
    /// without threading the flag through every call site. The dropdown
    /// no longer needs this: it folds the enable state into each
    /// `DropdownItem` at build time via `dd_action_enabled`.
    static HAS_SELECTION_FOR_VIEW: Cell<bool> = const { Cell::new(true) };
    /// True when the active schematic has at least one net with a
    /// custom colour applied. Same purpose as `HAS_SELECTION_FOR_VIEW`
    /// but for the NetColor "clear" actions.
    static HAS_NET_COLORS_FOR_VIEW: Cell<bool> = const { Cell::new(true) };
}

/// RAII guard that publishes both `has_selection` and `has_net_colors`
/// to helpers for the duration of one render. Resets to `true` (the
/// "no gating, everything enabled" default) on drop so any caller that
/// reaches `ab_icon_btn` outside a `view_bar` invocation still
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

/// A footprint-editor selection-filter preset. Parallel to
/// `CustomFilterPreset` but keyed on `SelectionFilterKind` (footprint
/// categories) instead of the schematic `SelectionFilter`. Persisted to
/// `prefs.json` under `footprint_filter_presets` (Task 6).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FootprintFilterPreset {
    pub name: String,
    pub kinds: Vec<crate::library::editor::footprint::state::selection_filter::SelectionFilterKind>,
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
        // 42 px fits the widest label ("90°" / "45°" / "Any") at size
        // 12 plus the 7 px side padding. Declared rather than measured
        // because `slot_offsets` places every dropdown from these
        // widths — see `ActiveBarItem::Custom`. This pill sits AFTER
        // every dropdown trigger on this bar, so an error here would
        // only shift the bar's centring, not the panels; state it
        // correctly anyway.
        items.push(ActiveBarItem::custom(pill, 42.0));
    }

    signex_widgets::active_bar::view(items, tokens)
}

