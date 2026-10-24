//! #964 / #965 — the pad Shape and Layer pickers must not store
//! something other than what they show.
//!
//! #965: moving a pad from Multi-Layer to a single copper layer used to
//! leave its drill in the model, hidden by the UI, and the hole reached
//! the saved file and the sketch bake. Top / Bottom now clears the
//! drill, Multi-Layer restores a default, one undo reverses either.
//!
//! #964: the shape picker offered Octagonal and Donut and wrote Round.
//! Every shape the picker can still produce must survive a save/reopen.

#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results with `let _ = ...` routinely; this is test scaffolding, not a production `iced::Task` being dropped, and `[lints]` in Cargo.toml is package-scoped so each integration-test crate root needs its own"
)]

use signex_app::app::{EditMsg, FootprintEditorState, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
use signex_app::library::editor::footprint::state::{
    DEFAULT_THROUGH_HOLE_DRILL_MM, EditorPad, PadSide,
};
use signex_app::panels::PanelMsg;
use signex_library::primitive::footprint::ChamferedCorners;
use signex_library::{Footprint, FootprintFile, LayerId, PadKind, PadShape};
use std::path::PathBuf;

/// One minted through-hole pad (0.9 mm drill, Multi-Layer) in an editor
/// wired to the active tab.
fn through_hole_editor(stem: &str) -> (Signex, PathBuf) {
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.kind = PadKind::Tht;
    pad.size_mm = (1.6, 1.6);
    pad.shape = PadShape::Round;
    pad.layers = vec![
        LayerId::new("*.Cu"),
        LayerId::new("F.Mask"),
        LayerId::new("B.Mask"),
    ];
    pad.drill_diameter_mm = Some(0.9);

    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let mut fp = Footprint::empty(stem);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    // A file on disk carries its pads in the primitive too; without
    // them an undo snapshot would be a footprint with no pads.
    let mut canvas = signex_app::library::editor::footprint::state::FootprintEditorState::empty();
    canvas.pads = vec![pad.clone()];
    signex_app::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(
        &canvas, &mut fp,
    );
    let mut editor = FootprintEditorState::new(path.clone(), FootprintFile::from_footprint(fp));
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

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
    (app, path)
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn set_side(app: &mut Signex, side: PadSide) {
    panel(app, PanelMsg::FpEditorSetSelectedPadSide { idx: 0, side });
}

fn editor<'a>(app: &'a Signex, path: &PathBuf) -> &'a FootprintEditorState {
    app.document_state.footprint_editors.get(path).unwrap()
}

/// What a save followed by a reopen yields for the editor's footprint.
fn saved_and_reopened(app: &Signex, path: &PathBuf) -> Footprint {
    let file = FootprintFile::from_footprint(editor(app, path).primitive().clone());
    let text = file.to_toml_string().expect("footprint serialises");
    let reopened = FootprintFile::from_toml_str(&text).expect("saved footprint reopens");
    reopened.footprints[0].clone()
}

/// The pads the sketch bake would emit, as `bake_pads` produces them.
fn baked_pads(app: &Signex, path: &PathBuf) -> Vec<signex_library::Pad> {
    use signex_sketch::solver::Solver;
    use signex_sketch::solver::residual::ResolvedParams;

    let sketch = editor(app, path)
        .primitive()
        .sketch
        .as_ref()
        .expect("sketch present");
    let solve = Solver::default()
        .solve(sketch, &ResolvedParams::new())
        .expect("solve");
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    signex_bake::bake_pads(
        sketch,
        &solve,
        &std::collections::HashMap::new(),
        &mut out,
        &mut warnings,
    )
    .expect("bake_pads ok");
    out
}

#[test]
fn multi_layer_to_top_removes_the_hole_from_model_save_and_bake() {
    let (mut app, path) = through_hole_editor("side-drill-top");
    assert_eq!(
        baked_pads(&app, &path)[0]
            .drill
            .as_ref()
            .map(|d| d.diameter),
        Some(0.9)
    );

    set_side(&mut app, PadSide::Top);

    assert_eq!(editor(&app, &path).state.pads[0].drill_diameter_mm, None);
    assert!(
        saved_and_reopened(&app, &path).pads[0].drill.is_none(),
        "the saved pad must not carry a hidden hole"
    );
    assert!(
        baked_pads(&app, &path)[0].drill.is_none(),
        "the bake output must not carry a hidden hole"
    );
}

