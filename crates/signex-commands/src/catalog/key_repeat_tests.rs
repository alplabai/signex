//! Held keys do not repeat cycle and toggle commands (Hakan, 2026-11-19).

use super::*;

/// Cycles and toggles whose id does not say so: each steps or flips one
/// setting, so a held key would spin through it.
const IGNORED_BY_MEANING: &[&str] = &[
    // Flips the wire/bus corner.
    "switch_segment_posture",
    // Step the grid size ("Cycle Grid Size").
    "next_grid",
    "previous_grid",
    // Flip the board view between 3D and flat, and below and above.
    "view_board_3d",
    "flip_board_view",
];

/// `cycle_` / `toggle_` ids that still repeat, each with its reason. None
/// today.
const PREFIXED_EXCEPTIONS: &[(&str, &str)] = &[];

fn is_cycle_or_toggle_id(id: &str) -> bool {
    id.starts_with("cycle_") || id.starts_with("toggle_")
}

#[test]
fn every_cycle_and_toggle_id_ignores_key_repeat() {
    let mut checked = 0;
    for meta in all_metadata().filter(|m| is_cycle_or_toggle_id(m.id)) {
        if PREFIXED_EXCEPTIONS.iter().any(|(id, _)| *id == meta.id) {
            continue;
        }
        assert_eq!(meta.key_repeat, KeyRepeat::Ignored, "{}", meta.id);
        checked += 1;
    }
    assert!(checked >= 25, "only {checked} cycle/toggle ids found");
}

#[test]
fn only_cycles_and_toggles_ignore_key_repeat() {
    for meta in all_metadata().filter(|m| m.key_repeat == KeyRepeat::Ignored) {
        assert!(
            is_cycle_or_toggle_id(meta.id) || IGNORED_BY_MEANING.contains(&meta.id),
            "{} ignores key repeat but is not listed as a cycle or toggle",
            meta.id
        );
    }
    for id in IGNORED_BY_MEANING {
        let meta = metadata_for_str(id).unwrap_or_else(|| panic!("{id} is in the catalog"));
        assert_eq!(meta.key_repeat, KeyRepeat::Ignored, "{id}");
    }
}

#[test]
fn a_repeating_command_keeps_the_default() {
    for id in [
        "undo",
        "redo",
        "zoom_in_at_cursor",
        "zoom_out_at_cursor",
        "next_layer",
        "rotate_selection",
    ] {
        let meta = metadata_for_str(id).unwrap_or_else(|| panic!("{id} is in the catalog"));
        assert_eq!(meta.key_repeat, KeyRepeat::Runs, "{id}");
    }
}
