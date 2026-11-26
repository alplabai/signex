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

/// The kept panes go to a recorder, never to the test's `prefs.json`.
#[derive(Default)]
struct PanesRecorder;

impl crate::document_panes::DocumentPanesStore for PanesRecorder {
    fn write(&self, _value: &Value) -> Result<(), String> {
        Ok(())
    }
}

fn app_with_recorded_panes() -> Signex {
    let mut app = app_at_work();
    app.ui_state.workspace_layouts.panes.store = std::sync::Arc::new(PanesRecorder);
    app
}

/// Mockup row 207: Split Right moves the active document into a new pane on
/// the right, which has the focus; the other document stays on the left.
#[test]
fn split_right_moves_the_active_document_into_a_new_pane() {
    let mut app = app_with_recorded_panes();
    let active = app.document_state.active_tab;
    let reply = call(&mut app, "split_pane_right", json!({}));
    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(reply["data"]["layout"], "side_by_side");
    assert_eq!(
        panes(&reply),
        [(json!("a"), json!(false)), (json!("b"), json!(true))]
    );
    assert_eq!(app.document_state.active_tab, active, "the tab stays");
    assert!(app.ui_state.workspace_layouts.panes.split.is_some());
}

/// Open Second View shows the active document in a second pane as well; a
/// split across the panes is refused with its reason, and Untile merges.
#[test]
fn a_second_view_shows_the_document_twice_and_a_crosswise_split_is_refused() {
    let mut app = app_with_recorded_panes();
    let reply = call(&mut app, "open_second_view", json!({}));
    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(
        panes(&reply),
        [(json!("b"), json!(false)), (json!("b"), json!(true))]
    );

    let across = call(&mut app, "split_pane_down", json!({}));
    assert_eq!(across["status"], "refused", "{across}");
    assert!(
        across["reason"]
            .as_str()
            .is_some_and(|why| why.contains("run the other way")),
        "{across}"
    );

    let merged = call(&mut app, "untile_documents", json!({}));
    assert_eq!(merged["data"]["tiled"], false, "{merged}");
    assert!(app.ui_state.workspace_layouts.panes.split.is_none());
}
