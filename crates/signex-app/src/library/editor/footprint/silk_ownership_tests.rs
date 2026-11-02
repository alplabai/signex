//! #1718 — a silk re-bake replaces only the entries the sketch baked.
//! Hand-drawn silk survives re-bakes, deletes, undo, a late drag solve and
//! a file from an older build.

use std::path::PathBuf;

use signex_library::FootprintFile;
use signex_library::primitive::footprint::{Footprint, FpGraphic, FpGraphicKind};
use signex_sketch::attr::SilkAttr;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_types::layer::SignexLayer;

use super::sketch_bake_tests::{delete_all_entities, entity_mut, hand_text, rebuild, square};
use super::sketch_dispatch::apply_sketch_edit;
use super::sketch_mode::SketchEdit;
use super::sketch_solve_task::{dispatch_pending, run_solve, snapshot_request};
use super::state::FootprintEditorState;
use super::updates::apply_footprint_primitive_edit;
use crate::app::FootprintDocument;
use crate::library::messages::FootprintEditorMsg;

/// The stock polarised 0805 as trunk wrote it before #1718: one baked,
/// unmarked cathode bar from (-1.7, -0.7) to (-1.7, 0.7) on top silk.
const PRE_1718_STOCK: &str = include_str!(
    "../../../../../signex-library/tests/fixtures/snxfpt/pre_1718_chip_0805_polarised.snxfpt"
);

/// The fixture's free cathode-bar end point (-1.7, -0.7).
fn bar_start() -> SketchEntityId {
    SketchEntityId("c28fd9f3-638f-4440-9ed8-190c58a6f17e".parse().unwrap())
}

fn top_silk() -> Option<SilkAttr> {
    Some(SilkAttr {
        layer: SignexLayer::TopSilk,
    })
}

/// A hand line with the bake's own stroke width, away from any bake output.
fn hand_line() -> FpGraphic {
    FpGraphic::drawn(
        FpGraphicKind::Line {
            from: [0.0, 3.0],
            to: [1.0, 3.0],
        },
        0.12,
        false,
    )
}

fn hand_entries(fp: &Footprint) -> Vec<FpGraphic> {
    fp.silk_f
        .iter()
        .filter(|g| !g.is_baked())
        .cloned()
        .collect()
}

fn reopen(text: &str, stem: &str) -> FootprintDocument {
    let file = FootprintFile::from_toml_str(text).expect("parse");
    FootprintDocument::new(PathBuf::from(format!("{stem}.snxfpt")), file)
}

/// The pre-#1718 stock file with hand silk an older build saved beside
/// the baked bar.
fn legacy_with_hand_silk(edit: impl FnOnce(&mut Footprint)) -> String {
    let mut file = FootprintFile::from_toml_str(PRE_1718_STOCK).expect("fixture");
    let fp = &mut file.footprints[0];
    fp.silk_f.push(hand_text());
    fp.silk_f.push(hand_line());
    edit(fp);
    file.to_toml_string().expect("serialise")
}

/// A drag tick on the cathode bar, solved the way the editor solves it.
fn drag_bar(editor: &mut FootprintDocument) {
    let tick = FootprintEditorMsg::SketchMovePoint {
        id: bar_start(),
        dx: 0.0,
        dy: -0.1,
    };
    apply_footprint_primitive_edit(editor, tick);
    editor.settle_pending_solve();
}

#[test]
fn deleting_the_last_silk_entity_or_every_entity_removes_only_baked_silk() {
    let (mut fp, edges, _) = square();
    entity_mut(&mut fp, edges[0]).silk = top_silk();
    fp.silk_f = vec![hand_text(), hand_line()];
    let mut state = FootprintEditorState::from_footprint(&fp);
    rebuild(&mut state, &mut fp);
    assert_eq!(fp.silk_f.len(), 3);

    apply_sketch_edit(&mut state, &mut fp, SketchEdit::DeleteEntity(edges[0])).unwrap();
    assert_eq!(
        fp.silk_f,
        vec![hand_text(), hand_line()],
        "last silk entity"
    );

    entity_mut(&mut fp, edges[1]).silk = top_silk();
    rebuild(&mut state, &mut fp);
    assert_eq!(fp.silk_f.len(), 3);
    delete_all_entities(&mut state, &mut fp);
    assert_eq!(fp.silk_f, vec![hand_text(), hand_line()], "every entity");
}

