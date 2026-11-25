//! #2507: the dimension kinds on a board, driven through `Signex::update`.
//! Each kind's command arms its tool and a placement is one undo step; a
//! radial dimension follows its arc when the arc moves, in the same undo
//! step, and the board saves and reads the kind back.

use pcb_geom::ClickMode;
use signex_app::app::{EditMsg, FileMsg, Message, Signex};
use signex_app::diagnostics;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_engine::pcb::{BoardCommand, BoardItem};
use signex_types::format::{SNXPCB_FORMAT_V1_11, SnxPcb};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_dimension_kind::DimensionKind;
use uuid::Uuid;

const ARC: Uuid = Uuid::from_u128(0x601);

/// A half circle of radius 5 round (20, 20) on top copper.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.0, "trace_width": 0.3},
        "free": {
            "arcs": [{"uuid": ARC, "start": {"x": 25.0, "y": 20.0},
                      "mid": {"x": 20.0, "y": 25.0}, "end": {"x": 15.0, "y": 20.0},
                      "width": 0.3, "layer": "TopCopper", "net": 0}]
        }
    }))
    .unwrap()
}

struct Fixture {
    app: Signex,
    path: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}

fn open_board() -> Fixture {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    crate::drive::drive(&mut app, task);
    Fixture {
        app,
        path,
        _tmp: tmp,
    }
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

fn live(app: &Signex) -> PcbBoard {
    let docs = &app.document_state;
    docs.tab_board_engine(docs.active_tab)
        .map(|engine| engine.board())
        .expect("a board is open")
        .clone()
}

fn snapshot(app: &Signex) -> String {
    serde_json::to_string(&live(app)).unwrap()
}

fn run(app: &mut Signex, command: &str) {
    let id = AppCommandId::new(command).expect("a catalog id");
    send(app, Message::Menu(MenuMessage::Command(id)));
}

fn click(app: &mut Signex, x: f64, y: f64) {
    send(
        app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(x, y),
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6
}

#[test]
fn a_horizontal_and_a_vertical_dimension_measure_across_and_up_in_one_step_each() {
    let mut f = open_board();
    // The grid alone holds the clicks: this test is about the kinds, not
    // the object and axis snaps (#2879).
    f.app.ui_state.pcb_snap.objects.clear();
    f.app.ui_state.pcb_snap.to_axes = false;
    for (command, kind, value) in [
        (
            "place_board_dimension_horizontal",
            DimensionKind::Horizontal,
            "8.00mm",
        ),
        (
            "place_board_dimension_vertical",
            DimensionKind::Vertical,
            "5.00mm",
        ),
    ] {
        let before = snapshot(&f.app);
        run(&mut f.app, command);
        // From (50, 50) to (58, 55): 8 mm across, 5 mm up or down.
        click(&mut f.app, 50.0, 50.0);
        click(&mut f.app, 58.0, 55.0);
        click(&mut f.app, 54.0, 62.0);
        let placed = live(&f.app).free.dimensions;
        assert_eq!(placed.len(), 1, "{command}");
        assert_eq!(placed[0].kind, kind);
        assert_eq!(placed[0].value_text(), value, "{command}");
        send(&mut f.app, Message::Edit(EditMsg::Undo));
        assert_eq!(snapshot(&f.app), before, "{command}: one undo step");
    }
}

#[test]
fn a_radial_dimension_measures_the_arc_clicked_and_follows_it() {
    let mut f = open_board();
    let before = snapshot(&f.app);
    run(&mut f.app, "place_board_dimension_radial");
    click(&mut f.app, 20.0, 25.0);
    let placed = live(&f.app).free.dimensions;
    assert_eq!(placed.len(), 1, "one click places it");
    assert_eq!(placed[0].kind, DimensionKind::Radial);
    assert_eq!(placed[0].measures, Some(ARC));
    assert_eq!(placed[0].value_text(), "R5.00mm");
    let placed_state = snapshot(&f.app);

    // The arc moves 10 mm right: the dimension follows, in the same step.
    let mut moved = live(&f.app).free.arcs[0].clone();
    for p in [&mut moved.start, &mut moved.mid, &mut moved.end] {
        p.x += 10.0;
    }
    let path = f.path.clone();
    f.app
        .execute_board_commands(
            &path,
            vec![BoardCommand::ReplaceItems {
                items: vec![BoardItem::Arc(moved)],
            }],
            None,
        )
        .expect("the arc moves");
    let followed = &live(&f.app).free.dimensions[0];
    assert!(
        close(followed.start, Point::new(30.0, 20.0)),
        "{followed:?}"
    );
    assert!(close(followed.end, Point::new(30.0, 25.0)), "{followed:?}");
    assert_eq!(followed.value_text(), "R5.00mm");
    send(&mut f.app, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f.app), placed_state, "one undo puts both back");
    send(&mut f.app, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f.app), before, "the placement was one step");
}

#[test]
fn a_radial_dimension_is_saved_with_its_kind_and_read_back() {
    let mut f = open_board();
    run(&mut f.app, "place_board_dimension_radial");
    click(&mut f.app, 20.0, 25.0);
    send(&mut f.app, Message::File(FileMsg::Save));
    let text = std::fs::read_to_string(&f.path).unwrap();
    assert!(
        text.starts_with(&format!("format = \"{SNXPCB_FORMAT_V1_11}\"")),
        "{text}"
    );
    let saved = SnxPcb::parse(&text).unwrap().board;
    let dimension = &saved.free.dimensions[0];
    assert_eq!(dimension.kind, DimensionKind::Radial);
    assert_eq!(dimension.measures, Some(ARC));
    assert_eq!(dimension.value_text(), "R5.00mm");
}
