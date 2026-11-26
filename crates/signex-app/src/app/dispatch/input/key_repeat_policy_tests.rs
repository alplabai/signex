//! The held-key rule against the live catalog.

use super::*;
use crate::keymap::all_command_ids;

const NO_RUN: HeldKeyContext = HeldKeyContext { run_open: false };
const RUN: HeldKeyContext = HeldKeyContext { run_open: true };

fn id(text: &str) -> AppCommandId {
    AppCommandId::new(text).expect("a command id")
}

fn in_catalog(text: &str) -> bool {
    all_command_ids().any(|catalog_id| catalog_id == text)
}

/// How many catalog ids start with `cycle_` or `toggle_`
/// (`cycle_board_snapping` added by #2879, `toggle_live_erc` by #1288).
const CYCLE_AND_TOGGLE_IDS: usize = 28;

#[test]
fn every_cycle_and_toggle_id_in_the_catalog_runs_once() {
    let mut checked = 0;
    for text in all_command_ids().filter(|t| t.starts_with("cycle_") || t.starts_with("toggle_")) {
        for ctx in [NO_RUN, RUN] {
            assert!(!runs_on_key_repeat(&id(text), ctx), "{text} {ctx:?}");
        }
        checked += 1;
    }
    assert_eq!(checked, CYCLE_AND_TOGGLE_IDS);
}

#[test]
fn the_named_run_once_and_rotate_ids_are_catalog_ids() {
    for text in RUN_ONCE.iter().chain(BENDS_AN_OPEN_RUN) {
        assert!(in_catalog(text), "{text} is not a catalog id");
    }
}

#[test]
fn flips_mirrors_and_add_vertex_run_once_everywhere() {
    for text in RUN_ONCE {
        for ctx in [NO_RUN, RUN] {
            assert!(!runs_on_key_repeat(&id(text), ctx), "{text} {ctx:?}");
        }
    }
}

/// Mockup row 175: a held H steps the lighting once, and a held
/// Ctrl+Shift+C copies one screenshot of the board 3D view.
#[test]
fn the_board_3d_keys_run_once_while_held() {
    for text in [
        "flip_board_3d",
        "cycle_board_3d_lighting",
        "copy_board_3d_screenshot",
    ] {
        assert!(in_catalog(text), "{text}");
        for ctx in [NO_RUN, RUN] {
            assert!(!runs_on_key_repeat(&id(text), ctx), "{text} {ctx:?}");
        }
    }
}

#[test]
fn rotate_repeats_on_a_selection_and_runs_once_while_it_bends_a_run() {
    for text in BENDS_AN_OPEN_RUN {
        assert!(runs_on_key_repeat(&id(text), NO_RUN), "{text}");
        assert!(!runs_on_key_repeat(&id(text), RUN), "{text}");
    }
}

#[test]
fn steps_moves_and_views_keep_repeating() {
    for text in [
        "undo",
        "redo",
        "zoom_in_at_cursor",
        "zoom_out_at_cursor",
        "next_layer",
        "prev_layer",
        "next_sheet",
        "next_document_tab",
        "find_next",
    ] {
        assert!(in_catalog(text), "{text}");
        for ctx in [NO_RUN, RUN] {
            assert!(runs_on_key_repeat(&id(text), ctx), "{text} {ctx:?}");
        }
    }
}
