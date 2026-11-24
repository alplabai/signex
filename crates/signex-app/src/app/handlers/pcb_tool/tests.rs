//! #2373 — the board placement tools through the app: armed by their
//! command, previewed on the cursor, each click one undo step, Esc and a
//! right-click to end.

use std::path::{Path, PathBuf};

use pcb_geom::ClickMode;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{ContextMenuMsg, EditMsg, Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::pcb_canvas::PcbCanvasMsg;

/// A 1 mm grid, a 0.8 mm via with a 0.4 mm drill, a 0.3 mm track width
/// and a net-3 track along y = 0 on the top layer.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.0, "trace_width": 0.3, "via_diameter": 0.8,
                  "via_drill": 0.4, "clearance": 0.2},
        "nets": [{"number": 3, "name": "GND"}],
        "segments": [{"uuid": Uuid::from_u128(2), "start": {"x": 0.0, "y": 0.0},
                      "end": {"x": 10.0, "y": 0.0}, "width": 0.5,
                      "layer": "TopCopper", "net": 3}]
    }))
    .unwrap()
}

pub(super) fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

/// #2879 turned every object kind and axis snapping on by default, with a
/// reach of at least 8 px (2.67 mm at this rig's 3 px/mm): a click near the
/// net-3 track lands on its line, off the grid. For a test about a tool,
/// not about the snaps, the grid alone holds the clicks.
pub(super) fn grid_only(app: &mut Signex) {
    app.ui_state.pcb_snap.objects.clear();
    app.ui_state.pcb_snap.to_axes = false;
}

pub(super) fn live(app: &Signex, path: &Path) -> String {
    serde_json::to_string(app.document_state.pcb_board(path).expect("board")).unwrap()
}

pub(super) fn board_of<'a>(app: &'a Signex, path: &Path) -> &'a PcbBoard {
    app.document_state.pcb_board(path).expect("board")
}

pub(super) fn run(app: &mut Signex, command: &str) {
    let id = AppCommandId::new(command).expect("a catalog id");
    let _ = app.update(Message::Menu(MenuMessage::Command(id)));
}

pub(super) fn hover(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::PointerMoved {
        at: Point::new(x, y),
        zoom_pct: 100.0,
        tolerance_mm: 0.05,
    }));
}

pub(super) fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
        at: Point::new(x, y),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }));
}

pub(super) fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

pub(super) fn escape(app: &mut Signex) {
    app.clear_pcb_selection();
}

pub(super) fn right_click(app: &mut Signex) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(5.0, 5.0)));
}

pub(super) fn has_ghost(app: &Signex) -> bool {
    app.interaction_state.pcb_canvas.has_part_ghost()
}

#[test]
fn via_places_on_the_grid_on_the_net_under_it_one_step_per_click() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    grid_only(&mut app);
    let before = live(&app, &path);
    run(&mut app, "place_board_via");
    assert!(app.interaction_state.pcb_tool.is_some(), "Via is armed");
    hover(&mut app, 4.2, 0.3);
    assert!(has_ghost(&app), "the via to place follows the pointer");
    assert_eq!(live(&app, &path), before, "hovering adds nothing");

    click(&mut app, 4.2, 0.3);
    click(&mut app, 7.4, 6.2);
    let vias = &board_of(&app, &path).vias;
    assert_eq!(
        vias.len(),
        2,
        "each click places a via, the tool stays armed"
    );
    assert_eq!(vias[0].position, Point::new(4.0, 0.0), "on the 1 mm grid");
    assert_eq!((vias[0].diameter, vias[0].drill), (0.8, 0.4));
    assert_eq!(vias[0].net, 3, "it joins the track it lands on");
    assert_eq!(vias[1].net, 0);

    undo(&mut app);
    assert_eq!(board_of(&app, &path).vias.len(), 1, "one undo per via");
    undo(&mut app);
    assert_eq!(live(&app, &path), before);
}

#[test]
fn esc_and_a_right_click_end_the_via_tool() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_via");
    escape(&mut app);
    assert!(app.interaction_state.pcb_tool.is_none(), "Esc ends it");
    hover(&mut app, 2.0, 2.0);
    assert!(!has_ghost(&app));

    run(&mut app, "place_board_via");
    right_click(&mut app);
    assert!(
        app.interaction_state.pcb_tool.is_none(),
        "a right-click ends it"
    );
    assert!(
        app.interaction_state.context_menu.is_none(),
        "the right-click does not open the board menu"
    );
    let before = live(&app, &path);
    click(&mut app, 2.0, 2.0);
    assert_eq!(live(&app, &path), before, "an ended tool places nothing");
}

#[test]
fn moving_a_footprint_ends_the_armed_tool() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    run(&mut app, "place_board_via");
    run(&mut app, "move_object");
    assert!(app.interaction_state.pcb_tool.is_none());
}
