//! The board canvas menu (#2240, #2405), by what the right-click is on
//! (2026-11-18), as the sheet menu shows its selection rows only with a
//! selection. On an empty area with nothing selected: Find, the Place
//! flyout, Undo and Redo, Paste, Paste Special and Select All, Board Sketch
//! and the reuse block Place rows, then the View group. On an object or with
//! a selection the object rows follow, each shown only for the kinds it
//! acts on: the footprint rows (Change Footprint, Swap Pins, Swap Gates,
//! Arrange in Box) for a footprint, the route rows (Lock, Unlock, Gloss
//! Selected Routes, Add Teardrops) for a track or via, Select Net and
//! Highlight Net for an object with a net, then Cross Probe, Properties and
//! Compare with Schematic. Each row runs its catalog command
//! and is live exactly when that command is enabled in the menu's window:
//! on a read-only board every editing row is greyed.

use super::*;

use super::board_part_rows::{place_rows, save_row, swap_rows};
use super::items::{
    dd_cmd_disabled, dd_command, dd_disabled, dd_msg, submenu_launcher, tag_command, text,
};
use super::menus::CanvasShortcuts;
use crate::app::command::CommandContext;
use crate::icons as ic;
use crate::keymap::AppCommandId;
use iced::widget::svg::Handle;
use signex_i18n::{I18n, menu_ids};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// The board canvas menu for the facts of the menu's window `ctx` and the
/// project's reuse `blocks`.
pub(in crate::app::view::context_menu) fn pcb_canvas_menu_with_blocks(
    i18n: &I18n,
    tid: ThemeId,
    ctx: &CommandContext,
    active_submenu: Option<ContextSubmenu>,
    sc: &CanvasShortcuts,
    blocks: &[String],
) -> Vec<DropdownEntry<Message>> {
    let board = ctx.pcb;
    let row = |icon: Option<Handle>, command_id: &'static str| {
        let live = AppCommandId::new(command_id).is_ok_and(|id| is_enabled(&id, ctx));
        if live {
            dd_command(i18n, icon, command_id, sc.of(command_id), None)
        } else {
            dd_cmd_disabled(i18n, icon, command_id, sc.of(command_id))
        }
    };
    // A row live only when the selection holds what it acts on, and its
    // command is enabled.
    let gated_row = |command_id: &'static str, live: bool| {
        if live {
            row(None, command_id)
        } else {
            dd_cmd_disabled(i18n, None, command_id, sc.of(command_id))
        }
    };
    let launcher = |kind: ContextSubmenu, icon: Handle, label: String| {
        submenu_launcher(Some(icon), &label, kind, active_submenu == Some(kind))
    };
    let selected = board.any;
    let on_object = selected || board.hovered;
    let footprints = board.footprints > 0;
    let mut v = Vec::with_capacity(40);
    if on_object {
        // The sheet menu seeds Find Similar from what is under the pointer.
        v.push(row(
            Some(ic::icon_dd_find_similar(tid)),
            "find_similar_objects",
        ));
    }
    v.extend([
        row(Some(ic::icon_chrome_search(tid)), "find"),
        DropdownEntry::Separator,
        launcher(
            ContextSubmenu::Place,
            ic::icon_dd_place_menu(tid),
            text(i18n, menu_ids::MENU_PLACE),
        ),
    ]);
    if selected {
        v.push(launcher(
            ContextSubmenu::Align,
            ic::icon_dd_align_menu(tid),
            text(i18n, menu_ids::CTX_ALIGN),
        ));
    }
    v.extend([
        DropdownEntry::Separator,
        row(None, "undo"),
        row(None, "redo"),
        DropdownEntry::Separator,
    ]);
    // The clipboard group (#2361), as on a sheet.
    if selected {
        v.push(row(Some(ic::icon_dd_cut(tid)), "cut"));
        v.push(row(Some(ic::icon_dd_copy(tid)), "copy"));
    }
    v.extend([
        row(Some(ic::icon_dd_paste(tid)), "paste"),
        row(Some(ic::icon_dd_smart_paste(tid)), "smart_paste"),
    ]);
    if selected {
        v.push(row(None, "duplicate"));
    }
    v.push(row(Some(ic::icon_dd_select_all(tid)), "select_all"));
    if selected {
        v.push(DropdownEntry::Separator);
        v.extend(selection_rows(i18n, tid, ctx, sc));
    }
    v.push(DropdownEntry::Separator);
    // Enter or leave the board's sketch mode (#2234), and place a reuse
    // block (#2410).
    v.push(row(Some(ic::icon_dd_rect(tid)), "board_sketch"));
    v.extend(place_rows(i18n, board, blocks));
    if selected {
        v.push(DropdownEntry::Separator);
        // The board Tools edits on the selection (#2408, #2409).
        if board.routes {
            v.push(gated_row("gloss_selected_routes", true));
            v.push(gated_row("add_teardrops", true));
        }
        v.push(gated_row("board_shape_from_selection", true));
        v.push(gated_row("keepout_from_selection", true));
        if footprints {
            v.push(gated_row("arrange_in_box", board.placeable));
        }
    }
    if on_object {
        v.push(DropdownEntry::Separator);
        // The net of the selection or of what is under the pointer (#2403).
        if board.net {
            v.push(row(None, "select_board_net"));
            v.push(row(None, "highlight_board_net"));
        }
        v.push(row(Some(ic::icon_dd_cross_probe(tid)), "cross_probe"));
        // Shows the Properties panel, where one selected footprint's
        // designator and value are edited (#1312).
        v.push(row(
            Some(ic::icon_dd_properties(tid)),
            "edit_selected_object_properties",
        ));
        // Compare with Schematic (#2462); the empty-area menu stays short.
        v.push(compare_with_schematic_row(i18n, ctx));
    }
    v.extend([
        DropdownEntry::Separator,
        // View group (#2223): the View menu's board-view row.
        row(None, "toggle_ratsnest"),
    ]);
    v
}

