//! #1671 - `refresh_view` redraws the active canvas: every cache it draws
//! from is dropped, so the next frame is drawn from the model again.

use std::path::PathBuf;

use crate::app::command::CommandArgs;
use crate::app::{Signex, TabInfo, TabKind};
use crate::keymap::AppCommandId;

fn app_on_a_sheet() -> Signex {
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("redraw.snxsch");
    let engine =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "redraw".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    app
}

#[test]
fn refresh_view_drops_every_cache_of_the_active_sheet_canvas() {
    let mut app = app_on_a_sheet();
    assert!(app.has_active_schematic(), "precondition: a sheet is shown");
    let canvas = app.interaction_state.active_canvas();
    let before = (
        canvas.bg_cache_clears,
        canvas.content_cache_clears,
        canvas.overlay_cache_clears,
    );

    let id = AppCommandId::new("refresh_view").unwrap();
    let _ = app.dispatch_command(&id, CommandArgs::none(), None);

    let canvas = app.interaction_state.active_canvas();
    assert!(canvas.bg_cache_clears > before.0, "paper and grid redraw");
    assert!(canvas.content_cache_clears > before.1, "objects redraw");
    assert!(canvas.overlay_cache_clears > before.2, "overlay redraws");
}
