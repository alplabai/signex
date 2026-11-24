//! #2373 — the String tool through the app: `place_board_string` arms it;
//! each click places a board string on the active copper (#2883), overlay,
//! mechanical or documentation layer, one undo step each, and a click on
//! any other layer places nothing.

use std::path::Path;

use signex_types::pcb::{PCB_DEFAULT_TEXT_SIZE_MM, Point};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

use super::BoardToolSession;
use super::tests::{app_with_board, board_of, click, escape, hover, live, run, undo};
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::{Message, PcbMsg, PcbViewMsg, Signex};

fn arm_string(app: &mut Signex, path: &Path) {
    app.interaction_state.pcb_tool = Some(BoardToolSession {
        tool: BoardPlaceTool::TextString,
        path: path.to_path_buf(),
        window: app.interaction_state.acting_window,
        points: Vec::new(),
        cursor: None,
        route: None,
        router: None,
        paused: None,
        note: None,
    });
}

fn pick_layer(app: &mut Signex, layer: PcbLayer) {
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        layer,
    ))));
}

#[test]
fn place_board_string_arms_the_string_tool_and_a_mechanical_layer_takes_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    assert_eq!(
        BoardPlaceTool::TextString.command(),
        Some("place_board_string")
    );
    run(&mut app, "place_board_string");
    let armed = app.interaction_state.pcb_tool.as_ref().map(|s| s.tool);
    assert_eq!(armed, Some(BoardPlaceTool::TextString));
    let mech = signex_types::pcb_layer::MechanicalLayer::new(1).unwrap();
    pick_layer(&mut app, PcbLayer::Mechanical(mech));
    click(&mut app, 4.2, 6.3);
    let texts = &board_of(&app, &path).texts;
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0].layer, "Mechanical1");
}

#[test]
fn a_string_lands_on_the_grid_on_the_active_overlay_one_step_per_click() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    let overlay = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    pick_layer(&mut app, overlay);
    arm_string(&mut app, &path);
    hover(&mut app, 4.2, 6.3);
    let preview = app.board_tool_preview().expect("a preview on the cursor");
    assert_eq!(
        preview.texts.len(),
        1,
        "the string to place follows the pointer"
    );
    assert_eq!(preview.texts[0].position, Point::new(4.0, 6.0));
    assert_eq!(preview.texts[0].layer, "TopSilkscreen");
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

#[test]
fn a_string_click_on_a_layer_that_takes_no_text_places_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    arm_string(&mut app, &path);
    // Top copper was in this list; it takes a string since Caner's
    // 2026-11-24 decision (#2883), so a solder mask stands in for it.
    for layer in [
        PcbLayer::Technical(TechnicalLayer::SolderMaskTop),
        PcbLayer::Technical(TechnicalLayer::EdgeCuts),
    ] {
        pick_layer(&mut app, layer);
        hover(&mut app, 4.2, 6.3);
        let preview = app.board_tool_preview().expect("the tool is armed");
        assert!(preview.texts.is_empty(), "no string follows on {layer:?}");
        click(&mut app, 4.2, 6.3);
        assert_eq!(live(&app, &path), before, "nothing placed on {layer:?}");
    }
    assert!(
        app.interaction_state.pcb_tool.is_some(),
        "the tool stays armed"
    );
}
