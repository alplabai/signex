//! The parity golden and the checks on its rows.

use std::collections::BTreeSet;

use super::super::CommandContext;
use super::annotations::ANNOTATIONS;
use super::inputs::{FACTS, Input, read_by};
use super::{HEADER, rows, table_text};

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
/// `none` after a review; the golden is checked too, so a hand edit of it
/// cannot slip a word in.
#[test]
fn every_row_has_implicit_inputs_from_the_vocabulary() {
    let unreviewed: Vec<String> = rows()
        .iter()
        .filter(|row| !row.has_value())
        .map(|row| format!("{}\t{}", row.id, row.form.name()))
        .collect();
    assert!(
        unreviewed.is_empty(),
        "rows with no implicit input and no review; annotate them in parity/annotations.rs:\n{}",
        unreviewed.join("\n")
    );
    let words: BTreeSet<&str> = Input::ALL.iter().map(|input| input.word()).collect();
    for line in GOLDEN.lines().skip(1) {
        let cells: Vec<&str> = line.split('\t').collect();
        assert_eq!(cells.len(), 5, "row `{line}` has five cells");
        let inputs = cells[4];
        assert!(!inputs.is_empty(), "row `{line}` has no implicit inputs");
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

/// Each fact of `CommandContext` has its row in `FACTS`: a field added to
/// either struct stops this compiling until it is listed.
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

/// The probe finds exactly the facts a gate reads.
#[test]
fn the_probe_reads_what_a_gate_reads() {
    assert_eq!(
        read_by(|ctx| ctx.has_selection && ctx.pcb.pasteable),
        BTreeSet::from([Input::Selection, Input::Clipboard])
    );
    assert_eq!(
        read_by(|ctx| ctx.drawing || ctx.can_tile_windows),
        BTreeSet::from([Input::InProgressEdit, Input::Window])
    );
    assert!(read_by(|_| true).is_empty());
}
