//! #2552 on the agent channel: `set_object_position` reaches an agent only
//! inside `apply_edits`, whose transaction holds the engine's notices until
//! it commits; the move's answer still names the contact it could not keep.

use serde_json::json;

use super::{app_in, call, sheet_entry};
use crate::app::command::placed_move_tests::blocked_resistor;
use crate::app::handlers::editing_commands::net_color_driver as fixture;

#[test]
fn an_agent_move_inside_apply_edits_names_the_contact_it_could_not_keep() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path());
    let person = app.document_state.active_path.clone().expect("a sheet");
    let (sheet, r1) = blocked_resistor();
    let file = dir.path().join("c.snxsch");
    fixture::add_sheet_with(&mut app, &file, sheet);
    app.document_state.projects[0]
        .data
        .sheets
        .push(sheet_entry("c"));
    app.document_state.mark_document_clean(&file);
    fixture::activate(&mut app, &person);
    let started = app.document_state.engines[&file].revision();

    let applied = call(
        &mut app,
        "apply_edits",
        json!({
            "document": file.display().to_string(),
            "expect_revision": started,
            "edits": [{"id": "set_object_position", "args": {"id": r1.to_string(), "at": [50, 60]}}],
        }),
    );

    assert_eq!(applied["status"], "answered", "{applied}");
    let answer = &applied["data"]["results"][0];
    let fallbacks = answer["fallbacks"].as_array().expect("a list");
    assert_eq!(fallbacks.len(), 1, "{applied}");
    assert_eq!(fallbacks[0]["kind"], "pins_not_kept");
    assert_eq!(fallbacks[0]["names"], json!(["R1 pin 1"]));
    let messages = applied["messages"].to_string();
    assert!(
        messages.contains("R1 pin 1"),
        "still in Messages: {messages}"
    );
}
