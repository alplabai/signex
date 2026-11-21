//! #2368 through `Signex::update`: a click picks a designator, a box and
//! Select All take its footprint, and Move and Measure pass over the text
//! for the part under it. With the Texts filter off, a press on a text
//! grabs nothing, so it starts a box.

use std::path::{Path, PathBuf};

use pcb_geom::{BoardTextField, ClickMode, ItemRef, PickKind};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::Message;
use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::keymap::AppCommandId;
use crate::pcb_canvas::{PcbCanvasMsg, SnapScope};

const R1: Uuid = Uuid::from_u128(0x10);
const DESIGNATOR: ItemRef = ItemRef::FootprintText {
    footprint: R1,
    field: BoardTextField::Designator,
};

/// R1 at (10, 10): pads one mm above and below its origin, its designator
/// at the origin, inside the body between the pads.
fn board() -> PcbBoard {
    let pad = |n: u128, y: f64| {
        serde_json::json!({
            "uuid": Uuid::from_u128(n), "number": n.to_string(),
            "pad_type": "smd", "shape": "rect", "position": {"x": 0.0, "y": y},
            "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "footprints": [{
            "uuid": R1, "reference": "R1", "position": {"x": 10.0, "y": 10.0},
            "layer": "TopCopper", "pads": [pad(0x11, -1.0), pad(0x12, 1.0)],
            "properties": [{ "key": "Reference", "value": "R1",
                             "position": {"x": 0.0, "y": 0.0}, "layer": "TopSilkscreen" }]
        }]
    }))
    .unwrap()
}

fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.ui_state.pcb_snap.to_grids = false;
    app.ui_state.pcb_snap.scope = SnapScope::Off;
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn run(app: &mut Signex, id: &str) {
    let command = AppCommandId::new(id).expect("a catalog id");
    let _ = app.dispatch_command(&command, CommandArgs::none(), None);
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
        at: Point::new(x, y),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }));
}

fn selected(app: &Signex) -> Vec<ItemRef> {
    app.interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect()
}

#[test]
fn a_click_picks_the_designator_and_a_box_or_select_all_the_footprint() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    click(&mut app, 10.0, 10.0);
    assert_eq!(selected(&app), vec![DESIGNATOR]);

    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::BoxSelected {
        from: Point::new(0.0, 0.0),
        to: Point::new(20.0, 20.0),
        mode: ClickMode::Replace,
        view_rotation_deg: 0.0,
    }));
    let boxed = selected(&app);
    assert!(boxed.contains(&ItemRef::Footprint(R1)), "{boxed:?}");
    assert!(!boxed.contains(&DESIGNATOR), "{boxed:?}");

    click(&mut app, 50.0, 50.0);
    run(&mut app, "select_all");
    let all = selected(&app);
    assert!(all.contains(&ItemRef::Footprint(R1)), "{all:?}");
    assert!(!all.contains(&DESIGNATOR), "{all:?}");
}

#[test]
fn move_picks_the_part_under_its_designator() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "move_object");
    click(&mut app, 10.0, 10.0);
    assert_eq!(selected(&app), vec![ItemRef::Footprint(R1)]);
    click(&mut app, 15.0, 10.0);
    let board = app.document_state.pcb_board(&path).expect("board");
    assert_eq!(board.footprints[0].position, Point::new(15.0, 10.0));
}

#[test]
fn measure_marks_the_part_under_its_designator() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    run(&mut app, "measure_distance");
    click(&mut app, 10.0, 10.0);
    let measure = app.interaction_state.pcb_query.measure.as_ref().unwrap();
    let first = measure.first.expect("the first point");
    assert_eq!(first.item, Some(ItemRef::Footprint(R1)));
}

fn drag(app: &mut Signex, from: Point, to: Point) {
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::FootprintDragged {
        from,
        to,
        tolerance_mm: 0.05,
        leaves_tracks: false,
    }));
}

