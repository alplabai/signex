//! The tool groups of Place on a sheet (#2415): every schematic placement
//! tool the Active Bar has, in submenus grouped the way the bar groups them.
//! A live tool runs its command and is gated by it; a planned one is greyed
//! with the planned tooltip (`items::tool_item`).

use super::MenuEntry;
use super::items::{cmd, submenu, tool_item};
use crate::menu_bar::MenuContext;
use signex_i18n::menu_ids;

const SEP: MenuEntry = MenuEntry::Separator;

/// The submenus under the sheet's top Place rows.
pub(super) fn tool_groups(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        submenu(ctx, menu_ids::MENU_PLACE_PORTS, ports(ctx)),
        submenu(ctx, menu_ids::MENU_PLACE_DIRECTIVES, directives(ctx)),
        submenu(ctx, menu_ids::MENU_PLACE_GRAPHICS, graphics(ctx)),
    ]
}

/// The bar's Port, Sheet Symbol and Harness groups (Sheet Entry stays a top
/// row). Global and hierarchical
/// labels are not repeated here: their commands arm the same Port and Off
/// Sheet Connector tools.
fn ports(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        tool_item(ctx, cmd("place_port", "")),
        tool_item(ctx, cmd("place_off_sheet_connector", "")),
        SEP,
        tool_item(ctx, cmd("place_sheet_symbol", "")),
        tool_item(ctx, cmd("place_device_sheet_symbol", "")),
        tool_item(ctx, cmd("place_reuse_block", "")),
        SEP,
        tool_item(ctx, cmd("place_signal_harness", "")),
        tool_item(ctx, cmd("place_harness_connector", "")),
        tool_item(ctx, cmd("place_harness_entry", "")),
    ]
}

/// The bar's Directives group, in its order.
fn directives(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        tool_item(ctx, cmd("place_parameter_set", "")),
        tool_item(ctx, cmd("place_no_erc", "")),
        tool_item(ctx, cmd("place_diff_pair", "")),
        tool_item(ctx, cmd("place_blanket", "")),
        tool_item(ctx, cmd("place_compile_mask", "")),
    ]
}

/// The bar's Drawing Tools group, in its order.
fn graphics(ctx: &MenuContext) -> Vec<MenuEntry> {
    vec![
        tool_item(ctx, cmd("draw_arc", "")),
        tool_item(ctx, cmd("draw_full_circle", "")),
        tool_item(ctx, cmd("draw_elliptical_arc", "")),
        tool_item(ctx, cmd("draw_ellipse", "")),
        SEP,
        tool_item(ctx, cmd("draw_line", "")),
        tool_item(ctx, cmd("draw_rectangle", "")),
        tool_item(ctx, cmd("draw_round_rectangle", "")),
        tool_item(ctx, cmd("draw_polygon", "")),
        tool_item(ctx, cmd("draw_bezier", "")),
        SEP,
        tool_item(ctx, cmd("place_graphic", "")),
    ]
}
