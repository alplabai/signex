//! #1671 - `refresh_view` clears the caches of the active sheet or board
//! canvas, so its next frame is drawn again.

use std::path::PathBuf;

use crate::app::command::CommandArgs;
use crate::app::{FileMsg, Message, Signex, TabInfo, TabKind};
use crate::keymap::AppCommandId;

fn refresh(app: &mut Signex) {
    let id = AppCommandId::new("refresh_view").unwrap();
    let _ = app.dispatch_command(&id, CommandArgs::none(), None);
}

fn app_on_a_sheet() -> Signex {
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("redraw.snxsch");
    let engine =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "redraw".to_string(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    app
}

fn empty_board() -> signex_types::pcb::PcbBoard {
    signex_types::pcb::PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        free: Default::default(),
        options: Default::default(),
        unknown: Default::default(),
        uuid: uuid::Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}

#[test]
fn refresh_view_clears_the_active_sheet_canvas_caches() {
    let mut app = app_on_a_sheet();
    assert!(app.has_active_schematic(), "precondition: a sheet is shown");
    let canvas = app.interaction_state.active_canvas();
    let before = (
        canvas.bg_cache_clears,
        canvas.content_cache_clears,
        canvas.overlay_cache_clears,
    );

    refresh(&mut app);

    let canvas = app.interaction_state.active_canvas();
    assert!(canvas.bg_cache_clears > before.0, "background cache");
    assert!(canvas.content_cache_clears > before.1, "content cache");
    assert!(canvas.overlay_cache_clears > before.2, "overlay cache");
}

#[test]
fn refresh_view_clears_the_active_board_canvas_content_cache() {
    let (mut app, _boot) = Signex::new();
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: PathBuf::from("/w/redraw.snxpcb"),
        title: "board".to_string(),
        result: Ok(Box::new(empty_board())),
    }));
    assert!(app.has_active_pcb(), "precondition: a board is shown");
    let before = app.interaction_state.pcb_canvas.scene_generation();

    refresh(&mut app);

    assert_ne!(
        app.interaction_state.pcb_canvas.scene_generation(),
        before,
        "the board's content (and GPU scene) cache was not cleared"
    );
}
