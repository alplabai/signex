//! The parity golden and the checks on its rows.

use std::collections::BTreeSet;

use super::super::{CommandContext, enablement, needs};
use super::annotations::ANNOTATIONS;
use super::forms::{Form, Owner, owner_of};
use super::inputs::{FACTS, Input, MIN_PIVOTS, facts_read_by, pivots_of, read_by, samples};
use super::{Basis, HEADER, rows, table_text};
use crate::keymap::{AppCommandId, HeadlessUse, all_command_ids};

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
        assert!(
            inputs != "none" || cells[5] != "gate",
            "row `{line}` says `none` from its gate alone"
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
    // A conjunction of six facts: each decides it only when the other five
    // hold, which the probe still finds at least MIN_PIVOTS times.
    let six = |ctx: &CommandContext| {
        ctx.has_schematic
            && ctx.has_selection
            && ctx.drawing
            && ctx.is_tiled
            && ctx.pcb.pasteable
            && ctx.pcb.any
    };
    assert!(pivots_of(six).values().all(|pivots| *pivots >= MIN_PIVOTS));
    assert_eq!(
        facts_read_by(|ctx| ctx.has_schematic
            && ctx.has_selection
            && ctx.drawing
            && ctx.is_tiled
            && ctx.pcb.pasteable
            && ctx.pcb.any),
        BTreeSet::from([
            "has_schematic",
            "has_selection",
            "drawing",
            "is_tiled",
            "pcb.pasteable",
            "pcb.any",
        ])
    );
}

/// A gate the table probes, named.
type Gate = (String, Box<dyn Fn(&CommandContext) -> bool>);

/// Every gate the table probes: each `needs()` gate, and the locked-selection
/// refusal, which depends on the id only through its gate.
fn every_gate() -> Vec<Gate> {
    let mut gates: Vec<Gate> = Vec::new();
    for id in all_command_ids() {
        let command = AppCommandId::new(id).expect("catalog id is valid");
        let gate = needs::needs(&command);
        let names = [
            format!("{gate:?}"),
            format!("explains_refusal_in with {gate:?}"),
        ];
        if gates.iter().any(|(known, _)| *known == names[0]) {
            continue;
        }
        let [needs_name, refusal_name] = names;
        if let Some(gate) = gate {
            gates.push((needs_name, Box::new(move |ctx| gate.is_met(ctx))));
        } else {
            gates.push((needs_name, Box::new(|_| true)));
        }
        gates.push((
            refusal_name,
            Box::new(move |ctx| enablement::explains_refusal_in(&command, ctx)),
        ));
    }
    gates
}

/// Every fact a gate reads decides it in at least `MIN_PIVOTS` samples, so
/// none needs more than 6 other facts set; a gate drifting toward facts the
/// sample count could miss fails here first.
#[test]
fn every_found_fact_is_pivotal_often() {
    for (name, gate) in every_gate() {
        for (fact, pivots) in pivots_of(gate) {
            assert!(
                pivots >= MIN_PIVOTS,
                "{name}: `{fact}` decides it in {pivots} samples only; raise SAMPLES"
            );
        }
    }
}

/// The probe sets `pcb.footprints` to 0 or 2 only, which is sound while no
/// gate tells 1 from 0 or 2.
#[test]
fn no_gate_reads_the_footprint_count() {
    for (name, gate) in every_gate() {
        for ctx in samples() {
            let answers: Vec<bool> = [0, 1, 2]
                .into_iter()
                .map(|footprints| {
                    let mut ctx = ctx;
                    ctx.pcb.footprints = footprints;
                    gate(&ctx)
                })
                .collect();
            assert!(
                answers.iter().all(|answer| *answer == answers[0]),
                "{name} reads the footprint count; give the probe that fact"
            );
        }
    }
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
