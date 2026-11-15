//! #2373 — the String tool through the app. Place ▸ String stays greyed
//! until board text is drawn and picked (#2387), so these arm it directly:
//! each click places a board string on the active overlay layer, one undo
//! step each, and a click on any other layer places nothing.

use std::path::Path;

use signex_types::pcb::{PCB_DEFAULT_TEXT_SIZE_MM, Point};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::BoardToolSession;
use super::tests::{app_with_board, board_of, click, escape, hover, live, undo};
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
    });
}

fn pick_layer(app: &mut Signex, layer: PcbLayer) {
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        layer,
    ))));
}

#[test]
fn the_string_tool_stays_greyed_until_board_text_is_drawn() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    assert_eq!(BoardPlaceTool::TextString.command(), None);
    app.arm_board_tool(BoardPlaceTool::TextString);
    assert!(app.interaction_state.pcb_tool.is_none(), "nothing arms it");
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
fn a_string_click_off_the_overlay_layers_places_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    arm_string(&mut app, &path);
    for layer in [
        PcbLayer::Copper(CopperLayer::Top),
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
