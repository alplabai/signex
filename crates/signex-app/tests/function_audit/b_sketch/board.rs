//! m110 and m179 (board sketch mode), m179d (outline checks) and m179e
//! (holes on a pattern driven by parameters). m110 is superseded by m179,
//! so its controls are the ones m179 keeps: Bake, Discard and the Board
//! section of Properties.

use signex_app::app::{BoardSketchMsg, Message, PcbMsg};
use signex_app::library::editor::footprint::parameters_table::SketchParametersMsg;
use signex_app::library::editor::footprint::{CheckLine, CheckStatus, SketchTool};
use signex_app::library::messages::{FootprintEditorMsg, RoleTag};
use signex_app::panels::{BoardSketchAttrMsg, PanelMsg};
use signex_i18n::RenderTranslatable;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::id::SketchEntityId;

use super::support::{BoardRig, empty_board, near, open_board};

fn sketch_msg(rig: &mut BoardRig, msg: BoardSketchMsg) {
    rig.send(Message::Pcb(PcbMsg::Sketch(msg)));
}

fn outline_edges(rig: &mut BoardRig) -> Vec<SketchEntityId> {
    let edges = rig.rectangle((0.0, 0.0), (40.0, 30.0));
    assert_eq!(edges.len(), 4, "a rectangle is four lines");
    rig.panel(PanelMsg::BoardSketch(BoardSketchAttrMsg::OutlineToggled {
        id: edges[0],
        on: true,
    }));
    edges
}

fn check_lines(rig: &BoardRig) -> Vec<CheckLine> {
    rig.app
        .document_state
        .panel_ctx
        .footprint_editor
        .as_ref()
        .and_then(|fp| fp.board.as_ref())
        .expect("board sketch Properties")
        .outline_checks
        .clone()
}

/// Push a linear pattern of `source` straight into the working sketch: the
/// pattern tool's own click is covered on the footprint host.
fn pattern_holes(rig: &mut BoardRig, source: SketchEntityId, count: &str, dx: &str) {
    let session = rig
        .app
        .interaction_state
        .board_sketch
        .as_mut()
        .expect("sketch mode");
    session
        .editor
        .primitive_mut()
        .sketch
        .as_mut()
        .expect("a working sketch")
        .arrays
        .push(Array {
            id: ArrayId::new(),
            kind: ArrayKind::Linear {
                source,
                count_expr: count.into(),
                dx_expr: dx.into(),
                dy_expr: "0mm".into(),
            },
            numbering: NumberingScheme::default(),
        });
}

fn hole_xs(rig: &BoardRig) -> Vec<f64> {
    let stored = rig.board().sketch.as_ref().expect("the board's sketch");
    let mut xs: Vec<f64> = stored.holes.iter().map(|h| h.position.x).collect();
    xs.sort_by(f64::total_cmp);
    xs
}

#[test]
fn m179_bake_writes_the_outline_and_leaves_as_one_undo_step() {
    let mut rig = open_board(empty_board());
    let before = rig.snapshot();
    rig.toggle_sketch();
    outline_edges(&mut rig);
    assert_eq!(rig.snapshot(), before, "drawing does not reach the board");

    sketch_msg(&mut rig, BoardSketchMsg::BakeRequested);

    assert!(!rig.in_sketch(), "Bake leaves");
    assert_eq!(rig.board().outline.len(), 4);
    assert!(rig.dirty());
    rig.undo();
    assert_eq!(rig.snapshot(), before, "one undo restores the board");
}

#[test]
fn m179_discard_asks_first_and_keep_editing_keeps_the_drawing() {
    let mut rig = open_board(empty_board());
    rig.toggle_sketch();
    outline_edges(&mut rig);

    sketch_msg(&mut rig, BoardSketchMsg::DiscardRequested);
    assert!(rig.in_sketch(), "a changed sketch asks first");
    sketch_msg(&mut rig, BoardSketchMsg::DiscardCancelled);

    assert!(rig.in_sketch());
    assert_eq!(rig.lines().len(), 4, "the drawing is kept");
}

