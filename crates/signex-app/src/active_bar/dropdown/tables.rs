//! Uniform per-menu entry tables for the Active Bar dropdowns.

use iced::widget::svg;
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
    Item {
        icon: fn(ThemeId) -> svg::Handle,
        label: &'static str,
        action: ActiveBarAction,
    },
}

impl EntrySpec {
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

pub(super) const POWER: &[EntrySpec] = &[
    EntrySpec::item(
        ic::icon_dd_gnd,
        "Place GND power port",
        ActiveBarAction::PlacePowerGND,
    ),
    EntrySpec::item(
        ic::icon_dd_vcc,
        "Place VCC power port",
        ActiveBarAction::PlacePowerVCC,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_plus12,
        "Place +12 power port",
        ActiveBarAction::PlacePowerPlus12,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_plus5,
        "Place +5 power port",
        ActiveBarAction::PlacePowerPlus5,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_minus5,
        "Place -5 power port",
        ActiveBarAction::PlacePowerMinus5,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_pwr_arrow,
        "Place Arrow style power port",
        ActiveBarAction::PlacePowerArrow,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_wave,
        "Place Wave style power port",
        ActiveBarAction::PlacePowerWave,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_bar,
        "Place Bar style power port",
        ActiveBarAction::PlacePowerBar,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_circle,
        "Place Circle style power port",
        ActiveBarAction::PlacePowerCircle,
    ),
    EntrySpec::Separator,
    EntrySpec::item(
        ic::icon_dd_pwr_signal_gnd,
        "Place Signal Ground power port",
        ActiveBarAction::PlacePowerSignalGND,
    ),
    EntrySpec::item(
        ic::icon_dd_pwr_earth,
        "Place Earth power port",
        ActiveBarAction::PlacePowerEarth,
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
        "Parameter Set",
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
        "Blanket",
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
pub(super) fn render(
    specs: &'static [EntrySpec],
    tid: ThemeId,
    sel: bool,
    nc: bool,
) -> Vec<DropdownEntry<ActiveBarMsg>> {
    specs
        .iter()
        .map(|spec| match spec {
            EntrySpec::Separator => DropdownEntry::Separator,
            EntrySpec::Item {
                icon,
                label,
                action,
            } => dd_item(icon(tid), label, action.clone(), sel, nc),
        })
        .collect()
}
