//! Pad rotation is a geometry input, and the active-bar transforms act
//! on the whole selection.
//!
//! The pre-existing coverage in `regression.rs`
//! (`v026g_rotate_selection_increments_rotation_by_90_degrees` and
//! siblings) only ever asserted that the `rotation_deg` FIELD changed
//! on a single-pad selection. That is exactly why three defects
//! survived: nothing checked that rotation reached the geometry, and
//! nothing checked a multi-pad selection. These tests assert GEOMETRY
//! and MULTI-pad.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::library::editor::footprint::state::EditorPad;
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use std::path::PathBuf;
use tempfile::TempDir;

/// Fresh app + a footprint editor holding `count` default pads, with
/// the active tab pointed at it so `Message::Edit(EditMsg::Undo)`
/// resolves through `active_footprint_editor_path()`.
fn fixture(stem: &str, count: usize) -> (Signex, PathBuf, TempDir) {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_library::{Footprint, FootprintFile};

    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join(format!("{stem}.snxfpt"));
    std::fs::write(&path, b"{}").expect("write .snxfpt placeholder");

    let file = FootprintFile::from_footprint(Footprint::empty(stem));
    let mut editor = FootprintEditorState::new(path.clone(), file);
    for i in 0..count {
        editor.state.pads.push(EditorPad::new_default(
            (i + 1).to_string(),
            (i as f64 * 3.0, 0.0),
        ));
    }

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: stem.into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path, tmp)
}

fn dispatch(app: &mut Signex, path: &PathBuf, msg: FootprintEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(msg),
    }));
}

/// A 2×1 mm pad turned 90° occupies ±0.5 mm in X and ±1.0 mm in Y.
/// The old `contains_mm` tested the un-rotated box, so it answered
/// exactly backwards on both probes.
#[test]
fn rotated_pad_hit_tests_against_the_turned_copper() {
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.size_mm = (2.0, 1.0);
    pad.rotation_deg = 90.0;

    assert!(
        pad.contains_mm(0.0, 0.9),
        "(0, 0.9) is inside the rotated copper (half-height 1.0) and must hit"
    );
    assert!(
        !pad.contains_mm(0.9, 0.0),
        "(0.9, 0) is outside the rotated copper (half-width 0.5) and must MISS \
         — it only looks like a hit against the un-rotated box"
    );
}

/// The auto-fit courtyard is built from the pad extents. Reading the
/// un-rotated box let a turned pad stick out of its own courtyard.
#[test]
fn courtyard_encloses_the_rotated_pad() {
    use signex_app::library::editor::footprint::state::FootprintEditorState as CanvasState;

    let mut state = CanvasState::empty();
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.size_mm = (2.0, 1.0);
    pad.rotation_deg = 90.0;
    state.pads.push(pad);
    state.auto_fit_courtyard = true;
    state.recompute_courtyard();

    let court = state.courtyard_mm.expect("auto-fit produced a courtyard");
    assert!(
        court.max_y >= 1.0 && court.min_y <= -1.0,
        "rotated pad reaches ±1.0 mm in Y; courtyard is [{}, {}] and does not contain it",
        court.min_y,
        court.max_y
    );
    assert!(
        court.max_x >= 0.5 && court.min_x <= -0.5,
        "courtyard must still cover the rotated pad in X: [{}, {}]",
        court.min_x,
        court.max_x
    );
}

/// Rotate acted on `selected_pad` alone; pads 1 and 2 never turned.
#[test]
fn rotate_turns_every_pad_in_the_selection() {
    let (mut app, path, _tmp) = fixture("multi-rotate", 3);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = Some(0);
        editor.state.selected_pads_extra = vec![1, 2];
    }

    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::ActiveBarRotateSelection,
    );

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    for (i, pad) in editor.state.pads.iter().enumerate() {
        assert_eq!(
            pad.rotation_deg, 90.0,
            "pad {i} must be rotated — Rotate acts on the whole selection, not just the primary"
        );
    }
}

/// The fab-error case: a partial flip leaves some pads on F.* and some
/// on B.*, which is a board that cannot be built.
#[test]
fn flip_moves_every_selected_pad_to_the_back_side() {
    let (mut app, path, _tmp) = fixture("multi-flip", 3);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = Some(0);
        editor.state.selected_pads_extra = vec![1, 2];
        for pad in &editor.state.pads {
            assert!(
                pad.layers.iter().all(|l| l.as_str().starts_with("F.")),
                "fixture starts front-side"
            );
        }
    }

    dispatch(&mut app, &path, FootprintEditorMsg::ActiveBarFlipSelection);

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    for (i, pad) in editor.state.pads.iter().enumerate() {
        assert!(
            pad.layers.iter().all(|l| l.as_str().starts_with("B.")),
            "pad {i} still carries a front-side layer after Flip: {:?} \
             — mixed F./B. across one footprint is a fabrication error",
            pad.layers
                .iter()
                .map(|l| l.as_str().to_string())
                .collect::<Vec<_>>()
        );
    }
}

/// `apply_footprint_primitive_edit` blanket-pushes a snapshot for every
/// mutating message, and Rotate is not on its exemption list — so the
/// arm must NOT push its own. One Ctrl+Z has to reverse the whole
/// multi-pad rotate; two would mean the history got double-stacked.
#[test]
fn one_undo_reverses_the_whole_multi_pad_rotate() {
    let (mut app, path, _tmp) = fixture("multi-rotate-undo", 3);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = Some(0);
        editor.state.selected_pads_extra = vec![1, 2];
        for pad in editor.state.pads.iter_mut() {
            pad.rotation_deg = 45.0;
        }
    }

    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::ActiveBarRotateSelection,
    );
    {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        assert!(
            editor.state.pads.iter().all(|p| p.rotation_deg == 135.0),
            "pre-condition: all three rotated 45° → 135°"
        );
    }

    let _ = app.update(Message::Edit(EditMsg::Undo));

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    for (i, pad) in editor.state.pads.iter().enumerate() {
        assert_eq!(
            pad.rotation_deg, 45.0,
            "a SINGLE Ctrl+Z must restore pad {i} to its pre-rotate angle"
        );
    }
}