#[test]
fn a_pre_1718_file_is_classified_on_open_and_a_move_neither_doubles_nor_drops_silk() {
    let mut editor = reopen(&legacy_with_hand_silk(|_| {}), "legacy-1718");
    assert!(!editor.is_dirty(), "classifying is not an edit");
    let baked: Vec<_> = editor
        .primitive()
        .silk_f
        .iter()
        .map(FpGraphic::is_baked)
        .collect();
    assert_eq!(baked, vec![true, false, false]);
    assert!(
        editor.file.changed_since(&editor.baseline).is_empty(),
        "a save must not treat the markers as an edit"
    );

    drag_bar(&mut editor);

    let silk = &editor.primitive().silk_f;
    assert_eq!(
        silk.len(),
        3,
        "polarity mark doubled or hand silk lost: {silk:?}"
    );
    let FpGraphicKind::Line { from, .. } = &silk[0].kind else {
        panic!("the bar comes first: {silk:?}");
    };
    assert!((from[1] + 0.8).abs() < 1e-9, "the bar moved: {from:?}");
    assert_eq!(
        hand_entries(editor.primitive()),
        vec![hand_text(), hand_line()]
    );
}

#[test]
fn an_unsolvable_legacy_sketch_marks_nothing_deletes_nothing_and_warns_once() {
    let stem = "unsolvable-1718";
    let text = legacy_with_hand_silk(make_unsolvable);
    let loaded = FootprintFile::from_toml_str(&text).unwrap().footprints[0]
        .silk_f
        .clone();
    let warned_before = crate::diagnostics::logged_count(stem);

    let editor = reopen(&text, stem);

    assert_eq!(editor.primitive().silk_f, loaded);
    assert!(editor.primitive().silk_f.iter().all(|g| !g.is_baked()));
    assert_eq!(crate::diagnostics::logged_count(stem), warned_before + 1);
    assert!(!editor.is_dirty());
}

#[test]
fn markers_an_older_build_stripped_are_recovered_on_reopen() {
    let mut editor = reopen(&legacy_with_hand_silk(|_| {}), "strip-1718");
    drag_bar(&mut editor);
    let saved = editor.file.to_toml_string().unwrap();
    assert!(saved.contains("sketch_entity = "));

    // An older build drops the field it does not know on save.
    let stripped: String = saved
        .lines()
        .filter(|l| !l.starts_with("sketch_entity = "))
        .map(|l| format!("{l}\n"))
        .collect();
    let editor = reopen(&stripped, "strip-1718-again");

    let silk = &editor.primitive().silk_f;
    assert_eq!(
        silk[0].sketch_entity,
        Some(SketchEntityId(
            "7fff648f-930d-4e7e-a1e8-ddcc9023e196".parse().unwrap()
        ))
    );
    assert_eq!(
        hand_entries(editor.primitive()),
        vec![hand_text(), hand_line()]
    );
}

#[test]
fn undo_and_redo_across_a_bake_restore_silk_and_markers() {
    let mut editor = reopen(&legacy_with_hand_silk(|_| {}), "undo-1718");
    drag_bar(&mut editor);
    let after = editor.primitive().silk_f.clone();
    let tag = FootprintEditorMsg::SketchSetRole {
        id: SketchEntityId("7fff648f-930d-4e7e-a1e8-ddcc9023e196".parse().unwrap()),
        role: crate::library::messages::RoleTag::Unassigned,
    };
    apply_footprint_primitive_edit(&mut editor, tag);
    assert_eq!(editor.primitive().silk_f, vec![hand_text(), hand_line()]);

    assert!(editor.undo());
    assert_eq!(editor.primitive().silk_f, after);
    assert!(editor.primitive().silk_f[0].is_baked());
    assert!(editor.redo());
    assert_eq!(editor.primitive().silk_f, vec![hand_text(), hand_line()]);
}

