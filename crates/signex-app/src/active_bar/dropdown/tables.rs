//! Uniform per-menu entry tables for the Active Bar dropdowns.

use iced::widget::svg;
use signex_i18n::{MessageId, menu_ids};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::DropdownEntry;

use crate::active_bar::{ActiveBarAction, ActiveBarMsg};
use crate::icons as ic;

use super::dd_item;

/// One row inside a uniform per-menu entry table — theme- and
/// context-free, unlike `DropdownEntry`/`DropdownItem`, so the table
/// itself can be a `const` array (icon fn pointers and fieldless enum
/// variants are const-constructible; `ActiveBarAction` is not `Copy`,
/// so `render` clones it per row). Collapses the ~78 near-identical
/// `dd_item(...)` call sites this file used to carry across ~11
/// uniform per-menu builder functions into ~11 data tables + one
/// renderer (#457, epic #278).
pub(super) enum EntrySpec {
    Separator,
    /// A subhead over a group of rows (Power's Common / Styles / Grounds,
    /// the Drawing Tools' Dimension).
    Header(MessageId),
    Item {
        icon: fn(ThemeId) -> svg::Handle,
        label: &'static str,
        action: ActiveBarAction,
    },
    /// A power port row: its own port glyph in the power colour and a
    /// short label; the command's full text is its tooltip.
    Power {
        icon: fn(ThemeId) -> svg::Handle,
        short: MessageId,
        /// The full text, used when the action has no catalog id.
        full: &'static str,
        action: ActiveBarAction,
    },
}

impl EntrySpec {
    /// The action the row runs; `None` for a separator or a subhead.
    pub(super) fn action(&self) -> Option<&ActiveBarAction> {
        match self {
            Self::Item { action, .. } | Self::Power { action, .. } => Some(action),
            Self::Separator | Self::Header(_) => None,
        }
    }

    const fn power(
        icon: fn(ThemeId) -> svg::Handle,
        short: MessageId,
        full: &'static str,
        action: ActiveBarAction,
    ) -> Self {
        Self::Power {
            icon,
            short,
            full,
            action,
        }
    }

    pub(super) const fn item(
        icon: fn(ThemeId) -> svg::Handle,
        label: &'static str,
        action: ActiveBarAction,
    ) -> Self {
        Self::Item {
            icon,
            label,
            action,
        }
    }
}

pub(super) const SELECT_MODE: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_select_lasso,
        "Lasso Select",
        ActiveBarAction::LassoSelect,
    ),
    EntrySpec::item(
        ic::icon_dd_select_inside,
        "Inside Area",
        ActiveBarAction::InsideArea,
    ),
    EntrySpec::item(
        ic::icon_dd_select_outside,
        "Outside Area",
        ActiveBarAction::OutsideArea,
    ),
    EntrySpec::item(
        ic::icon_dd_select_touching_rect,
        "Touching Rectangle",
        ActiveBarAction::TouchingRectangle,
    ),
    EntrySpec::item(
        ic::icon_dd_select_touching_line,
        "Touching Line",
        ActiveBarAction::TouchingLine,
    ),
    EntrySpec::item(ic::icon_dd_select_all, "All", ActiveBarAction::SelectAll),
    EntrySpec::item(
        ic::icon_dd_select_connection,
        "Connection",
        ActiveBarAction::SelectConnection,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_select_toggle,
        "Toggle Selection",
        ActiveBarAction::ToggleSelection,
    ),
];

pub(super) const SELECT: &[EntrySpec] = &[
    EntrySpec::item(ic::icon_dd_drag, "Drag", ActiveBarAction::Drag),
    EntrySpec::item(ic::icon_dd_move, "Move", ActiveBarAction::Move),
    EntrySpec::item(
        ic::icon_dd_move_sel,
        "Move Selection",
        ActiveBarAction::MoveSelection,
    ),
    EntrySpec::item(
        ic::icon_dd_move_xy,
        "Move Selection by X, Y...",
        ActiveBarAction::MoveSelectionXY,
    ),
    EntrySpec::item(
        ic::icon_dd_drag_sel,
        "Drag Selection",
        ActiveBarAction::DragSelection,
    ),
    EntrySpec::item(
        ic::icon_dd_move_to_front,
        "Move To Front",
        ActiveBarAction::MoveToFront,
    ),
    EntrySpec::item(
        ic::icon_dd_rotate,
        "Rotate Selection Counter-Clockwise",
        ActiveBarAction::RotateSelection,
    ),
    EntrySpec::item(
        ic::icon_dd_rotate_cw,
        "Rotate Selection Clockwise",
        ActiveBarAction::RotateSelectionCW,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_bring_front,
        "Bring To Front",
        ActiveBarAction::BringToFront,
    ),
    EntrySpec::item(
        ic::icon_dd_send_back,
        "Send To Back",
        ActiveBarAction::SendToBack,
    ),
    EntrySpec::item(
        ic::icon_dd_bring_front_of,
        "Bring To Front Of",
        ActiveBarAction::BringToFrontOf,
    ),
    EntrySpec::item(
        ic::icon_dd_send_back_of,
        "Send To Back Of",
        ActiveBarAction::SendToBackOf,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_flip_x,
        "Flip Selected Sheet Symbols Along X",
        ActiveBarAction::FlipSelectedX,
    ),
    EntrySpec::item(
        ic::icon_dd_flip_y,
        "Flip Selected Sheet Symbols Along Y",
        ActiveBarAction::FlipSelectedY,
    ),
];

