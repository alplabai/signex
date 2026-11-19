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