#[test]
fn a_slot_goes_with_the_drill() {
    let (mut app, path) = through_hole_editor("side-drill-slot");
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadDrillSlotLength {
            idx: 0,
            value: "1.8".into(),
        },
    );
    assert_eq!(
        editor(&app, &path).state.pads[0]
            .passthrough
            .drill_slot_length_mm,
        Some(1.8),
        "pre-condition: slot set"
    );

    set_side(&mut app, PadSide::Bottom);
    set_side(&mut app, PadSide::All);

    let pad = &editor(&app, &path).state.pads[0];
    assert_eq!(pad.passthrough.drill_slot_length_mm, None);
    assert_eq!(pad.drill_diameter_mm, Some(DEFAULT_THROUGH_HOLE_DRILL_MM));
}

#[test]
fn back_to_multi_layer_gets_a_default_drill() {
    let (mut app, path) = through_hole_editor("side-drill-back");
    set_side(&mut app, PadSide::Top);
    set_side(&mut app, PadSide::All);

    let pad = &editor(&app, &path).state.pads[0];
    assert_eq!(pad.drill_diameter_mm, Some(DEFAULT_THROUGH_HOLE_DRILL_MM));
    assert_eq!(
        saved_and_reopened(&app, &path).pads[0]
            .drill
            .as_ref()
            .map(|d| d.diameter),
        Some(DEFAULT_THROUGH_HOLE_DRILL_MM)
    );
    assert_eq!(
        baked_pads(&app, &path)[0]
            .drill
            .as_ref()
            .map(|d| d.diameter),
        Some(DEFAULT_THROUGH_HOLE_DRILL_MM)
    );
}

#[test]
fn staying_multi_layer_keeps_the_authored_drill() {
    let (mut app, path) = through_hole_editor("side-drill-keep");
    set_side(&mut app, PadSide::All);
    assert_eq!(
        editor(&app, &path).state.pads[0].drill_diameter_mm,
        Some(0.9)
    );
}

#[test]
fn one_undo_restores_the_hole() {
    let (mut app, path) = through_hole_editor("side-drill-undo");
    set_side(&mut app, PadSide::Top);
    assert_eq!(editor(&app, &path).state.pads[0].drill_diameter_mm, None);

    let _ = app.update(Message::Edit(EditMsg::Undo));

    let pad = &editor(&app, &path).state.pads[0];
    assert_eq!(pad.drill_diameter_mm, Some(0.9));
    assert!(
        pad.layers.iter().any(|l| l.as_str() == "*.Cu"),
        "undo restores the layers with the drill"
    );
    assert_eq!(
        saved_and_reopened(&app, &path).pads[0]
            .drill
            .as_ref()
            .map(|d| d.diameter),
        Some(0.9)
    );
}

#[test]
fn a_non_plated_hole_is_never_cleared_by_a_side_change() {
    let (mut app, path) = through_hole_editor("side-drill-npt");
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadKind {
            idx: 0,
            kind: PadKind::NptHole,
        },
    );
    set_side(&mut app, PadSide::Top);
    assert_eq!(
        editor(&app, &path).state.pads[0].drill_diameter_mm,
        Some(0.9)
    );
}

#[test]
fn every_offered_shape_survives_save_and_reopen() {
    let shapes = [
        PadShape::Round,
        PadShape::Rect,
        PadShape::RoundRect { radius_ratio: 0.25 },
        PadShape::Chamfered {
            chamfer_ratio: 0.25,
            corners: ChamferedCorners::all(),
        },
    ];
    for shape in shapes {
        let (mut app, path) = through_hole_editor("shape-roundtrip");
        panel(
            &mut app,
            PanelMsg::FpEditorSetSelectedPadShape {
                idx: 0,
                shape: shape.clone(),
            },
        );
        assert_eq!(
            saved_and_reopened(&app, &path).pads[0].shape,
            shape,
            "{shape:?} changed across save/reopen"
        );
    }
}
