//! #2249: Tile Horizontally, Tile Vertically and Untile on the agent
//! channel. Each names no document, runs the menu row's handler and answers
//! with the main window's layout and pane order; the active tab stays.

use serde_json::{Value, json};

use super::{app_at_work, call};
use crate::app::{Signex, TileLayout};

/// The panes of a reply as (name, focused).
fn panes(reply: &Value) -> Vec<(Value, Value)> {
    reply["data"]["panes"]
        .as_array()
        .expect("a pane list")
        .iter()
        .map(|pane| (pane["name"].clone(), pane["focused"].clone()))
        .collect()
}

fn tiled_by_agent(id: &str) -> (Signex, Value) {
    let mut app = app_at_work();
    let active = app.document_state.active_tab;
    let reply = call(&mut app, id, json!({}));
    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(app.document_state.active_tab, active, "the tab stays");
    (app, reply)
}

#[test]
fn tile_horizontally_answers_with_the_stacked_panes() {
    let (app, reply) = tiled_by_agent("tile_windows_horizontally");
    assert_eq!(reply["data"]["tiled"], true);
    assert_eq!(reply["data"]["layout"], "stacked");
    assert_eq!(
        panes(&reply),
        [(json!("a"), json!(false)), (json!("b"), json!(true))]
    );
    assert_eq!(app.ui_state.document_tiling, Some(TileLayout::Stacked));
}

#[test]
fn tile_vertically_answers_with_the_side_by_side_panes() {
    let (app, reply) = tiled_by_agent("tile_windows_vertically");
    assert_eq!(reply["data"]["layout"], "side_by_side");
    assert_eq!(
        panes(&reply),
        [(json!("a"), json!(false)), (json!("b"), json!(true))]
    );
    assert!(reply["data"]["panes"][0]["document"].is_string(), "{reply}");
    assert_eq!(app.ui_state.document_tiling, Some(TileLayout::SideBySide));
}

/// Untile answers with no panes; with nothing tiled it is
/// refused with the menu row's reason.
#[test]
fn untile_answers_with_no_panes_and_is_refused_when_not_tiled() {
    let (mut app, _) = tiled_by_agent("tile_windows_vertically");
    let reply = call(&mut app, "untile_documents", json!({}));
    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(
        reply["data"],
        json!({ "tiled": false, "layout": null, "panes": [] })
    );
    assert_eq!(app.ui_state.document_tiling, None);

    let again = call(&mut app, "untile_documents", json!({}));
    assert_eq!(again["status"], "refused", "{again}");
    assert!(
        again["reason"]
            .as_str()
            .is_some_and(|why| why.contains("not tiled")),
        "{again}"
    );
}