#[test]
fn m179_a_confirmed_discard_leaves_the_board_as_it_was() {
    let mut rig = open_board(empty_board());
    let before = rig.snapshot();
    rig.toggle_sketch();
    outline_edges(&mut rig);

    sketch_msg(&mut rig, BoardSketchMsg::DiscardRequested);
    sketch_msg(&mut rig, BoardSketchMsg::DiscardConfirmed);

    assert!(!rig.in_sketch());
    assert_eq!(rig.snapshot(), before);
    assert!(!rig.dirty(), "nothing reached the board");
}

#[test]
fn m179_an_open_outline_is_refused_and_keeps_the_board_and_the_mode() {
    let mut rig = open_board(empty_board());
    let before = rig.snapshot();
    rig.toggle_sketch();
    rig.edit(FootprintEditorMsg::SketchSetTool(SketchTool::Line));
    rig.click(0.0, 0.0);
    rig.click(10.0, 0.0);
    rig.edit(FootprintEditorMsg::ToolEscape);
    let line = rig.lines()[0];
    rig.panel(PanelMsg::BoardSketch(BoardSketchAttrMsg::OutlineToggled {
        id: line,
        on: true,
    }));

    sketch_msg(&mut rig, BoardSketchMsg::BakeRequested);

    assert!(rig.in_sketch(), "a refused bake keeps the mode on");
    assert_eq!(rig.snapshot(), before);
    let session = rig.app.interaction_state.board_sketch.as_ref().unwrap();
    assert!(session.refusal.is_some(), "the bar says why");
}

#[test]
fn m179_undo_in_the_sketch_steps_the_drawing_and_not_the_board() {
    let mut rig = open_board(empty_board());
    let before = rig.snapshot();
    rig.toggle_sketch();
    rig.edit(FootprintEditorMsg::SketchSetTool(SketchTool::Line));
    rig.click(0.0, 0.0);
    rig.click(10.0, 0.0);
    rig.edit(FootprintEditorMsg::ToolEscape);
    assert_eq!(rig.lines().len(), 1);

    rig.undo();

    assert!(rig.lines().is_empty(), "the line is undone");
    assert!(rig.in_sketch());
    assert_eq!(rig.snapshot(), before);
}

#[test]
fn m179_the_sketch_marks_the_board_origin_where_the_pcb_view_has_it() {
    let mut board = empty_board();
    board.options.origin.at.x = 3.0;
    board.options.origin.at.y = 4.0;
    let mut rig = open_board(board);
    rig.toggle_sketch();

    let session = rig.app.interaction_state.board_sketch.as_ref().unwrap();
    let marker = session
        .editor
        .state
        .origin_marker_mm()
        .expect("the marker is shown");

    assert!(near(marker.0, 3.0) && near(marker.1, 4.0), "{marker:?}");
}

#[test]
fn m110_a_circle_made_a_mounting_hole_bakes_with_its_drill_and_net() {
    let mut rig = open_board(empty_board());
    rig.toggle_sketch();
    outline_edges(&mut rig);
    let hole = rig.circle(5.0, 5.0);
    rig.panel(PanelMsg::BoardSketch(
        BoardSketchAttrMsg::MountingHoleToggled { id: hole, on: true },
    ));
    rig.panel(PanelMsg::BoardSketch(BoardSketchAttrMsg::HoleDrillEdited {
        id: hole,
        value: "3.2mm".into(),
    }));
    rig.panel(PanelMsg::BoardSketch(
        BoardSketchAttrMsg::HolePlatedToggled { id: hole, on: true },
    ));
    rig.panel(PanelMsg::BoardSketch(BoardSketchAttrMsg::HoleNetEdited {
        id: hole,
        value: "GND".into(),
    }));

    sketch_msg(&mut rig, BoardSketchMsg::BakeRequested);

    let stored = rig.board().sketch.as_ref().expect("the board's sketch");
    assert_eq!(stored.holes.len(), 1);
    assert!(
        near(stored.holes[0].drill, 3.2),
        "{}",
        stored.holes[0].drill
    );
    assert!(stored.holes[0].plated);
}