pub(super) const ALIGN: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_align_left,
        "Align Left",
        ActiveBarAction::AlignLeft,
    ),
    EntrySpec::item(
        ic::icon_dd_align_right,
        "Align Right",
        ActiveBarAction::AlignRight,
    ),
    EntrySpec::item(
        ic::icon_dd_align_hcenter,
        "Align Horizontal Centers",
        ActiveBarAction::AlignHorizontalCenters,
    ),
    EntrySpec::item(
        ic::icon_dd_dist_horiz,
        "Distribute Horizontally",
        ActiveBarAction::DistributeHorizontally,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_align_top,
        "Align Top",
        ActiveBarAction::AlignTop,
    ),
    EntrySpec::item(
        ic::icon_dd_align_bottom,
        "Align Bottom",
        ActiveBarAction::AlignBottom,
    ),
    EntrySpec::item(
        ic::icon_dd_align_vcenter,
        "Align Vertical Centers",
        ActiveBarAction::AlignVerticalCenters,
    ),
    EntrySpec::item(
        ic::icon_dd_dist_vert,
        "Distribute Vertically",
        ActiveBarAction::DistributeVertically,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_align_grid,
        "Align To Grid",
        ActiveBarAction::AlignToGrid,
    ),
];

pub(super) const WIRING: &[EntrySpec] = &[
    EntrySpec::item(ic::icon_dd_wire, "Wire", ActiveBarAction::DrawWire),
    EntrySpec::item(ic::icon_dd_bus, "Bus", ActiveBarAction::DrawBus),
    EntrySpec::item(
        ic::icon_dd_bus_entry,
        "Bus Entry",
        ActiveBarAction::PlaceBusEntry,
    ),
    EntrySpec::item(
        ic::icon_dd_net_label,
        "Net Label",
        ActiveBarAction::PlaceNetLabel,
    ),
    EntrySpec::item(
        ic::icon_dd_junction,
        "Junction",
        ActiveBarAction::PlaceJunction,
    ),
];

/// Power ports under the Common / Styles / Grounds subheads, each a short
/// label over its own glyph; the full "Place … power port" text is the
/// row's tooltip.
pub(super) const POWER: &[EntrySpec] = &[
    EntrySpec::Header(menu_ids::CTX_POWER_COMMON),
    EntrySpec::power(
        ic::icon_dd_gnd,
        menu_ids::CTX_POWER_GND,
        "Place GND power port",
        ActiveBarAction::PlacePowerGND,
    ),
    EntrySpec::power(
        ic::icon_dd_vcc,
        menu_ids::CTX_POWER_VCC,
        "Place VCC power port",
        ActiveBarAction::PlacePowerVCC,
    ),
    EntrySpec::power(
        ic::icon_dd_pwr_plus12,
        menu_ids::CTX_POWER_PLUS12,
        "Place +12 power port",
        ActiveBarAction::PlacePowerPlus12,
    ),
    EntrySpec::power(
        ic::icon_dd_pwr_plus5,
        menu_ids::CTX_POWER_PLUS5,
        "Place +5 power port",
        ActiveBarAction::PlacePowerPlus5,
    ),
    EntrySpec::power(
        ic::icon_dd_pwr_minus5,
        menu_ids::CTX_POWER_MINUS5,
        "Place -5 power port",
        ActiveBarAction::PlacePowerMinus5,
    ),
    EntrySpec::Header(menu_ids::CTX_POWER_STYLES),
    EntrySpec::power(
        ic::icon_dd_pwr_arrow,
        menu_ids::CTX_POWER_ARROW,
        "Place Arrow style power port",
        ActiveBarAction::PlacePowerArrow,
    ),
    EntrySpec::power(
        ic::icon_dd_pwr_wave,
        menu_ids::CTX_POWER_WAVE,
        "Place Wave style power port",
        ActiveBarAction::PlacePowerWave,
    ),
    EntrySpec::power(
        ic::icon_dd_pwr_bar,
        menu_ids::CTX_POWER_BAR,
        "Place Bar style power port",
        ActiveBarAction::PlacePowerBar,
    ),
    EntrySpec::power(
        ic::icon_dd_pwr_circle,
        menu_ids::CTX_POWER_CIRCLE,
        "Place Circle style power port",
        ActiveBarAction::PlacePowerCircle,
    ),
    EntrySpec::Header(menu_ids::CTX_POWER_GROUNDS),
    EntrySpec::power(
        ic::icon_dd_pwr_signal_gnd,
        menu_ids::CTX_POWER_SIGNAL_GROUND,
        "Place Signal Ground power port",
        ActiveBarAction::PlacePowerSignalGND,
    ),
    EntrySpec::power(
        ic::icon_dd_pwr_earth,
        menu_ids::CTX_POWER_EARTH,
        "Place Earth power port",
        ActiveBarAction::PlacePowerEarth,
    ),
    EntrySpec::power(
        ic::icon_dd_pwr_chassis,
        menu_ids::CTX_POWER_CHASSIS,
        "Place Chassis power port",
        ActiveBarAction::PlacePowerChassis,
    ),
];

