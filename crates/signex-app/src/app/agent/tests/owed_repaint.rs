//! #2445: an agent's command run as one labelled step (`run_as_one_step`)
//! repaints the sheet once, with only what the command changed.

use serde_json::json;

use super::{app_at_work, call_as, path};
use crate::app::edit_tail_tests::{cached_wire_end, stale_wire_in_cache};
use crate::app::handlers::editing_commands::net_color_driver as fixture;

#[test]
fn an_agents_one_step_command_repaints_only_what_it_changed() {
    let mut app = app_at_work();
    fixture::activate(&mut app, &path("a.snxsch"));
    let stale = stale_wire_in_cache(&mut app, "a.snxsch");

    let reply = call_as(
        &mut app,
        7772,
        "color_net",
        json!({"document": path("a.snxsch").display().to_string(), "at": [5, 0], "color": "#336699"}),
    );

    assert_eq!(reply["status"], "ran", "{reply}");
    let engine = &app.document_state.engines[&path("a.snxsch")];
    let cache = app.interaction_state.canvas.render_cache.as_ref().unwrap();
    assert!(!engine.document().net_colors.is_empty());
    assert_eq!(
        cache.snapshot().net_colors,
        engine.document().net_colors,
        "the new colours are shown"
    );
    assert_eq!(cached_wire_end(&app), stale, "the wires were not re-cloned");
}
