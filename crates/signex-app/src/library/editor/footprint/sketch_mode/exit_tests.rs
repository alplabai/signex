//! #2518 — the footprint host's way out of sketch mode.

use signex_types::sketch::SketchHost;
use signex_types::sketch::baked_kinds::BakedKind;

use super::*;
use crate::library::editor::footprint::state::EditorMode;
use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
use crate::library::messages::FootprintEditorMsg as M;

fn editor_in_sketch() -> FootprintDocument {
    use signex_library::{Footprint, FootprintFile};
    let file = FootprintFile::from_footprint(Footprint::empty("t"));
    let mut editor = FootprintDocument::new(std::path::PathBuf::from("t.snxfpt"), file);
    apply_footprint_primitive_edit(&mut editor, M::SetMode(EditorMode::Sketch));
    editor
}

fn draw_a_point(editor: &mut FootprintDocument) {
    apply_footprint_primitive_edit(
        editor,
        M::SketchPlacePoint {
            x_mm: 1.0,
            y_mm: 1.0,
        },
    );
}

fn entity_count(editor: &FootprintDocument) -> usize {
    editor
        .primitive()
        .sketch
        .as_ref()
        .map_or(0, |sketch| sketch.entities.len())
}

#[test]
fn bake_leaves_sketch_mode_and_keeps_the_edits() {
    let mut editor = editor_in_sketch();
    draw_a_point(&mut editor);
    let drawn = entity_count(&editor);
    assert!(drawn > 0, "the point was drawn");
    apply_footprint_primitive_edit(&mut editor, M::SketchBakeRequested);
    assert_eq!(editor.state.mode, EditorMode::Normal);
    assert_eq!(entity_count(&editor), drawn);
}

/// A footprint with `count` sketch pads, baked: Pads mode, nothing selected.
fn baked_with_pads(count: usize) -> FootprintDocument {
    use crate::library::messages::RoleTag;
    let mut editor = editor_in_sketch();
    for i in 0..count {
        apply_footprint_primitive_edit(
            &mut editor,
            M::SketchPlacePoint {
                x_mm: 2.0 * i as f64,
                y_mm: 1.0,
            },
        );
        let point = editor
            .primitive()
            .sketch
            .as_ref()
            .and_then(|sketch| sketch.entities.last())
            .map(|entity| entity.id)
            .expect("the point");
        apply_footprint_primitive_edit(
            &mut editor,
            M::SketchSetRole {
                id: point,
                role: RoleTag::Pad,
            },
        );
    }
    apply_footprint_primitive_edit(
        &mut editor,
        M::SketchSelect {
            id: None,
            shift: false,
        },
    );
    apply_footprint_primitive_edit(&mut editor, M::SketchBakeRequested);
    assert_eq!(editor.state.pads.len(), count, "the pads baked");
    assert!(shown_outcome(&editor).is_some(), "the outcome shows");
    editor
}

/// DECISIONS 2026-11-23 (#2835): one pad click keeps the panel, and so does
/// a click on another single pad.
#[test]
fn a_click_on_one_pad_keeps_the_outcome() {
    let mut editor = baked_with_pads(2);
    apply_footprint_primitive_edit(&mut editor, M::SelectPad(Some(0)));
    assert!(shown_outcome(&editor).is_some(), "the first pad");
    apply_footprint_primitive_edit(&mut editor, M::SelectPad(Some(1)));
    assert!(shown_outcome(&editor).is_some(), "another pad");
}

#[test]
fn an_empty_click_ends_the_outcome_with_or_without_a_pad_selected() {
    let mut editor = baked_with_pads(1);
    apply_footprint_primitive_edit(&mut editor, M::SelectPad(None));
    assert!(shown_outcome(&editor).is_none(), "nothing was selected");

    let mut editor = baked_with_pads(1);
    apply_footprint_primitive_edit(&mut editor, M::SelectPad(Some(0)));
    apply_footprint_primitive_edit(&mut editor, M::SelectPad(None));
    assert!(shown_outcome(&editor).is_none(), "a pad was selected");
    apply_footprint_primitive_edit(&mut editor, M::SelectPad(Some(0)));
    assert!(shown_outcome(&editor).is_none(), "it does not come back");
}

#[test]
fn a_rubber_band_over_nothing_ends_the_outcome() {
    let mut editor = baked_with_pads(1);
    apply_footprint_primitive_edit(&mut editor, M::SelectPads(Vec::new()));
    assert!(shown_outcome(&editor).is_none());
}

#[test]
fn a_multi_selection_ends_the_outcome() {
    let mut editor = baked_with_pads(2);
    apply_footprint_primitive_edit(&mut editor, M::SelectPads(vec![0, 1]));
    assert!(shown_outcome(&editor).is_none());
    apply_footprint_primitive_edit(&mut editor, M::SelectPad(Some(0)));
    assert!(shown_outcome(&editor).is_none(), "it does not come back");
}

#[test]
fn a_non_pad_object_ends_the_outcome() {
    let mut editor = baked_with_pads(1);
    apply_footprint_primitive_edit(&mut editor, M::SelectPad(Some(0)));
    apply_footprint_primitive_edit(&mut editor, M::SelectSilkF(Some(0)));
    assert!(shown_outcome(&editor).is_none());
}

/// An edit that leaves every count as it was still ends the panel.
#[test]
fn an_edit_that_keeps_the_counts_ends_the_outcome() {
    let mut editor = baked_with_pads(1);
    editor.primitive_mut().name = "edited".into();
    assert!(shown_outcome(&editor).is_none(), "a footprint field");

    let mut editor = baked_with_pads(1);
    editor.state.pads[0].position_mm.0 += 1.0;
    assert!(shown_outcome(&editor).is_none(), "a pad");
}