#[test]
fn m179_a_patterned_hole_bakes_every_instance() {
    let mut rig = open_board(empty_board());
    rig.toggle_sketch();
    outline_edges(&mut rig);
    let hole = rig.circle(5.0, 5.0);
    rig.panel(PanelMsg::BoardSketch(
        BoardSketchAttrMsg::MountingHoleToggled { id: hole, on: true },
    ));
    pattern_holes(&mut rig, hole, "4", "5mm");

    sketch_msg(&mut rig, BoardSketchMsg::BakeRequested);

    assert!(!rig.in_sketch(), "the bake was taken");
    let xs = hole_xs(&rig);
    assert_eq!(xs.len(), 4, "four holes from one pattern: {xs:?}");
    assert!(near(xs[3] - xs[0], 15.0), "{xs:?}");
}

#[test]
fn m179d_a_closed_outline_reads_as_one_loop() {
    let mut rig = open_board(empty_board());
    rig.toggle_sketch();
    let edges = rig.rectangle((0.0, 0.0), (40.0, 30.0));
    for id in edges {
        rig.panel(PanelMsg::FpEditorSetRole {
            id,
            role: RoleTag::BoardOutline,
        });
    }

    let lines = check_lines(&rig);

    assert!(
        lines.iter().all(|l| l.status != CheckStatus::Failed),
        "{lines:#?}"
    );
    assert_eq!(lines[0].status, CheckStatus::Passed);
}

#[test]
fn m179d_a_cutout_outside_fails_names_its_place_and_bake_still_runs() {
    let mut rig = open_board(empty_board());
    rig.toggle_sketch();
    for id in rig.rectangle((0.0, 0.0), (40.0, 30.0)) {
        rig.panel(PanelMsg::FpEditorSetRole {
            id,
            role: RoleTag::BoardOutline,
        });
    }
    let stray = rig.rectangle((60.0, 0.0), (70.0, 10.0))[0];
    rig.panel(PanelMsg::FpEditorSetRole {
        id: stray,
        role: RoleTag::Cutout,
    });

    let failed: Vec<CheckLine> = check_lines(&rig)
        .into_iter()
        .filter(|l| l.status == CheckStatus::Failed)
        .collect();
    assert_eq!(failed.len(), 1, "{failed:#?}");
    assert_eq!(failed[0].place, Some(stray));
    assert!(
        failed[0].text.render_en_us().contains("outside"),
        "{}",
        failed[0].text.render_en_us()
    );

    sketch_msg(&mut rig, BoardSketchMsg::BakeRequested);
    assert!(!rig.in_sketch(), "a failing check warns only");
    assert_eq!(rig.board().outline.len(), 4);
}

#[test]
fn m179e_a_size_parameter_drives_the_hole_pattern_spacing() {
    let mut rig = open_board(empty_board());
    rig.toggle_sketch();
    outline_edges(&mut rig);
    let hole = rig.circle(5.0, 5.0);
    rig.panel(PanelMsg::BoardSketch(
        BoardSketchAttrMsg::MountingHoleToggled { id: hole, on: true },
    ));
    rig.edit(FootprintEditorMsg::SketchParameters(
        SketchParametersMsg::ExpressionEdited {
            name: "hole_pitch".into(),
            expr: "10mm".into(),
        },
    ));
    pattern_holes(&mut rig, hole, "3", "hole_pitch");

    sketch_msg(&mut rig, BoardSketchMsg::BakeRequested);

    let xs = hole_xs(&rig);
    assert_eq!(xs.len(), 3, "{xs:?}");
    assert!(near(xs[1] - xs[0], 10.0), "{xs:?}");
    assert!(near(xs[2] - xs[1], 10.0), "{xs:?}");
}

#[test]
fn m179e_a_parameter_added_in_the_sketch_stays_off_the_board_until_bake() {
    let mut rig = open_board(empty_board());
    let before = rig.snapshot();
    rig.toggle_sketch();

    rig.edit(FootprintEditorMsg::SketchParameters(
        SketchParametersMsg::AddClicked,
    ));

    assert!(rig.working().parameters.get_raw("param_1").is_some());
    assert_eq!(rig.snapshot(), before);
    rig.undo();
    assert!(rig.working().parameters.get_raw("param_1").is_none());
}
