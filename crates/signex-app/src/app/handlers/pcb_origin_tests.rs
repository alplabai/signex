//! #2384 — a new board counts from the bottom-left of its area, Y up, and
//! the canvas marker, the status bar and the Component view's X and Y
//! follow that origin, through `Signex::update`.

use std::path::{Path, PathBuf};

use pcb_geom::{ClickMode, ItemRef};
use signex_engine::pcb::component::LocationField;
use signex_types::pcb::{Footprint, PcbBoard, Point};
use signex_types::project::SetupProfile;
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::panels::{ComponentField, PanelMsg, PcbComponentMsg, Shared};
use crate::pcb_canvas::PcbCanvasMsg;

const U1: Uuid = Uuid::from_u128(0x10);

/// A new board's blank, with U1 placed 20 mm right of and 20 mm above its
/// origin. Its two pads leave its centre bare, so a click there takes the
/// body: a pad is picked over the body under it.
fn new_board() -> PcbBoard {
    let mut board = SetupProfile::Standard.pcb(2).new_board();
    let fp: Footprint = serde_json::from_value(serde_json::json!({
        "uuid": U1, "reference": "U1", "position": {"x": 20.0, "y": 60.0},
        "layer": "TopCopper",
        "pads": [{"uuid": Uuid::from_u128(0x11), "number": "1", "pad_type": "smd",
                  "shape": "rect", "position": {"x": -2.0, "y": 0.0},
                  "size": {"x": 1.0, "y": 1.0}, "layers": ["TopCopper"]},
                 {"uuid": Uuid::from_u128(0x12), "number": "2", "pad_type": "smd",
                  "shape": "rect", "position": {"x": 2.0, "y": 0.0},
                  "size": {"x": 1.0, "y": 1.0}, "layers": ["TopCopper"]}]
    }))
    .unwrap();
    board.footprints.push(fp);
    board
}

fn open(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.unit = signex_types::coord::Unit::Mm;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), new_board());
    (app, path)
}

fn component(app: &mut Signex, msg: PcbComponentMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbComponent(
        msg,
    ))));
}

fn type_location(app: &mut Signex, path: &Path, field: LocationField, text: &str) {
    let field = ComponentField::Location(field);
    component(
        app,
        PcbComponentMsg::FieldTyped {
            board: path.to_path_buf(),
            footprints: vec![U1],
            field: field.clone(),
            text: text.into(),
        },
    );
    component(
        app,
        PcbComponentMsg::FieldSubmitted {
            board: path.to_path_buf(),
            footprints: vec![U1],
            field,
        },
    );
}

fn u1(app: &Signex, path: &Path) -> Point {
    let board = app.document_state.pcb_board(path).expect("board");
    board
        .footprints
        .iter()
        .find(|f| f.uuid == U1)
        .unwrap()
        .position
}

#[test]
fn a_new_board_counts_from_the_bottom_left_of_its_area_with_y_up() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = open(dir.path());
    let board = app.document_state.pcb_board(&path).expect("board");
    let corner = signex_types::pcb_origin::board_area_bottom_left(&board.outline);
    assert_eq!(board.options.origin.at, corner);
    assert_eq!(
        app.interaction_state.pcb_canvas.board_marks().origin,
        [0.0, 80.0]
    );

    // The status bar reads the cursor from the origin, Y up.
    app.ui_state.cursor_x = 5.0;
    app.ui_state.cursor_y = 75.0;
    let window = app.ui_state.main_window_id.unwrap();
    assert_eq!(app.status_cursor(window), (5.0, 5.0));

    // Properties shows U1 20 mm right of and above the origin.
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
        at: Point::new(20.0, 60.0),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }));
    let selected: Vec<ItemRef> = app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert_eq!(selected, vec![ItemRef::Footprint(U1)]);
    let facts = app
        .pcb_component_facts(&path, app.document_state.pcb_board(&path).unwrap())
        .expect("the Component view");
    assert_eq!((facts.x, facts.y), (Shared::Same(20.0), Shared::Same(20.0)));

    // Placed at (5, 5): 5 mm right of and 5 mm above the corner.
    type_location(&mut app, &path, LocationField::X, "5");
    type_location(&mut app, &path, LocationField::Y, "5");
    assert_eq!(u1(&app, &path), Point::new(5.0, 75.0));
}