#[test]
fn undo_ends_the_outcome_and_redo_does_not_bring_it_back() {
    let mut editor = baked_with_pads(1);
    assert!(editor.undo());
    assert!(shown_outcome(&editor).is_none());
    assert!(editor.redo());
    assert!(shown_outcome(&editor).is_none());
}

/// DECISIONS 2026-11-22, mockup row 190: after Bake, Properties shows the
/// pad count and what was written, until the selection changes to anything
/// but one pad (DECISIONS 2026-11-23, #2835).
#[test]
fn bake_shows_its_outcome_until_the_selection_changes() {
    use crate::library::messages::RoleTag;
    let mut editor = editor_in_sketch();
    draw_a_point(&mut editor);
    let point = editor
        .primitive()
        .sketch
        .as_ref()
        .and_then(|sketch| sketch.entities.last())
        .map(|entity| entity.id)
        .expect("the point");
    apply_footprint_primitive_edit(
        &mut editor,
        M::SketchSetRole {
            id: point,
            role: RoleTag::Pad,
        },
    );
    apply_footprint_primitive_edit(&mut editor, M::SketchBakeRequested);

    let outcome = shown_outcome(&editor).expect("the outcome shows after Bake");
    assert_eq!(outcome.pads, 1);
    assert!(outcome.written.contains(&(BakedKind::Pads, 1)));

    editor.state.selected_silk_f = Some(0);
    assert!(shown_outcome(&editor).is_none(), "a new selection ends it");
    settle_outcome(&mut editor);
    editor.state.selected_silk_f = None;
    assert!(
        shown_outcome(&editor).is_none(),
        "and it does not come back"
    );
}

#[test]
fn a_bake_preview_records_no_outcome() {
    let mut editor = editor_in_sketch();
    apply_footprint_primitive_edit(&mut editor, M::SketchBakePreviewToggled);
    assert!(shown_outcome(&editor).is_none());
}

#[test]
fn discard_without_edits_leaves_without_asking() {
    let mut editor = editor_in_sketch();
    apply_footprint_primitive_edit(&mut editor, M::SketchDiscardRequested);
    assert_eq!(editor.state.mode, EditorMode::Normal);
    assert!(!editor.state.sketch_exit.confirming_discard);
}

#[test]
fn discard_with_edits_asks_then_restores_the_footprint_as_one_undo_step() {
    let mut editor = editor_in_sketch();
    let found = editor.primitive().clone();
    draw_a_point(&mut editor);
    assert!(has_changes(&editor));

    apply_footprint_primitive_edit(&mut editor, M::SketchDiscardRequested);
    assert_eq!(editor.state.mode, EditorMode::Sketch, "it asks first");
    assert!(editor.state.sketch_exit.confirming_discard);

    apply_footprint_primitive_edit(&mut editor, M::SketchDiscardCancelled);
    assert!(!editor.state.sketch_exit.confirming_discard);
    assert!(has_changes(&editor), "Keep editing keeps the edits");

    apply_footprint_primitive_edit(&mut editor, M::SketchDiscardRequested);
    apply_footprint_primitive_edit(&mut editor, M::SketchDiscardConfirmed);
    assert_eq!(editor.state.mode, EditorMode::Normal);
    assert_eq!(
        editor.primitive().sketch,
        found.sketch,
        "the edits are gone"
    );

    assert!(editor.undo(), "the discard is one undo step");
    assert!(entity_count(&editor) > 0, "Undo brings the edits back");
}

#[test]
fn the_preview_toggles_without_an_undo_step() {
    let mut editor = editor_in_sketch();
    let steps = editor.history.undo_len();
    apply_footprint_primitive_edit(&mut editor, M::SketchBakePreviewToggled);
    assert!(editor.state.sketch_exit.preview);
    apply_footprint_primitive_edit(&mut editor, M::SketchBakePreviewToggled);
    assert!(!editor.state.sketch_exit.preview);
    assert_eq!(editor.history.undo_len(), steps);
}

/// The preview counts exactly the targets the footprint host bakes.
#[test]
fn the_footprint_preview_counts_every_footprint_target() {
    let fp = signex_library::Footprint::empty("t");
    for kind in [
        BakedKind::Pads,
        BakedKind::SilkF,
        BakedKind::SilkB,
        BakedKind::Courtyard,
        BakedKind::MaskOpenings,
        BakedKind::MaskExcludes,
        BakedKind::PasteApertures,
        BakedKind::Pours,
        BakedKind::Keepouts,
        BakedKind::Cutouts,
        BakedKind::VScores,
        BakedKind::BoardOutline,
        BakedKind::MountingHoles,
    ] {
        assert_eq!(
            footprint_count(&fp, kind).is_some(),
            SketchHost::Footprint.bake_targets().contains(&kind),
            "{kind:?}"
        );
    }
}

/// #2518: Esc answers an open "Discard changes?" with Keep editing, as on a
/// board: the edits and sketch mode stay, and the question closes.
#[test]
fn esc_answers_the_discard_question_with_keep_editing() {
    for esc in [M::ToolEscape, M::SketchToolEscape] {
        let mut editor = editor_in_sketch();
        draw_a_point(&mut editor);
        let drawn = entity_count(&editor);
        apply_footprint_primitive_edit(&mut editor, M::SketchDiscardRequested);
        assert!(editor.state.sketch_exit.confirming_discard);
        apply_footprint_primitive_edit(&mut editor, esc.clone());
        assert!(!editor.state.sketch_exit.confirming_discard, "{esc:?}");
        assert_eq!(editor.state.mode, EditorMode::Sketch, "{esc:?}");
        assert_eq!(entity_count(&editor), drawn, "{esc:?}: the edits stay");
    }
}