pub(super) const HARNESS: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_harness,
        "Signal Harness",
        ActiveBarAction::PlaceSignalHarness,
    ),
    EntrySpec::item(
        ic::icon_dd_harness_conn,
        "Harness Connector",
        ActiveBarAction::PlaceHarnessConnector,
    ),
    EntrySpec::item(
        ic::icon_dd_harness_entry,
        "Harness Entry",
        ActiveBarAction::PlaceHarnessEntry,
    ),
];

pub(super) const SHEET_SYMBOL: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_sheet_symbol,
        "Sheet Symbol",
        ActiveBarAction::PlaceSheetSymbol,
    ),
    EntrySpec::item(
        ic::icon_dd_sheet_entry,
        "Sheet Entry",
        ActiveBarAction::PlaceSheetEntry,
    ),
    EntrySpec::item(
        ic::icon_dd_device_sheet,
        "Device Sheet Symbol",
        ActiveBarAction::PlaceDeviceSheetSymbol,
    ),
    EntrySpec::item(
        ic::icon_dd_reuse_block,
        "Reuse Block...",
        ActiveBarAction::PlaceReuseBlock,
    ),
];

pub(super) const PORT: &[EntrySpec] = &[
    EntrySpec::item(ic::icon_dd_port, "Port", ActiveBarAction::PlacePort),
    EntrySpec::item(
        ic::icon_dd_off_sheet,
        "Off Sheet Connector",
        ActiveBarAction::PlaceOffSheetConnector,
    ),
];

pub(super) const DIRECTIVES: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_param_set,
        "Net Parameters",
        ActiveBarAction::PlaceParameterSet,
    ),
    EntrySpec::item(
        ic::icon_dd_no_erc,
        "Generic No ERC",
        ActiveBarAction::PlaceNoERC,
    ),
    EntrySpec::item(
        ic::icon_dd_diff_pair,
        "Differential Pair",
        ActiveBarAction::PlaceDiffPair,
    ),
    EntrySpec::item(
        ic::icon_dd_blanket,
        "Net Blanket",
        ActiveBarAction::PlaceBlanket,
    ),
    EntrySpec::item(
        ic::icon_dd_blanket,
        "Compile Mask",
        ActiveBarAction::PlaceCompileMask,
    ),
];

pub(super) const TEXT_TOOLS: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_text_string,
        "Text String",
        ActiveBarAction::PlaceTextString,
    ),
    EntrySpec::item(
        ic::icon_dd_text_frame,
        "Text Frame",
        ActiveBarAction::PlaceTextFrame,
    ),
    EntrySpec::item(ic::icon_dd_note, "Note", ActiveBarAction::PlaceNote),
];