/// The board menu's Compare with Schematic row (#2462): runs
/// `compare_board_with_schematic` under a shorter name than its menu text,
/// live exactly when the command is.
fn compare_with_schematic_row(i18n: &I18n, ctx: &CommandContext) -> DropdownEntry<Message> {
    const ID: &str = "compare_board_with_schematic";
    let label = text(i18n, menu_ids::CTX_COMPARE_WITH_SCHEMATIC);
    let live = AppCommandId::new(ID).ok().filter(|id| is_enabled(id, ctx));
    let entry = match live {
        Some(id) => dd_msg(
            None,
            &label,
            "",
            Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
                id,
                project: None,
            })),
        ),
        None => dd_disabled(None, &label, None),
    };
    tag_command(entry, ID)
}

/// The rows that edit the selection: move, turn, flip and mirror; the
/// footprint rows; the lock rows for tracks and vias; Save as Reuse Block;
/// Delete.
fn selection_rows(
    i18n: &I18n,
    tid: ThemeId,
    ctx: &CommandContext,
    sc: &CanvasShortcuts,
) -> Vec<DropdownEntry<Message>> {
    let board = ctx.pcb;
    let row = |icon: Option<Handle>, command_id: &'static str| {
        let live = AppCommandId::new(command_id).is_ok_and(|id| is_enabled(&id, ctx));
        if live {
            dd_command(i18n, icon, command_id, sc.of(command_id), None)
        } else {
            dd_cmd_disabled(i18n, icon, command_id, sc.of(command_id))
        }
    };
    let gated_row = |command_id: &'static str, live: bool| {
        if live {
            row(None, command_id)
        } else {
            dd_cmd_disabled(i18n, None, command_id, sc.of(command_id))
        }
    };
    let mut v = vec![
        row(Some(ic::icon_dd_move_sel(tid)), "move_selection"),
        DropdownEntry::Separator,
        row(Some(ic::icon_dd_rotate(tid)), "rotate_counterclockwise"),
        row(Some(ic::icon_dd_rotate_cw(tid)), "rotate_clockwise"),
        row(Some(ic::icon_dd_flip_x(tid)), "flip_selection"),
        row(Some(ic::icon_dd_flip_x(tid)), "mirror_x"),
        row(Some(ic::icon_dd_flip_y(tid)), "mirror_y"),
    ];
    if board.footprints > 0 {
        // Picks a library footprint for the selected components (#2366).
        v.push(row(None, "change_footprint"));
        v.extend(swap_rows(i18n, board, sc));
    }
    if board.routes {
        // Lock or unlock the selected tracks and vias (#2407).
        v.push(gated_row("lock_board_items", board.lockable));
        v.push(gated_row("unlock_board_items", board.unlockable));
    }
    v.extend([
        save_row(i18n, board, sc),
        DropdownEntry::Separator,
        row(Some(ic::icon_dd_delete(tid)), "delete_selection"),
    ]);
    v
}

/// Whether `id` is enabled in `ctx`.
fn is_enabled(id: &AppCommandId, ctx: &CommandContext) -> bool {
    crate::app::command::is_enabled(id, ctx)
}