#[test]
fn a_dragged_designator_moves_itself_with_no_ghost() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    drag(&mut app, Point::new(10.0, 10.0), Point::new(13.0, 12.0));
    let carry = app.interaction_state.pcb_carry.as_ref().expect("a carry");
    assert_eq!(carry.text, Some((R1, BoardTextField::Designator)));
    let canvas = &app.interaction_state.pcb_canvas;
    assert!(canvas.highlight().ghost.is_empty(), "no ghost");
    assert!(
        canvas.highlight().selected.is_empty(),
        "no mark left behind"
    );
    let base = canvas.active_renderer_snapshot().expect("a board");
    assert!(base.texts.is_empty(), "the text left its old place");
    assert_eq!(base.pads.len(), 2, "the part stays");
    let moving = canvas.drag_preview_texts();
    assert_eq!(moving.len(), 1);
    assert_eq!(moving[0].content, "R1");
    assert_eq!(moving[0].position, [13.0, 12.0]);
    let scene = canvas.drag_preview_scene().expect("a moving scene");
    assert!(scene.top_polygons.is_empty(), "its pads do not move");
}

/// No counter sees the fill or ratsnest code, so a marker line stands in: a
/// recomputed board would lose it.
#[test]
fn a_text_drag_keeps_the_ratsnest_and_fills_it_had() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    let canvas = &mut app.interaction_state.pcb_canvas;
    let mut shown = canvas.active_renderer_snapshot().expect("a board").clone();
    shown.ratsnest_lines.push(signex_renderer::pcb::RatsnestInput {
        p0: [1.0, 2.0],
        p1: [3.0, 4.0],
        net: 77,
    });
    canvas.set_renderer_snapshot(Some(shown));
    drag(&mut app, Point::new(10.0, 10.0), Point::new(13.0, 12.0));
    drag(&mut app, Point::new(10.0, 10.0), Point::new(14.0, 12.5));
    let canvas = &app.interaction_state.pcb_canvas;
    assert_eq!(canvas.drag_preview_texts()[0].position, [14.0, 12.5]);
    let base = canvas.active_renderer_snapshot().expect("a board");
    assert!(base.texts.is_empty(), "the text left its old place");
    assert_eq!(base.ratsnest_lines.len(), 1, "the ratsnest is the one shown");
    assert_eq!(base.ratsnest_lines[0].net, 77);
}

#[test]
fn a_dragged_footprint_moves_itself_with_no_ghost() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    drag(&mut app, Point::new(10.0, 11.0), Point::new(13.0, 11.0));
    let carry = app.interaction_state.pcb_carry.as_ref().expect("a carry");
    assert!(carry.footprints.contains(&R1));
    let delta = carry.delta;
    let canvas = &app.interaction_state.pcb_canvas;
    assert!(canvas.highlight().ghost.is_empty(), "no ghost");
    let base = canvas.active_renderer_snapshot().expect("a board");
    assert!(
        base.pads.is_empty() && base.texts.is_empty(),
        "nothing left"
    );
    let moving = canvas.drag_preview_texts();
    assert_eq!(moving.len(), 1, "its designator goes with it");
    assert_eq!(
        moving[0].position,
        [(10.0 + delta.x) as f32, (10.0 + delta.y) as f32]
    );
    let scene = canvas.drag_preview_scene().expect("a moving scene");
    assert_eq!(scene.top_polygons.len(), 2, "both pads are drawn");
}

#[test]
fn with_texts_filtered_out_a_designator_is_not_grabbed() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    let grabbed = |app: &mut Signex| {
        app.with_pcb_selection(|sel, board| sel.paint(board, None).1.len())
            .expect("a board")
    };
    let with_texts = grabbed(&mut app);
    app.ui_state.pcb_pick_filter.toggle(PickKind::Texts);
    assert_eq!(grabbed(&mut app), with_texts - 1);
}
