//! The board menu's pin swap and design reuse rows (#2410): Swap Pins and
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

/// A command row, live while `enabled`.
fn command(
    i18n: &I18n,
    sc: &CanvasShortcuts,
    command_id: &'static str,
    enabled: bool,
) -> DropdownEntry<Message> {
    if enabled {
        dd_command(i18n, None, command_id, sc.of(command_id), None)
    } else {
        dd_cmd_disabled(i18n, None, command_id, sc.of(command_id))
    }
}

/// Swap Pins and Swap Gates: footprint rows, shown with a footprint
/// selected (2026-11-18).
pub(super) fn swap_rows(
    i18n: &I18n,
    board: PcbTargets,
    sc: &CanvasShortcuts,
) -> Vec<DropdownEntry<Message>> {
    vec![
        command(i18n, sc, "swap_pins", board.pad_pair),
        command(i18n, sc, "swap_gates", board.pad_pair),
    ]
}

/// Save as Reuse Block, live with board objects selected.
pub(super) fn save_row(
    i18n: &I18n,
    board: PcbTargets,
    sc: &CanvasShortcuts,
) -> DropdownEntry<Message> {
    command(i18n, sc, "save_board_reuse_block", board.copyable)
}

/// One Place row per block of the project; a greyed Place Reuse Block row
/// with none saved.
pub(super) fn place_rows(
    i18n: &I18n,
    board: PcbTargets,
    blocks: &[String],
) -> Vec<DropdownEntry<Message>> {
    if blocks.is_empty() {
        return vec![dd_cmd_disabled(i18n, None, "place_board_reuse_block", "")];
    }
    blocks
        .iter()
        .map(|name| {
            let label =
                t!(i18n, ids::PCB_REUSE_BLOCK_PLACE_ROW, "name" => name.as_str()).into_string();
            if board.one_footprint {
                let place = ReuseBlockMsg::PlaceRequested { name: name.clone() };
                dd_kb(None, &label, "", ContextAction::ReuseBlock(place))
            } else {
                dd_disabled(None, &label, None)
            }
        })
        .collect()
}
