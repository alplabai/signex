//! The board menu's pin swap and design reuse group (#2410): Swap Pins and
//! Swap Gates, live with two pads of one unlocked component selected; Save
//! as Reuse Block, live with board objects selected; and one Place row per
//! block of the project, live with one unlocked component selected (the
//! anchor). With no block saved the Place row is greyed. A Place row is a
//! context action, so the menu closes and an undocked window's row acts on
//! the board that window shows.

use super::items::{dd_cmd_disabled, dd_command, dd_disabled, dd_kb};
use super::menus::CanvasShortcuts;
use crate::app::handlers::pcb_placement::PcbTargets;
use crate::app::{ContextAction, Message, ReuseBlockMsg};
use signex_i18n::{I18n, ids, t};
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// The group's rows for `board`'s selection and the project's `blocks`.
pub(super) fn part_rows(
    i18n: &I18n,
    board: PcbTargets,
    sc: &CanvasShortcuts,
    blocks: &[String],
) -> Vec<DropdownEntry<Message>> {
    let command = |command_id: &'static str, enabled: bool| {
        if enabled {
            dd_command(i18n, None, command_id, sc.of(command_id), None)
        } else {
            dd_cmd_disabled(i18n, None, command_id, sc.of(command_id))
        }
    };
    let mut rows = vec![
        command("swap_pins", board.pad_pair),
        command("swap_gates", board.pad_pair),
        command("save_board_reuse_block", board.copyable),
    ];
    if blocks.is_empty() {
        rows.push(dd_cmd_disabled(i18n, None, "place_board_reuse_block", ""));
    }
    for name in blocks {
        let label = t!(i18n, ids::PCB_REUSE_BLOCK_PLACE_ROW, "name" => name.as_str()).into_string();
        rows.push(if board.one_footprint {
            let place = ReuseBlockMsg::PlaceRequested { name: name.clone() };
            dd_kb(None, &label, "", ContextAction::ReuseBlock(place))
        } else {
            dd_disabled(None, &label, None)
        });
    }
    rows
}