#[test]
fn a_late_drag_solve_merges_into_the_current_silk() {
    let path = PathBuf::from("late-1718.snxfpt");
    let mut editor = FootprintDocument::new(
        path.clone(),
        FootprintFile::from_toml_str(PRE_1718_STOCK).unwrap(),
    );
    let tick = FootprintEditorMsg::SketchMovePoint {
        id: bar_start(),
        dx: 0.0,
        dy: -0.1,
    };
    apply_footprint_primitive_edit(&mut editor, tick);
    let _task = dispatch_pending(&mut editor, &path);
    let solved = run_solve(snapshot_request(&editor).expect("a sketch"));

    // Drawn while the solve ran.
    editor.primitive_mut().silk_f.push(hand_text());
    apply_footprint_primitive_edit(&mut editor, solved);

    let silk = &editor.primitive().silk_f;
    assert_eq!(silk.len(), 2, "{silk:?}");
    assert!(silk[0].is_baked());
    assert_eq!(silk[1], hand_text());
}

/// Two conflicting distances on the cathode bar: the sketch cannot solve.
fn make_unsolvable(fp: &mut Footprint) {
    let sketch = fp.sketch.as_mut().unwrap();
    let end = SketchEntityId("58ea49eb-0a16-407d-b677-1a20322ad110".parse().unwrap());
    for mm in [1.0, 2.0] {
        sketch.constraints.push(Constraint {
            id: ConstraintId::new(),
            kind: ConstraintKind::DistancePtPt {
                p1: bar_start(),
                p2: end,
                target: DimTarget::Literal(mm),
            },
        });
    }
}

#[test]
fn a_file_this_build_saved_is_not_classified_again_on_open() {
    let stem = "this-build-1718";
    let mut editor = reopen(&legacy_with_hand_silk(|_| {}), "this-build-1718-first");
    drag_bar(&mut editor);
    let mut file = editor.file.clone();
    // Classifying would have to solve it, and say it could not.
    make_unsolvable(&mut file.footprints[0]);
    let text = file.to_toml_string().unwrap();
    let saved = file.footprints[0].silk_f.clone();
    let warned_before = crate::diagnostics::logged_count(stem);

    let editor = reopen(&text, stem);

    assert_eq!(crate::diagnostics::logged_count(stem), warned_before);
    assert_eq!(editor.primitive().silk_f, saved);
}

#[test]
fn silk_left_unmatched_beside_an_unused_bake_output_is_reported() {
    let stem = "partial-1718";
    // An older build saved the bar, then someone moved it in Pads mode:
    // it no longer matches what the sketch bakes.
    let text = legacy_with_hand_silk(|fp| {
        if let FpGraphicKind::Line { from, .. } = &mut fp.silk_f[0].kind {
            from[0] = -1.6;
        }
    });
    let warned_before = crate::diagnostics::logged_count(stem);

    let editor = reopen(&text, stem);

    assert!(editor.primitive().silk_f.iter().all(|g| !g.is_baked()));
    assert_eq!(crate::diagnostics::logged_count(stem), warned_before + 1);
}

#[test]
fn a_refused_delete_of_baked_silk_keeps_the_redo_step() {
    let mut editor = reopen(&legacy_with_hand_silk(|_| {}), "redo-1718");
    let add = FootprintEditorMsg::AddText {
        x_mm: 4.0,
        y_mm: 4.0,
    };
    apply_footprint_primitive_edit(&mut editor, add);
    assert!(editor.undo());
    let (history, redo) = (editor.history.len(), editor.redo.len());
    assert_eq!(redo, 1);

    apply_footprint_primitive_edit(&mut editor, FootprintEditorMsg::SelectSilkF(Some(0)));
    let before = editor.primitive().silk_f.clone();
    apply_footprint_primitive_edit(&mut editor, FootprintEditorMsg::DeleteSilkF);

    assert_eq!(editor.primitive().silk_f, before, "the baked bar stays");
    assert_eq!(editor.history.len(), history, "no empty undo step");
    assert_eq!(editor.redo.len(), redo, "Redo is still available");
    assert!(editor.redo());
}
