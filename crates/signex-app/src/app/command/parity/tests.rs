//! The parity golden and the checks on its rows.

use std::collections::BTreeSet;

use super::super::CommandContext;
use super::annotations::ANNOTATIONS;
use super::forms::{Form, Owner, owner_of};
use super::inputs::{FACTS, Input};
use super::{Basis, HEADER, rows, table_text};
use crate::keymap::HeadlessUse;

const GOLDEN: &str = include_str!("testdata/parity.golden.tsv");
const MISMATCH_NOTE: &str = "The parity table follows the routing code, so it changes whenever a route, \
its owner or what it reads changes; every ADR-0006 slice that moves a route changes it. Read the diff \
as that change, then regenerate with `UPDATE_GOLDEN=1 cargo test -p signex-app golden_parity_table` \
and commit the file with it. Hand annotations live in `parity/annotations.rs`, not in the golden.";

#[test]
fn golden_parity_table_matches_the_routing_code() {
    let got = table_text(&rows());
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let path = format!(
            "{}/src/app/command/parity/testdata/parity.golden.tsv",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::write(&path, &got).expect("write golden");
        return;
    }
    if GOLDEN == got {
        return;
    }
    let want: BTreeSet<&str> = GOLDEN.lines().collect();
    let live: BTreeSet<&str> = got.lines().collect();
    let mut diff = String::new();
    for row in want.difference(&live) {
        diff.push_str(&format!("- {row}\n"));
    }
    for row in live.difference(&want) {
        diff.push_str(&format!("+ {row}\n"));
    }
    panic!("parity.golden.tsv differs (- golden, + live):\n{diff}{MISMATCH_NOTE}");
}

/// Every row names its implicit inputs from the closed vocabulary, or says
/// `none` after a read of its handler or routing, and names its basis; the
/// golden is checked too, so a hand edit of it cannot slip a word in.
#[test]
fn every_row_has_implicit_inputs_from_the_vocabulary() {
    let unreviewed: Vec<String> = rows()
        .iter()
        .filter(|row| !row.has_value())
        .map(|row| format!("{}\t{}", row.id, row.form.name()))
        .collect();
    assert!(
        unreviewed.is_empty(),
        "gate-only rows that found no input; read the handler and annotate it in parity/annotations.rs:\n{}",
        unreviewed.join("\n")
    );
    let words: BTreeSet<&str> = Input::ALL.iter().map(|input| input.word()).collect();
    let bases = [Basis::Handler, Basis::Routing, Basis::Gate].map(Basis::name);
    for line in GOLDEN.lines().skip(1) {
        let cells: Vec<&str> = line.split('\t').collect();
        assert_eq!(cells.len(), 6, "row `{line}` has six cells");
        assert!(bases.contains(&cells[5]), "row `{line}` has no basis");
        let inputs = cells[4];
        assert!(!inputs.is_empty(), "row `{line}` has no implicit inputs");
        // The origin and the settings are port operands, not implicit inputs.
        let ports = [Input::Origin.word(), Input::Settings.word()];
        let implicit = inputs != "none" && inputs.split(',').any(|word| !ports.contains(&word));
        assert!(
            implicit || cells[5] != "gate",
            "row `{line}` names no implicit input from its gate alone"
        );
        if inputs != "none" {
            for word in inputs.split(',') {
                assert!(
                    words.contains(word),
                    "`{word}` in `{line}` is not in the vocabulary"
                );
            }
        }
    }
    assert_eq!(GOLDEN.lines().next(), Some(HEADER));
}

/// A hand annotation names a row that exists, once.
#[test]
fn every_annotation_names_one_row() {
    let rows = rows();
    let mut seen = BTreeSet::new();
    for (ids, form, _) in ANNOTATIONS {
        for id in *ids {
            assert!(
                rows.iter().any(|row| row.id == *id && row.form == *form),
                "the annotation of `{id}` {} names no row",
                form.name()
            );
            assert!(
                seen.insert((*id, *form)),
                "`{id}` {} is annotated twice",
                form.name()
            );
        }
    }
}

/// A field added to `CommandContext` or `PcbTargets` stops this compiling
/// until the pattern names it; the count below must then be raised, and
/// `FACTS` must have that many distinct names. The test does not match the
/// names in `FACTS` against the field names.
#[test]
fn every_context_fact_is_listed() {
    let CommandContext {
        has_schematic: _,
        has_pcb: _,
        pcb_view_turned: _,
        undocked_has_canvas: _,
        document_is_schematic: _,
        document_is_editor: _,
        document_is_dirty_board: _,
        has_sheets: _,
        has_symbol_editor: _,
        has_footprint_editor: _,
        has_selection: _,
        selection_all_locked: _,
        has_drawing_selection: _,
        drawing: _,
        can_undo: _,
        can_redo: _,
        project_dirty: _,
        document_has_file: _,
        has_dirty_documents: _,
        has_document: _,
        has_open_project: _,
        has_parent_sheet: _,
        has_active_path: _,
        has_library_editor: _,
        has_open_library: _,
        has_project_sheet: _,
        can_tile_windows: _,
        is_tiled: _,
        can_remove_part: _,
        pcb:
            signex_commands::PcbTargets {
                shown: _,
                any: _,
                placeable: _,
                deletable: _,
                copyable: _,
                pasteable: _,
                part_in_hand: _,
                routing: _,
                lockable: _,
                unlockable: _,
                read_only: _,
                footprints: _,
                pad_pair: _,
                one_footprint: _,
            },
        turn_in_hand: _,
    } = CommandContext::default();
    let names: BTreeSet<&str> = FACTS.iter().map(|(name, _, _)| *name).collect();
    assert_eq!(names.len(), FACTS.len(), "a fact is listed twice");
    assert_eq!(FACTS.len(), 30 + 14, "30 context facts and 14 board facts");
}

