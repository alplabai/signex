//! What a held key's auto-repeat does to the command its stroke runs: run
//! it again, or not. Key repeat is a keyboard matter, so the rule lives in
//! the input adapter, not in the catalog the CLI and agents share. Decision:
//! `docs/DECISIONS.md`, "Held keys".

use crate::keymap::AppCommandId;

/// The window state a held key's rule depends on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct HeldKeyContext {
    /// A wire, bus or board track run is open, so the rotate keys bend it
    /// instead of turning the selection (#2372, #2373).
    pub run_open: bool,
}

/// Run-once commands whose id does not start with `cycle_` or `toggle_`:
/// each steps, flips or adds one thing, so a held key would pile them up.
const RUN_ONCE: &[&str] = &[
    "switch_segment_posture",
    "next_grid",
    "previous_grid",
    "view_board_3d",
    "flip_board_view",
    "flip_selection",
    "flip_selected_x",
    "flip_selected_y",
    "mirror_x",
    "mirror_y",
    // Mid draw it fixes a vertex at the cursor, else it arms a copy.
    "copy_attributes_or_add_vertex",
];

/// The rotate commands: they bend an open run, and run once while they do.
const BENDS_AN_OPEN_RUN: &[&str] = &[
    "rotate_clockwise",
    "rotate_counterclockwise",
    "rotate_selection",
    "rotate_selection_cw",
];

/// Whether the auto-repeat of a held key runs `command` again in `ctx`.
pub(crate) fn runs_on_key_repeat(command: &AppCommandId, ctx: HeldKeyContext) -> bool {
    let id = command.as_str();
    let is_cycle_or_toggle = id.starts_with("cycle_") || id.starts_with("toggle_");
    if is_cycle_or_toggle || RUN_ONCE.contains(&id) {
        return false;
    }
    !(ctx.run_open && BENDS_AN_OPEN_RUN.contains(&id))
}

#[cfg(test)]
#[path = "key_repeat_tests.rs"]
mod tests;
