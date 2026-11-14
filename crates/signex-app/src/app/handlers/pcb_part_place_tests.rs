//! #2364 — with a part on the cursor and nothing selected, the turn and
//! flip commands are live on the board and act on the part.

use signex_engine::pcb::part_place::PartToPlace;
use signex_types::pcb::{Footprint, PcbBoard};
use uuid::Uuid;

use crate::app::command::enablement::is_enabled;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message};
use crate::keymap::AppCommandId;
use crate::pcb_canvas::PcbCarry;

fn template() -> Footprint {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(0xf0), "position": {"x": 0.0, "y": 0.0},
        "layer": "TopCopper",
        "pads": [{"uuid": Uuid::from_u128(0xf1), "number": "1", "pad_type": "smd",
                  "shape": "rect", "position": {"x": 1.0, "y": 0.0},
                  "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"]}]
    }))
    .unwrap()
}

#[test]
fn turn_and_flip_are_live_for_the_part_on_the_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    let board: PcbBoard =
        serde_json::from_value(serde_json::json!({"uuid": Uuid::from_u128(1)})).unwrap();
    app.open_pcb_tab(path.clone(), "board".into(), board);
    let enabled = |app: &crate::app::Signex, id: &str| {
        is_enabled(&AppCommandId::new(id).unwrap(), &app.command_context(None))
    };
    assert!(!enabled(&app, "rotate_clockwise"), "nothing to turn yet");

    let part = PartToPlace::new(template(), "U", "").unwrap();
    app.interaction_state.pcb_carry = Some(PcbCarry::place(path, None, part));
    assert!(enabled(&app, "rotate_clockwise"));
    assert!(enabled(&app, "flip_selection"));

    let _ = app.update(Message::Edit(EditMsg::FlipSelection));
    let flipped = app
        .interaction_state
        .pcb_carry
        .as_ref()
        .and_then(|c| c.part.as_deref())
        .is_some_and(PartToPlace::is_bottom);
    assert!(
        flipped,
        "Flip Selection flips the part, not the empty selection"
    );
}