/// No core row stands on its gate alone: every core route's handler was
/// read, so the kill check covers them all (#2504).
#[test]
fn no_core_row_is_gate_only() {
    let unread: Vec<String> = rows()
        .iter()
        .filter(|row| row.owner == Owner::Core && row.basis == Basis::Gate)
        .map(|row| format!("{}\t{}", row.id, row.form.name()))
        .collect();
    assert!(
        unread.is_empty(),
        "core rows whose handler was not read; annotate them in parity/annotations.rs:\n{}",
        unread.join("\n")
    );
}

/// The kill list as the golden spells it: core rows that read in-progress
/// edits or the render cache. The origin never puts a row on it.
#[test]
fn the_kill_list_is_in_progress_edit_or_render_cache_only() {
    for row in rows() {
        let words = row.inputs_cell();
        let listed = row.owner == Owner::Core
            && (words.contains("in-progress edit") || words.contains("render cache"));
        assert_eq!(
            row.meets_the_kill_criterion(),
            listed,
            "{} {}",
            row.id,
            row.form.name()
        );
    }
}

/// The kill list, pinned: a row joining it or dropping off it is a
/// maintainer decision (ADR-0006 slice C, #2504), not a regenerate.
const KILL_LIST: &[(&str, &str)] = &[
    ("assign_net_class", "operand_handler"),
    ("close_active_document", "operand_handler"),
    ("draw_wire", "operand_handler"),
    ("flip_selected_x", "active_bar"),
    ("flip_selected_x", "refusal_needs"),
    ("flip_selected_x", "disabled_still_runs_refusal"),
    ("flip_selected_y", "active_bar"),
    ("flip_selected_y", "refusal_needs"),
    ("flip_selected_y", "disabled_still_runs_refusal"),
    ("mirror_x", "bridge"),
    ("mirror_x", "refusal_needs"),
    ("mirror_x", "disabled_still_runs_refusal"),
    ("mirror_y", "bridge"),
    ("mirror_y", "refusal_needs"),
    ("mirror_y", "disabled_still_runs_refusal"),
    ("paste_special", "bridge"),
    ("paste_special", "disabled_still_runs_refusal"),
    ("place_wire", "operand_handler"),
    ("project_add_requirement", "operand_handler"),
    ("project_remove_requirement", "operand_handler"),
    ("redo", "operand_handler"),
    ("revert", "operand_handler"),
    ("rotate_clockwise", "bridge"),
    ("rotate_clockwise", "refusal_needs"),
    ("rotate_clockwise", "disabled_still_runs_refusal"),
    ("rotate_counterclockwise", "bridge"),
    ("rotate_counterclockwise", "refusal_needs"),
    ("rotate_counterclockwise", "disabled_still_runs_refusal"),
    ("rotate_selection", "active_bar"),
    ("rotate_selection", "refusal_needs"),
    ("rotate_selection", "disabled_still_runs_refusal"),
    ("rotate_selection_cw", "active_bar"),
    ("rotate_selection_cw", "refusal_needs"),
    ("rotate_selection_cw", "disabled_still_runs_refusal"),
    ("run_erc", "bridge"),
    ("select_all", "bridge"),
    ("smart_paste", "bridge"),
    ("smart_paste", "disabled_still_runs_refusal"),
    ("undo", "operand_handler"),
];

#[test]
fn the_kill_list_is_the_pinned_one() {
    let live: BTreeSet<(&str, &str)> = rows()
        .iter()
        .filter(|row| row.meets_the_kill_criterion())
        .map(|row| (row.id, row.form.name()))
        .collect();
    let pinned: BTreeSet<(&str, &str)> = KILL_LIST.iter().copied().collect();
    assert_eq!(
        pinned.len(),
        KILL_LIST.len(),
        "a kill-list row is pinned twice"
    );
    let joined: Vec<_> = live.difference(&pinned).collect();
    let dropped: Vec<_> = pinned.difference(&live).collect();
    assert!(
        joined.is_empty() && dropped.is_empty(),
        "the kill list changed; joined {joined:?}, dropped {dropped:?}"
    );
}

/// An id the catalog has not decided reads `undecided` on every form.
#[test]
fn an_undecided_id_is_undecided_on_every_form() {
    for form in [
        Form::Bridge,
        Form::Operand,
        Form::HeadlessRefusal,
        Form::InWindow,
    ] {
        for skips in [false, true] {
            assert_eq!(owner_of(None, form, skips), Owner::Undecided);
        }
    }
    assert_eq!(
        owner_of(Some(HeadlessUse::Runs), Form::Bridge, true),
        Owner::Gui
    );
    assert_eq!(
        owner_of(Some(HeadlessUse::Runs), Form::Bridge, false),
        Owner::Core
    );
}
