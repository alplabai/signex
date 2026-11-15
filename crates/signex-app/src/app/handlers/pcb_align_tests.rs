//! #2373 — Align on a board through the PCB Active Bar's Align dropdown:
//! the selected footprints line up by their origins as one undo step.

use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, PcbBarMenu, PcbBarMsg, PcbMsg};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

fn board() -> PcbBoard {
    let fp = |n: u128, x: f64, y: f64| {
        serde_json::json!({"uuid": Uuid::from_u128(n), "reference": format!("U{n}"),
                           "position": {"x": x, "y": y}, "layer": "TopCopper",
                           "pads": [{"uuid": Uuid::from_u128(n + 100), "number": "1",
                                     "pad_type": "smd", "shape": "rect",
                                     "position": {"x": 0.0, "y": 0.0},
                                     "size": {"x": 1.0, "y": 1.0}, "drill": null,
                                     "layers": ["TopCopper"]}]})
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "footprints": [fp(0x10, 2.0, 3.0), fp(0x20, 8.0, 9.0)]
    }))
    .unwrap()
}

#[test]
fn the_align_dropdown_lines_the_selected_footprints_up_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    let live = |app: &crate::app::Signex| {
        serde_json::to_string(app.document_state.pcb_board(&path).unwrap()).unwrap()
    };
    let before = live(&app);
    let select_all = AppCommandId::new("select_all").unwrap();
    let _ = app.update(Message::Menu(MenuMessage::Command(select_all)));

    let bar = |msg| Message::Pcb(PcbMsg::Bar(msg));
    let _ = app.update(bar(PcbBarMsg::MenuToggled(PcbBarMenu::Align)));
    assert_eq!(app.interaction_state.pcb_bar_menu, Some(PcbBarMenu::Align));
    let left = AppCommandId::new("align_left").unwrap();
    let _ = app.update(bar(PcbBarMsg::CommandPicked(left)));
    let positions: Vec<Point> = app
        .document_state
        .pcb_board(&path)
        .unwrap()
        .footprints
        .iter()
        .map(|f| f.position)
        .collect();
    assert_eq!(positions, [Point::new(2.0, 3.0), Point::new(2.0, 9.0)]);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(live(&app), before, "one undo step");
}
