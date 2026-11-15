//! #2373 — the String tool through the app: each click places a board
//! string on the active layer, one undo step each.

use signex_types::pcb::{PCB_DEFAULT_TEXT_SIZE_MM, Point};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

use super::tests::{app_with_board, board_of, click, escape, has_ghost, hover, live, run, undo};
use crate::app::{Message, PcbMsg, PcbViewMsg};

#[test]
fn a_string_lands_on_the_grid_on_the_active_layer_one_step_per_click() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    run(&mut app, "place_board_string");
    assert!(app.interaction_state.pcb_tool.is_some(), "String is armed");
    let overlay = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        overlay,
    ))));
    hover(&mut app, 4.2, 6.3);
    assert!(has_ghost(&app), "the string to place follows the pointer");
    assert_eq!(live(&app, &path), before, "hovering adds nothing");

    click(&mut app, 4.2, 6.3);
    click(&mut app, 7.4, 8.2);
    let texts = &board_of(&app, &path).texts;
    assert_eq!(
        texts.len(),
        2,
        "each click places one, the tool stays armed"
    );
    assert_eq!(texts[0].position, Point::new(4.0, 6.0), "on the 1 mm grid");
    assert_eq!(
        (texts[0].layer.as_str(), texts[0].text.as_str()),
        ("TopSilkscreen", "String")
    );
    assert_eq!(texts[0].font_size, PCB_DEFAULT_TEXT_SIZE_MM);
    assert_ne!(texts[0].uuid, texts[1].uuid);

    undo(&mut app);
    assert_eq!(board_of(&app, &path).texts.len(), 1, "one undo per string");
    undo(&mut app);
    assert_eq!(live(&app, &path), before);

    escape(&mut app);
    assert!(app.interaction_state.pcb_tool.is_none(), "Esc ends it");
}
