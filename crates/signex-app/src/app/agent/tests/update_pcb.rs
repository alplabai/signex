//! #2470: `update_pcb_from_schematic` on the agent channel names a sheet of
//! the project and is answered once its list is read, with the change list;
//! the person's tab does not move.

use serde_json::json;

use super::{app_in, call_settled, fixture};
use crate::app::Signex;

/// The fixture's project with an empty board named and open behind `b`.
fn app_with_board(dir: &std::path::Path) -> Signex {
    let mut app = app_in(dir);
    let board = signex_types::project::SetupProfile::Standard
        .pcb(4)
        .new_board();
    app.open_pcb_tab(dir.join("P.snxpcb"), "P.snxpcb".to_string(), board);
    app.document_state.projects[0].data.pcb_file = Some("P.snxpcb".to_string());
    let project = app.document_state.projects[0].id;
    for tab in &mut app.document_state.tabs {
        tab.project_id = Some(project);
    }
    fixture::activate(&mut app, &dir.join("b.snxsch"));
    app
}

#[test]
fn a_dry_run_on_the_channel_is_answered_with_the_change_list() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_board(dir.path());
    let active = app.document_state.active_tab;
    let doc = dir.path().join("a.snxsch").display().to_string();
    let reply = call_settled(
        &mut app,
        "update_pcb_from_schematic",
        json!({"document": doc, "dry_run": true}),
    );
    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(reply["data"]["dry_run"], true, "{reply}");
    let board = dir.path().join("P.snxpcb").display().to_string();
    assert_eq!(reply["data"]["board"], board, "{reply}");
    assert!(reply["data"]["changes"].is_array(), "{reply}");
    assert_eq!(reply["changed"], json!([]), "a dry run changes nothing");
    assert_eq!(app.document_state.active_tab, active, "the tab stays");
    assert!(app.ui_state.update_pcb.is_none(), "no dialog is left open");
    assert!(app.agent.waiting.is_empty(), "answered on the socket");
}

/// The channel keeps its own rule: its gate needs a named document, so the
/// only-board fallback of a script's call does not apply here, even with a
/// single board open.
#[test]
fn update_pcb_on_the_channel_needs_its_document() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_board(dir.path());
    let reply = super::call(
        &mut app,
        "update_pcb_from_schematic",
        json!({"dry_run": true}),
    );
    assert_eq!(reply["status"], "refused", "{reply}");
    let reason = reply["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("explicit `document`"), "{reply}");
    assert!(app.ui_state.update_pcb.is_none());
}