pub(super) const SHAPES: &[EntrySpec] = &[
    EntrySpec::item(ic::icon_dd_arc, "Arc", ActiveBarAction::DrawArc),
    EntrySpec::item(
        ic::icon_dd_circle,
        "Full Circle",
        ActiveBarAction::DrawFullCircle,
    ),
    EntrySpec::item(
        ic::icon_dd_arc,
        "Elliptical Arc",
        ActiveBarAction::DrawEllipticalArc,
    ),
    EntrySpec::item(ic::icon_dd_ellipse, "Ellipse", ActiveBarAction::DrawEllipse),
    EntrySpec::Separator,
    EntrySpec::item(ic::icon_dd_line, "Line", ActiveBarAction::DrawLine),
    EntrySpec::item(
        ic::icon_dd_rect,
        "Rectangle",
        ActiveBarAction::DrawRectangle,
    ),
    EntrySpec::item(
        ic::icon_dd_round_rect,
        "Round Rectangle",
        ActiveBarAction::DrawRoundRectangle,
    ),
    EntrySpec::item(ic::icon_dd_polygon, "Polygon", ActiveBarAction::DrawPolygon),
    EntrySpec::item(ic::icon_dd_bezier, "Bezier", ActiveBarAction::DrawBezier),
    EntrySpec::item(ic::icon_dd_table, "Table", ActiveBarAction::DrawTable),
    EntrySpec::Separator,
    EntrySpec::Header(signex_i18n::menu_ids::MENU_PLACE_DIMENSION),
    EntrySpec::item(
        ic::icon_pcb_dimension,
        "Linear",
        ActiveBarAction::PlaceLinearDimension,
    ),
    EntrySpec::item(
        ic::icon_pcb_dimension,
        "Aligned",
        ActiveBarAction::PlaceAlignedDimension,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_graphic,
        "Graphic...",
        ActiveBarAction::PlaceGraphic,
    ),
];

/// Render a uniform per-menu `EntrySpec` table into `DropdownEntry`
/// rows, resolving each row's icon for the active theme and folding in
/// the same `dd_item` enable/disable gating every row used before the
/// table refactor.
#[cfg(test)]
pub(super) fn render(
    i18n: &signex_i18n::I18n,
    specs: &'static [EntrySpec],
    tid: ThemeId,
    sel: bool,
    nc: bool,
) -> Vec<DropdownEntry<ActiveBarMsg>> {
    render_with_hints(i18n, specs, tid, sel, nc, &|_| String::new())
}

/// [`render`] with each command row's key hint from the active profile
/// (`hint` resolves a command id; empty when the profile binds none).
pub(super) fn render_with_hints(
    i18n: &signex_i18n::I18n,
    specs: &'static [EntrySpec],
    tid: ThemeId,
    sel: bool,
    nc: bool,
    hint: &dyn Fn(&str) -> String,
) -> Vec<DropdownEntry<ActiveBarMsg>> {
    let power_ink = signex_widgets::theme_ext::power_port(&signex_types::theme::canvas_colors(tid));
    specs
        .iter()
        .map(|spec| {
            let row = match spec {
                EntrySpec::Separator => return DropdownEntry::Separator,
                EntrySpec::Header(id) => {
                    return DropdownEntry::Header(i18n.message_plain(*id, &[]));
                }
                EntrySpec::Item {
                    icon,
                    label,
                    action,
                } => dd_item(i18n, icon(tid), label, action.clone(), sel, nc),
                EntrySpec::Power {
                    icon,
                    short,
                    full,
                    action,
                } => {
                    let row = PowerRow {
                        icon: icon(tid),
                        short: *short,
                        full,
                        ink: power_ink,
                    };
                    row.entry(i18n, action, sel, nc)
                }
            };
            with_key_hint(row, spec.action(), hint)
        })
        .collect()
}

/// A power port row as drawn: its glyph in the power colour, a short
/// label, and the full command text as its tooltip.
struct PowerRow {
    icon: svg::Handle,
    short: MessageId,
    full: &'static str,
    ink: iced::Color,
}

impl PowerRow {
    fn entry(
        self,
        i18n: &signex_i18n::I18n,
        action: &ActiveBarAction,
        sel: bool,
        nc: bool,
    ) -> DropdownEntry<ActiveBarMsg> {
        let tooltip = crate::app::command::active_bar::action_label(i18n, action, self.full);
        match dd_item(i18n, self.icon, self.full, action.clone(), sel, nc) {
            DropdownEntry::Item(mut item) => {
                item.label = i18n.message_plain(self.short, &[]);
                DropdownEntry::Item(item.icon_tint(self.ink).tooltip(tooltip))
            }
            other => other,
        }
    }
}

/// `row` with the active profile's key for its action's command, if any.
fn with_key_hint(
    row: DropdownEntry<ActiveBarMsg>,
    action: Option<&ActiveBarAction>,
    hint: &dyn Fn(&str) -> String,
) -> DropdownEntry<ActiveBarMsg> {
    let key = action
        .and_then(crate::app::command::active_bar::id_for_action)
        .map(hint)
        .unwrap_or_default();
    match row {
        DropdownEntry::Item(item) if !key.is_empty() => DropdownEntry::Item(item.shortcut(key)),
        other => other,
    }
}
