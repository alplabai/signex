//! #2178: an agent's `apply_edits` is one labelled undo step, all or nothing,
//! checked against the document's revision, and `revert` takes back only that
//! step. None of it touches what a person is doing. The tests run command ids
//! with operands, as a headless run does.

use serde_json::{Value, json};
use signex_types::schematic::{Point, SelectedItem, SelectedKind};

use super::interaction_snapshot_tests::interaction_snapshot;
use super::queries::tests::{WIRE_A, app_with, path, uid};
use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::editing_commands::net_color_driver as fixture;
use crate::keymap::AppCommandId;

const A: u128 = 100;
const B: u128 = 101;

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a valid id")
}

fn run(app: &mut Signex, name: &str, operands: &[String]) -> CommandOutcome {
    let command = id(name);
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&command, &refs).expect("operands parse");
    app.run_operands(&command, &args).0
}

fn revision(app: &Signex, file: &str) -> u64 {
    app.document_state.engines[&path(file)].revision()
}

/// `apply_edits` on `file` at its current revision.
fn apply(app: &mut Signex, file: &str, edits: Value) -> CommandOutcome {
    let current = revision(app, file);
    apply_at(app, file, current, edits)
}

fn apply_at(app: &mut Signex, file: &str, expected: u64, edits: Value) -> CommandOutcome {
    run(
        app,
        "apply_edits",
        &[
            format!("document={}", path(file).display()),
            format!("expect_revision={expected}"),
            format!("edits={edits}"),
        ],
    )
}

fn applied(app: &mut Signex, file: &str, edits: Value) -> Value {
    match apply(app, file, edits) {
        CommandOutcome::Answered(value) => value,
        other => panic!("apply_edits was not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

fn revert(app: &mut Signex, file: &str, group: u64) -> CommandOutcome {
    run(
        app,
        "revert",
        &[
            format!("document={}", path(file).display()),
            format!("group={group}"),
        ],
    )
}

/// Three edits that each add something away from the fixture's wires.
fn three_edits() -> Value {
    json!([
        {"id": "place_junction", "args": {"at": [20, 20]}},
        {"id": "place_no_connect", "args": {"at": [22, 22]}},
        {"id": "place_net_label", "args": {"at": [24, 24], "text": "AGENT"}},
    ])
}

/// What a sheet holds that three_edits add, and its place on the timeline.
fn shape(app: &Signex, file: &str) -> String {
    let engine = &app.document_state.engines[&path(file)];
    let sheet = engine.document();
    let at = |p: &Point| format!("{:.3},{:.3}", p.x, p.y);
    let mut parts: Vec<String> = Vec::new();
    parts.extend(
        sheet
            .junctions
            .iter()
            .map(|j| format!("J {}", at(&j.position))),
    );
    parts.extend(
        sheet
            .no_connects
            .iter()
            .map(|n| format!("NC {}", at(&n.position))),
    );
    parts.extend(
        sheet
            .labels
            .iter()
            .map(|l| format!("L {} {}", l.text, at(&l.position))),
    );
    parts.extend(
        sheet
            .wires
            .iter()
            .map(|w| format!("W {} {}", at(&w.start), at(&w.end))),
    );
    parts.sort();
    format!("{parts:?}")
}

fn state(app: &Signex, file: &str) -> (u64, usize, bool, String) {
    let engine = &app.document_state.engines[&path(file)];
    (
        engine.revision(),
        engine.undo_position(),
        app.document_state.is_document_dirty(&path(file)),
        shape(app, file),
    )
}

/// Two clean sheets, `b` the active tab, with the person at work on it: a
/// wire selected, the wire tool, a wire in progress, a move armed.
fn app_at_work() -> Signex {
    let mut app = app_with(&[("a.snxsch", A), ("b.snxsch", B)]);
    app.ui_state.snap_enabled = false;
    fixture::activate(&mut app, &path("b.snxsch"));
    let canvas = app.interaction_state.active_canvas_mut();
    canvas.selected = vec![SelectedItem::new(uid(WIRE_A), SelectedKind::Wire)];
    canvas.wire_preview = vec![Point::new(1.0, 2.0)];
    canvas.drawing_mode = true;
    app.interaction_state.current_tool = crate::app::Tool::Wire;
    app.ui_state.move_pick = Some(crate::canvas::MovePick::Selection);
    app.interaction_state.canvas.move_pick = Some(crate::canvas::MovePick::Selection);
    app
}

/// [`app_at_work`] with nothing in the middle: a wire selected and the wire
/// tool, no drawing and no move armed.
fn app_idle() -> Signex {
    let mut app = app_at_work();
    let canvas = app.interaction_state.active_canvas_mut();
    canvas.wire_preview.clear();
    canvas.drawing_mode = false;
    canvas.move_pick = None;
    app.ui_state.move_pick = None;
    app
}

/// #2199: a wire edit answers its report, which `apply_edits` carries in
/// `results` by edit index (`null` for an edit that only ran); a dry run among
/// the edits changes nothing, and a refused wire refuses the whole list.
#[test]
fn a_wire_edit_answers_its_report_in_results_and_a_refused_one_rolls_the_list_back() {
    let mut app = app_idle();
    let edits = json!([
        {"id": "place_junction", "args": {"at": [20, 20]}},
        {"id": "place_wire", "args": {"start": [26, 26], "end": [28, 26], "allow_open": true}},
        {"id": "place_wire", "args": {"start": [26, 40], "end": [28, 40], "allow_open": true, "dry_run": true}},
    ]);
    let before = shape(&app, "b.snxsch");
    let answer = applied(&mut app, "b.snxsch", edits);
    let results = answer["results"].as_array().expect("results");
    assert_eq!(results.len(), 3);
    assert_eq!(results[0], Value::Null);
    assert_eq!(results[1]["dry_run"], false);
    assert_eq!(results[1]["segments"], json!([[26.0, 26.0, 28.0, 26.0]]));
    assert_eq!(results[2]["dry_run"], true);
    let after = shape(&app, "b.snxsch");
    assert!(after.contains("W 26.000,26.000 28.000,26.000"), "{after}");
    assert!(
        !after.contains("W 26.000,40.000"),
        "the dry run placed nothing: {after}"
    );
    assert_ne!(before, after);

    // An open end without `allow_open` refuses the list; nothing of it stays.
    let kept = state(&app, "b.snxsch");
    let refused = refusal(apply(
        &mut app,
        "b.snxsch",
        json!([
            {"id": "place_junction", "args": {"at": [30, 30]}},
            {"id": "place_wire", "args": {"start": [34, 34], "end": [36, 34]}},
        ]),
    ));
    assert!(refused.contains("edit 2 (`place_wire`)"), "{refused}");
    assert!(refused.contains("nothing to connect to"), "{refused}");
    assert_eq!(state(&app, "b.snxsch"), kept);
}

#[test]
fn one_undo_reverts_exactly_the_batch() {
    let mut app = app_at_work();
    let before = state(&app, "b.snxsch");
    let answer = applied(&mut app, "b.snxsch", three_edits());
    assert_eq!(answer["changed"], true);
    assert_eq!(answer["edits"], 3);
    let group = answer["group"].as_u64().expect("a group id");
    let engine = &app.document_state.engines[&path("b.snxsch")];
    assert_eq!(engine.undo_position(), before.1 + 1, "one step, not three");
    assert_eq!(engine.next_undo_group().map(|g| g.get()), Some(group));
    assert_eq!(answer["revision"], engine.revision());
    assert!(app.document_state.is_document_dirty(&path("b.snxsch")));
    assert!(shape(&app, "b.snxsch").contains("AGENT"));

    // A person's Ctrl+Z on that sheet takes the whole batch, and Messages
    // says which step it was.
    let mark = crate::diagnostics::newest_entry_id();
    assert!(app.apply_engine_undo(false));
    let said: Vec<String> = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|entry| entry.message)
        .collect();
    assert!(
        said.iter()
            .any(|m| m.starts_with("Undone: Agent: place_junction")),
        "{said:?}"
    );
    assert_eq!(shape(&app, "b.snxsch"), before.3);
    assert_eq!(
        app.document_state.engines[&path("b.snxsch")].undo_position(),
        before.1
    );
    assert!(!app.document_state.is_document_dirty(&path("b.snxsch")));
    // And Ctrl+Y brings it back as one.
    assert!(app.apply_engine_redo(false));
    assert!(shape(&app, "b.snxsch").contains("AGENT"));
}

#[test]
fn the_undo_step_is_labelled_with_the_agents_summary_or_its_edit_ids() {
    let mut app = app_at_work();
    let first = applied(&mut app, "b.snxsch", three_edits());
    assert_eq!(
        first["label"],
        "Agent: place_junction, place_no_connect, place_net_label"
    );
    let current = revision(&app, "b.snxsch");
    let labelled = run(
        &mut app,
        "apply_edits",
        &[
            format!("document={}", path("b.snxsch").display()),
            format!("expect_revision={current}"),
            r#"edits=[{"id":"place_junction","args":{"at":[30,30]}}]"#.to_string(),
            "summary=tie off the spare pin".to_string(),
        ],
    );
    let CommandOutcome::Answered(second) = labelled else {
        panic!("{labelled:?}");
    };
    assert_eq!(second["label"], "Agent: tie off the spare pin");
    let group = second["group"].as_u64().unwrap();
    let labels = &app.document_state.agent_edits.labels;
    assert_eq!(
        labels
            .get(&signex_engine::UndoGroupId::new(group))
            .map(String::as_str),
        Some("Agent: tie off the spare pin")
    );
    assert_eq!(labels.len(), 2);
}

#[test]
fn a_failing_edit_in_the_middle_restores_the_sheet_and_its_dirty_flag() {
    let mut app = app_at_work();
    // First clean, then dirty: both flags must come back.
    for dirty_first in [false, true] {
        if dirty_first {
            applied(
                &mut app,
                "b.snxsch",
                json!([{"id": "place_junction", "args": {"at": [40, 40]}}]),
            );
        }
        let before = state(&app, "b.snxsch");
        let people = interaction_snapshot(&app);
        let reason = refusal(apply(
            &mut app,
            "b.snxsch",
            json!([
                {"id": "place_junction", "args": {"at": [20, 20]}},
                {"id": "place_net_label", "args": {"at": [24, 24], "text": "AGENT"}},
                {"id": "place_wire", "args": {"start": [5, 5], "end": [5, 5]}},
            ]),
        ));
        assert!(reason.contains("edit 3 (`place_wire`)"), "{reason}");
        assert!(reason.contains("Nothing was applied"), "{reason}");
        assert_eq!(state(&app, "b.snxsch"), before, "dirty_first={dirty_first}");
        assert_eq!(interaction_snapshot(&app), people);
        let engine = &app.document_state.engines[&path("b.snxsch")];
        assert!(!engine.in_transaction());
        assert!(app.document_state.agent_edits.target.is_none());
    }
    // The engine records again afterwards.
    assert_eq!(
        applied(&mut app, "b.snxsch", three_edits())["changed"],
        true
    );
}

#[test]
fn a_stale_expect_revision_changes_nothing_and_a_current_one_applies() {
    let mut app = app_at_work();
    let read = revision(&app, "b.snxsch");
    // The person edits after the agent read the document.
    assert_eq!(
        run(&mut app, "place_junction", &["at=50,50".to_string()]),
        CommandOutcome::Ran
    );
    let before = state(&app, "b.snxsch");
    assert_ne!(before.0, read);

    let reason = refusal(apply_at(&mut app, "b.snxsch", read, three_edits()));
    assert!(reason.contains("changed since revision"), "{reason}");
    assert!(reason.contains("Nothing was applied"), "{reason}");
    assert_eq!(
        state(&app, "b.snxsch"),
        before,
        "revision, history, document"
    );

    let answer = match apply_at(&mut app, "b.snxsch", before.0, three_edits()) {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("{other:?}"),
    };
    assert_eq!(answer["changed"], true);
    assert_ne!(revision(&app, "b.snxsch"), before.0);
}

#[test]
fn an_empty_list_and_a_malformed_one_change_nothing() {
    let mut app = app_at_work();
    let before = state(&app, "b.snxsch");
    let CommandOutcome::Answered(none) = apply(&mut app, "b.snxsch", json!([])) else {
        panic!("an empty list is answered");
    };
    assert_eq!(none["changed"], false);
    assert_eq!(none["revision"], before.0);
    for (edits, needle) in [
        (
            json!([{"id": "place_junction", "args": {"at": [1, 1]}}, {"id": "nope"}]),
            "edit 2",
        ),
        (json!([{"id": "read_document"}]), "does not edit a document"),
        (json!([{"id": "undo"}]), "steps the history"),
        (json!([{"id": "revert"}]), "steps the history"),
        (json!([{"id": "save_document"}]), "does not edit a document"),
        (
            json!([{"id": "assign_net_class", "args": {"net_name": "GND"}}]),
            "net classes",
        ),
        (json!([{"id": "apply_edits"}]), "its own edits"),
        (
            json!([{"id": "color_net", "args": {"at": [5, 0], "color": "#336699", "document": "x"}}]),
            "leave out its `document`",
        ),
        (
            json!([{"id": "place_junction", "args": {"at": [1, 1], "bogus": 1}}]),
            "bogus",
        ),
    ] {
        let reason = refusal(apply(&mut app, "b.snxsch", edits.clone()));
        assert!(reason.contains(needle), "{edits}: {reason}");
        assert_eq!(state(&app, "b.snxsch"), before, "{edits}");
    }
    assert!(app.document_state.agent_edits.labels.is_empty());
}

#[test]
fn an_agent_batch_leaves_the_persons_interaction_snapshot_byte_equal() {
    for file in ["b.snxsch", "a.snxsch"] {
        let mut app = app_at_work();
        let before = interaction_snapshot(&app);
        assert!(before.contains("move_pick=Some(Selection)"), "{before}");
        // The fixture's first panel refresh fills the canvas's designators.
        app.refresh_panel_ctx();
        let clears = app.interaction_state.active_canvas().content_cache_clears;
        applied(&mut app, file, three_edits());
        assert_eq!(interaction_snapshot(&app), before, "on {file}");
        if file == "a.snxsch" {
            // The person's canvas (and a drag's layers, dropped with its
            // caches) is not redrawn for an edit of a sheet it does not show.
            assert_eq!(
                app.interaction_state.active_canvas().content_cache_clears,
                clears
            );
        }
    }
}

/// A revert of a step on the person's own sheet waits while they draw or have
/// a move armed, so it is checked with them idle: the rest of what they hold
/// (selection, tool, panels) is untouched by it.
#[test]
fn revert_leaves_the_persons_interaction_snapshot_byte_equal() {
    for file in ["b.snxsch", "a.snxsch"] {
        let mut app = app_idle();
        let before = interaction_snapshot(&app);
        applied(&mut app, file, three_edits());
        let group = app.document_state.engines[&path(file)]
            .next_undo_group()
            .unwrap()
            .get();
        assert_eq!(
            revert(&mut app, file, group),
            CommandOutcome::Answered(json!({
                "reverted": true, "group": group, "revision": revision(&app, file),
            }))
        );
        assert_eq!(interaction_snapshot(&app), before, "revert on {file}");
    }
}

/// The person busy on sheet b (a wire in progress, drawing mode, a move
/// armed): a revert of an agent step on sheet a, which the main window does not
/// show, leaves every bit of it as it was.
#[test]
fn revert_on_another_sheet_leaves_a_person_busy_on_theirs_byte_equal() {
    let mut app = app_at_work();
    let busy = interaction_snapshot(&app);
    assert!(busy.contains("move_pick=Some(Selection)"), "{busy}");
    assert!(busy.contains("drawing_mode=true"), "{busy}");
    let group = applied(&mut app, "a.snxsch", three_edits())["group"]
        .as_u64()
        .unwrap();
    assert!(matches!(
        revert(&mut app, "a.snxsch", group),
        CommandOutcome::Answered(_)
    ));
    assert_eq!(interaction_snapshot(&app), busy);
}

/// A move the person armed, with a wire selected, survives an agent batch on
/// the same sheet that touches other items: nothing it is keyed on moves.
#[test]
fn a_move_in_progress_survives_an_agent_batch_on_the_same_sheet() {
    let mut app = app_at_work();
    let epoch = app.interaction_state.active_canvas().move_epoch;
    let selected = app.interaction_state.active_canvas().selected.clone();
    applied(&mut app, "b.snxsch", three_edits());
    let canvas = app.interaction_state.active_canvas();
    assert_eq!(
        canvas.move_epoch, epoch,
        "the float is stale once this moves"
    );
    assert_eq!(canvas.selected, selected, "or once the selection does");
    assert_eq!(
        app.ui_state.move_pick,
        Some(crate::canvas::MovePick::Selection)
    );
    assert!(canvas.drawing_mode);
    assert_eq!(canvas.wire_preview, vec![Point::new(1.0, 2.0)]);
}

#[test]
fn an_agent_deleting_the_selected_wire_drops_it_from_the_selection() {
    let mut app = app_at_work();
    // Two wires with a gap between them; the person selects the newer one.
    applied(
        &mut app,
        "b.snxsch",
        json!([
            {"id": "place_wire", "args": {"start": [20, 20], "end": [30, 20], "allow_open": true}},
            {"id": "place_wire", "args": {"start": [40, 20], "end": [50, 20], "allow_open": true}},
        ]),
    );
    let newer = {
        let sheet = app.document_state.engines[&path("b.snxsch")].document();
        sheet
            .wires
            .iter()
            .find(|w| w.start.x == 40.0)
            .expect("the newer wire")
            .uuid
    };
    let person = SelectedItem::new(newer, SelectedKind::Wire);
    let kept = SelectedItem::new(uid(WIRE_A), SelectedKind::Wire);
    app.interaction_state.active_canvas_mut().selected = vec![person, kept];

    // A wire that closes the gap folds all three into the oldest: the selected
    // wire is gone from the sheet.
    let answer = applied(
        &mut app,
        "b.snxsch",
        json!([{"id": "place_wire", "args": {"start": [30, 20], "end": [40, 20]}}]),
    );
    assert_eq!(answer["changed"], true);
    let sheet = app.document_state.engines[&path("b.snxsch")].document();
    assert!(
        sheet.wires.iter().all(|w| w.uuid != newer),
        "the batch removed the selected wire"
    );
    let selected = &app.interaction_state.active_canvas().selected;
    assert_eq!(
        selected,
        &vec![kept],
        "only the deleted wire left the selection"
    );
}

#[test]
fn the_batch_shows_live_on_the_active_canvas_and_every_undocked_window() {
    let mut app = app_at_work();
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        crate::app::state::WindowKind::UndockedTab {
            path: path("a.snxsch"),
            title: "a".into(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    applied(&mut app, "a.snxsch", three_edits());
    let engine = &app.document_state.engines[&path("a.snxsch")];
    let slot = &app.interaction_state.canvases[&window];
    assert_eq!(
        slot.render_cache
            .as_ref()
            .map(|c| c.snapshot().junctions.len()),
        Some(engine.document().junctions.len()),
        "the undocked window shows the new junction"
    );
    // The active canvas shows b; an edit of b is seen there at once.
    applied(&mut app, "b.snxsch", three_edits());
    let engine = &app.document_state.engines[&path("b.snxsch")];
    let shown = app.interaction_state.canvas.render_cache.as_ref().unwrap();
    assert_eq!(
        shown.snapshot().junctions.len(),
        engine.document().junctions.len()
    );
}

#[test]
fn revert_acts_only_while_the_group_is_the_newest_step_and_never_on_a_persons_step() {
    let mut app = app_idle();
    fixture::activate(&mut app, &path("a.snxsch"));
    let clean = state(&app, "a.snxsch");
    let group = applied(&mut app, "a.snxsch", three_edits())["group"]
        .as_u64()
        .unwrap();
    let agent = state(&app, "a.snxsch");

    // The person edits after the agent: their step is on top.
    assert_eq!(
        run(&mut app, "place_junction", &["at=50,50".to_string()]),
        CommandOutcome::Ran
    );
    let person = state(&app, "a.snxsch");
    let reason = refusal(revert(&mut app, "a.snxsch", group));
    assert!(reason.contains("a later step sits on top"), "{reason}");
    assert_eq!(state(&app, "a.snxsch"), person, "nothing changed");
    // A group the agent never made, and a person's own step, cannot be reverted.
    for other in [group + 1, 999] {
        let reason = refusal(revert(&mut app, "a.snxsch", other));
        assert!(reason.contains("not a step an agent made"), "{reason}");
        assert_eq!(state(&app, "a.snxsch"), person);
    }
    assert!(shape(&app, "a.snxsch").contains("J 50.000,50.000"));

    // Once the person's step is undone, the agent's is on top again, but the
    // person can still redo theirs, and that redo would bring the batch back.
    assert!(app.apply_engine_undo(false));
    assert_eq!(state(&app, "a.snxsch").3, agent.3);
    let undone = state(&app, "a.snxsch");
    let reason = refusal(revert(&mut app, "a.snxsch", group));
    assert!(reason.contains("can redo"), "{reason}");
    assert_eq!(state(&app, "a.snxsch"), undone, "nothing changed");
    // A newer agent step ends the person's redo; it is on top, and reverting
    // it leaves the older one on top, which reverts in turn.
    let fresh = applied(
        &mut app,
        "a.snxsch",
        json!([{"id": "place_junction", "args": {"at": [60, 60]}}]),
    );
    let newer = fresh["group"].as_u64().unwrap();
    let reason = refusal(revert(&mut app, "a.snxsch", group));
    assert!(reason.contains("a later step sits on top"), "{reason}");
    for (step, shape_after) in [(newer, &agent.3), (group, &clean.3)] {
        let CommandOutcome::Answered(done) = revert(&mut app, "a.snxsch", step) else {
            panic!("revert of {step} should be answered");
        };
        assert_eq!(done["reverted"], true);
        assert_eq!(&state(&app, "a.snxsch").3, shape_after);
    }
    assert!(!app.document_state.is_document_dirty(&path("a.snxsch")));
    // Neither lands on the redo stack, and a second revert has nothing to take.
    assert!(!app.document_state.engines[&path("a.snxsch")].can_redo());
    let again = refusal(revert(&mut app, "a.snxsch", group));
    assert!(again.contains("not a step an agent made"), "{again}");
}

#[test]
fn a_net_class_pick_on_top_blocks_revert_and_changes_nothing() {
    let mut app = app_at_work();
    let group = applied(&mut app, "b.snxsch", three_edits())["group"]
        .as_u64()
        .unwrap();
    let engine = app
        .document_state
        .engines
        .get_mut(&path("b.snxsch"))
        .unwrap();
    engine.record_class_pick(signex_engine::ClassPickMarker {
        project: 1,
        step: 1,
    });
    let before = state(&app, "b.snxsch");
    let reason = refusal(revert(&mut app, "b.snxsch", group));
    assert!(reason.contains("a later step sits on top"), "{reason}");
    assert_eq!(state(&app, "b.snxsch"), before);
}

#[test]
fn the_agent_gets_no_undo_through_the_headless_undo_of_its_own_step() {
    let mut app = app_at_work();
    applied(&mut app, "a.snxsch", three_edits());
    let before = state(&app, "a.snxsch");
    let reason = refusal(run(
        &mut app,
        "undo",
        &[format!("document={}", path("a.snxsch").display())],
    ));
    assert!(reason.contains("`revert`"), "{reason}");
    assert_eq!(state(&app, "a.snxsch"), before);
}

#[test]
fn a_batch_equals_the_same_edits_made_one_by_one_through_the_same_ids() {
    let edits = json!([
        {"id": "place_junction", "args": {"at": [20, 20]}},
        {"id": "place_wire", "args": {"start": [20, 20], "end": [30, 20], "allow_open": true}},
        {"id": "place_net_label", "args": {"at": [24, 24], "text": "AGENT", "rotation": 90}},
        {"id": "place_power_symbol", "args": {"at": [26, 26], "net_name": "VCC", "lib_id": "power:VCC"}},
        {"id": "draw_rectangle", "args": {"start": [40, 40], "end": [50, 45]}},
    ]);
    let mut batch = app_at_work();
    fixture::activate(&mut batch, &path("a.snxsch"));
    assert_eq!(
        applied(&mut batch, "a.snxsch", edits.clone())["changed"],
        true
    );

    let mut one_by_one = app_at_work();
    fixture::activate(&mut one_by_one, &path("a.snxsch"));
    for edit in edits.as_array().unwrap() {
        let operands: Vec<String> = edit["args"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, value)| {
                let text = super::operand_text(name, value).unwrap();
                format!("{name}={text}")
            })
            .collect();
        let name = edit["id"].as_str().unwrap();
        // A wire run answers its report; every other edit here only runs.
        let outcome = run(&mut one_by_one, name, &operands);
        assert!(
            outcome == CommandOutcome::Ran
                || (name == "place_wire" && matches!(outcome, CommandOutcome::Answered(_))),
            "{name}: {outcome:?}"
        );
    }
    assert_eq!(shape(&batch, "a.snxsch"), shape(&one_by_one, "a.snxsch"));
    let drawings = |app: &Signex| {
        app.document_state.engines[&path("a.snxsch")]
            .document()
            .drawings
            .len()
    };
    assert_eq!(drawings(&batch), drawings(&one_by_one));
    let symbols = |app: &Signex| {
        app.document_state.engines[&path("a.snxsch")]
            .document()
            .symbols
            .len()
    };
    assert_eq!(symbols(&batch), symbols(&one_by_one));
    // Five commands, five undo steps one by one; one for the batch.
    let steps = |app: &Signex| app.document_state.engines[&path("a.snxsch")].undo_position();
    assert_eq!(steps(&one_by_one), steps(&batch) + 4);
}

#[test]
fn net_colour_edits_run_on_the_named_sheet_and_the_batch_is_one_step() {
    let mut app = app_at_work();
    let people = interaction_snapshot(&app);
    let before = state(&app, "a.snxsch");
    let answer = applied(
        &mut app,
        "a.snxsch",
        json!([
            {"id": "color_net", "args": {"at": [5, 0], "color": "#336699"}},
            {"id": "place_junction", "args": {"at": [20, 20]}},
        ]),
    );
    assert_eq!(answer["changed"], true);
    let engine = &app.document_state.engines[&path("a.snxsch")];
    assert_eq!(engine.document().net_colors.len(), 3);
    assert_eq!(engine.undo_position(), before.1 + 1);
    assert_eq!(
        app.document_state.engines[&path("b.snxsch")]
            .document()
            .net_colors
            .len(),
        0
    );
    assert_eq!(interaction_snapshot(&app), people);
    let group = answer["group"].as_u64().unwrap();
    assert!(matches!(
        revert(&mut app, "a.snxsch", group),
        CommandOutcome::Answered(_)
    ));
    assert_eq!(
        state(&app, "a.snxsch"),
        (revision(&app, "a.snxsch"), before.1, false, before.3)
    );
}

#[test]
fn a_read_only_sheet_is_refused_before_anything_runs() {
    let mut app = app_at_work();
    let mut sheet = app.document_state.engines[&path("a.snxsch")]
        .document()
        .clone();
    sheet.unknown.newer_minor_source = Some("snxsch/9.9".into());
    app.document_state
        .engines
        .get_mut(&path("a.snxsch"))
        .unwrap()
        .set_document(sheet);
    let before = state(&app, "a.snxsch");
    let reason = refusal(apply(&mut app, "a.snxsch", three_edits()));
    assert!(
        reason.contains("Not changed: a.snxsch is read-only"),
        "{reason}"
    );
    assert_eq!(state(&app, "a.snxsch"), before);
}

#[test]
fn revert_waits_while_the_person_is_drawing_on_that_sheet() {
    let mut app = app_idle();
    let group = applied(&mut app, "b.snxsch", three_edits())["group"]
        .as_u64()
        .unwrap();
    app.interaction_state.wire_drawing = true;
    let before = state(&app, "b.snxsch");
    let reason = refusal(revert(&mut app, "b.snxsch", group));
    assert!(reason.contains("the person is drawing"), "{reason}");
    assert_eq!(state(&app, "b.snxsch"), before);
    app.interaction_state.wire_drawing = false;
    assert!(matches!(
        revert(&mut app, "b.snxsch", group),
        CommandOutcome::Answered(_)
    ));
}

/// A placement reads the sheet it edits, not the active one: a junction the
/// person's sheet already holds at that point does not stop the agent's.
#[test]
fn a_placement_checks_the_target_sheet_not_the_active_one() {
    let mut app = app_at_work();
    assert_eq!(
        run(&mut app, "place_junction", &["at=20,20".to_string()]),
        CommandOutcome::Ran,
        "the person's sheet (b) holds a junction there"
    );
    let answer = applied(
        &mut app,
        "a.snxsch",
        json!([{"id": "place_junction", "args": {"at": [20, 20]}}]),
    );
    assert_eq!(answer["changed"], true);
    assert!(shape(&app, "a.snxsch").contains("J 20.000,20.000"));
}

/// What a canvas shows follows the sheet only when the list lands: a colour
/// set by an edit that is later rolled back is never on the person's screen.
#[test]
fn a_rolled_back_edit_is_never_left_on_the_persons_canvas() {
    let mut app = app_at_work();
    let reason = refusal(apply(
        &mut app,
        "b.snxsch",
        json!([
            {"id": "color_net", "args": {"at": [5, 0], "color": "#336699"}},
            {"id": "place_wire", "args": {"start": [5, 5], "end": [5, 5]}},
        ]),
    ));
    assert!(reason.contains("edit 2"), "{reason}");
    let engine = &app.document_state.engines[&path("b.snxsch")];
    assert!(engine.document().net_colors.is_empty());
    let shown = app.interaction_state.canvas.render_cache.as_ref().unwrap();
    assert!(
        shown.snapshot().net_colors.is_empty(),
        "the canvas shows a colour the sheet no longer has"
    );
}

/// A wire edit snaps against the sheet it edits, not the person's: 0.5 mm from
/// the end of a wire of the target sheet, a click lands on that end.
#[test]
fn a_wire_edit_snaps_to_the_target_sheets_hotspots() {
    let mut app = app_idle();
    assert!(app.ui_state.snap_hotspots);
    assert!(matches!(
        apply(
            &mut app,
            "a.snxsch",
            json!([{"id": "place_wire", "args": {"start": [50, 50], "end": [60, 50], "allow_open": true}}])
        ),
        CommandOutcome::Answered(_)
    ));
    let outcome = apply(
        &mut app,
        "a.snxsch",
        json!([{"id": "place_wire", "args": {"start": [60, 50.5], "end": [70, 50.5]}}]),
    );
    let reason = refusal(outcome);
    assert!(reason.contains("not where a click lands"), "{reason}");
}

/// The drawing flag is global; the person's wire preview lives in the slot of
/// the window they draw in, which may be an undocked one.
#[test]
fn revert_waits_while_the_person_draws_in_an_undocked_window() {
    let mut app = app_idle();
    let group = applied(
        &mut app,
        "a.snxsch",
        json!([{"id": "place_junction", "args": {"at": [20, 20]}}]),
    )["group"]
        .as_u64()
        .unwrap();
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        crate::app::state::WindowKind::UndockedTab {
            path: path("a.snxsch"),
            title: "a".into(),
        },
    );
    let mut slot = crate::canvas::CanvasSlot::new();
    slot.drawing_mode = true;
    slot.wire_preview = vec![Point::new(20.0, 20.0)];
    app.interaction_state.canvases.insert(window, slot);
    app.interaction_state.wire_drawing = true;
    let before = state(&app, "a.snxsch");
    let reason = refusal(revert(&mut app, "a.snxsch", group));
    assert!(reason.contains("drawing"), "{reason}");
    assert_eq!(state(&app, "a.snxsch"), before);
}

/// Reverting the label the person is typing into would throw their text away
/// on submit.
#[test]
fn revert_waits_while_the_person_types_into_an_item_of_the_sheet() {
    let mut app = app_idle();
    let answer = applied(
        &mut app,
        "b.snxsch",
        json!([{"id": "place_net_label", "args": {"at": [24, 24], "text": "AGENT"}}]),
    );
    let group = answer["group"].as_u64().unwrap();
    let label = app.document_state.engines[&path("b.snxsch")]
        .document()
        .labels
        .iter()
        .find(|l| l.text == "AGENT")
        .unwrap()
        .uuid;
    app.interaction_state.editing_text = Some(crate::app::TextEditState {
        uuid: label,
        kind: SelectedKind::Label,
        text: "PERSON".into(),
        original_text: "AGENT".into(),
        world_x: 24.0,
        world_y: 24.0,
        sheet: Some(path("b.snxsch")),
        placement: None,
    });
    let before = state(&app, "b.snxsch");
    let reason = refusal(revert(&mut app, "b.snxsch", group));
    assert!(reason.contains("typing"), "{reason}");
    assert_eq!(state(&app, "b.snxsch"), before);
    app.interaction_state.editing_text = None;
    assert!(matches!(
        revert(&mut app, "b.snxsch", group),
        CommandOutcome::Answered(_)
    ));
}

/// A move armed on the sheet the main window shows is not cut under; one on
/// another sheet is not the person's business here.
#[test]
fn revert_waits_while_the_person_has_a_move_armed_on_that_sheet() {
    let mut app = app_idle();
    let on_b = applied(&mut app, "b.snxsch", three_edits())["group"]
        .as_u64()
        .unwrap();
    let on_a = applied(&mut app, "a.snxsch", three_edits())["group"]
        .as_u64()
        .unwrap();
    app.ui_state.move_pick = Some(crate::canvas::MovePick::Selection);
    app.interaction_state.canvas.move_pick = Some(crate::canvas::MovePick::Selection);
    let before = state(&app, "b.snxsch");
    let reason = refusal(revert(&mut app, "b.snxsch", on_b));
    assert!(reason.contains("move armed"), "{reason}");
    assert_eq!(state(&app, "b.snxsch"), before);
    assert!(matches!(
        revert(&mut app, "a.snxsch", on_a),
        CommandOutcome::Answered(_)
    ));
}

/// The default font is the project's of the sheet the edit lands on, not of
/// the sheet the person has open.
#[test]
fn a_placed_text_takes_the_target_projects_font() {
    use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
    use signex_types::schematic::SchematicSheet;
    let dir = std::env::temp_dir().join(format!("signex-2178-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["root.snxsch"]);
    let root = dir.join("root.snxsch");
    open_with(&mut app.document_state, &root, SchematicSheet::default());
    app.document_state.active_path = Some(root.clone());
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(crate::app::Message::Project(crate::app::ProjectMsg::Tab(
        crate::app::ProjectTabMsg::SchematicDefaultFontEdited {
            project_id,
            text: "Roboto".into(),
        },
    )));
    assert_eq!(
        app.document_state.placed_font_family().as_deref(),
        Some("Roboto")
    );
    app.document_state.projects[0]
        .data
        .schematic_defaults
        .text_size_pt = Some(20.0);
    let project_text_mm = app
        .document_state
        .active_schematic_defaults()
        .placed_text_mm();
    // The person works on a loose sheet outside the project.
    let loose =
        std::env::temp_dir().join(format!("signex-2178-loose-{}.snxsch", uuid::Uuid::now_v7()));
    open_with(&mut app.document_state, &loose, SchematicSheet::default());
    app.document_state.active_path = Some(loose);
    assert_eq!(app.document_state.placed_font_family(), None);
    let loose_text_mm = app
        .document_state
        .active_schematic_defaults()
        .placed_text_mm();
    assert_ne!(loose_text_mm, project_text_mm);
    let revision = app.document_state.engines[&root].revision();
    let outcome = run(
        &mut app,
        "apply_edits",
        &[
            format!("document={}", root.display()),
            format!("expect_revision={revision}"),
            format!(
                "edits={}",
                json!([{"id": "place_text", "args": {"at": [12.7, 12.7], "text": "T"}}])
            ),
        ],
    );
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    let doc = app.document_state.engines[&root].document();
    assert_eq!(doc.text_notes[0].font_family.as_deref(), Some("Roboto"));
    assert_eq!(doc.text_notes[0].font_size, project_text_mm);
}

/// The person's Undo of an agent step of mixed kinds repaints every kind.
#[test]
fn undo_of_a_mixed_agent_step_repaints_every_kind() {
    let mut app = app_idle();
    app.refresh_panel_ctx();
    applied(
        &mut app,
        "b.snxsch",
        json!([
            {"id": "place_junction", "args": {"at": [20, 20]}},
            {"id": "place_net_label", "args": {"at": [24, 24], "text": "AGENT"}},
        ]),
    );
    let shown = |app: &Signex| {
        let snap = app
            .interaction_state
            .canvas
            .render_cache
            .as_ref()
            .unwrap()
            .snapshot();
        (snap.junctions.len(), snap.labels.len())
    };
    let doc = |app: &Signex| {
        let d = app.document_state.engines[&path("b.snxsch")].document();
        (d.junctions.len(), d.labels.len())
    };
    assert_eq!(shown(&app), doc(&app));
    assert!(app.apply_engine_undo(true));
    assert_eq!(
        shown(&app),
        doc(&app),
        "the canvas shows what the sheet holds"
    );
}

/// Engine notices of edits that are rolled back never reach Messages; the
/// refusal does.
#[test]
fn a_rolled_back_list_reports_only_its_refusal() {
    let mut app = app_idle();
    let mark = crate::diagnostics::newest_entry_id();
    let reason = refusal(apply(
        &mut app,
        "b.snxsch",
        json!([
            {"id": "place_junction", "args": {"at": [20, 20]}},
            {"id": "place_wire", "args": {"start": [5, 5], "end": [5, 5]}},
        ]),
    ));
    assert!(reason.contains("Nothing was applied"), "{reason}");
    let said: Vec<String> = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|entry| entry.message)
        .collect();
    assert!(
        // The label this list would have had; the log is shared with the
        // tests running beside this one.
        said.iter()
            .all(|m| !m.contains("Agent: place_junction, place_wire")),
        "nothing says an edit was applied: {said:?}"
    );
}

/// An undocked window showing `file`, with a slot whose move is `armed`.
fn undock(app: &mut Signex, file: &str, armed: bool) {
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        crate::app::state::WindowKind::UndockedTab {
            path: path(file),
            title: file.into(),
        },
    );
    let mut slot = crate::canvas::CanvasSlot::new();
    slot.move_pick = armed.then_some(crate::canvas::MovePick::Object);
    app.interaction_state.canvases.insert(window, slot);
}

/// A move armed from an undocked window (the Active Bar there arms it on that
/// window's slot) is not cut under, though the main window shows another sheet.
#[test]
fn revert_waits_while_a_move_is_armed_from_an_undocked_window_of_that_sheet() {
    let mut app = app_idle();
    let group = applied(&mut app, "a.snxsch", three_edits())["group"]
        .as_u64()
        .unwrap();
    undock(&mut app, "a.snxsch", true);
    app.ui_state.move_pick = Some(crate::canvas::MovePick::Object);
    let before = state(&app, "a.snxsch");
    let reason = refusal(revert(&mut app, "a.snxsch", group));
    assert!(reason.contains("move armed"), "{reason}");
    assert_eq!(state(&app, "a.snxsch"), before);
}

/// The opposite: a move armed from an undocked window of b says nothing about
/// sheet a, which the main window shows; the app-wide flag is not the sheet's.
#[test]
fn a_move_armed_on_another_sheets_window_does_not_stop_a_revert() {
    let mut app = app_idle();
    fixture::activate(&mut app, &path("a.snxsch"));
    let group = applied(&mut app, "a.snxsch", three_edits())["group"]
        .as_u64()
        .unwrap();
    undock(&mut app, "b.snxsch", true);
    app.ui_state.move_pick = Some(crate::canvas::MovePick::Object);
    assert!(matches!(
        revert(&mut app, "a.snxsch", group),
        CommandOutcome::Answered(_)
    ));
}

/// What a handler says it did is held until the list lands: a refused list
/// leaves no line behind, a landed one shows it.
#[test]
fn a_rolled_back_list_shows_no_handler_line_and_a_landed_one_does() {
    let said_since = |mark| -> Vec<String> {
        crate::diagnostics::entries_since(mark)
            .entries
            .into_iter()
            .map(|entry| entry.message)
            .collect()
    };
    // One sheet per case: the log is shared with the tests running beside.
    for (file, lands) in [("held-2178-a.snxsch", false), ("held-2178-b.snxsch", true)] {
        let mut app = app_with(&[(file, 150)]);
        app.ui_state.snap_enabled = false;
        applied(
            &mut app,
            file,
            json!([{"id": "color_net", "args": {"at": [5, 0], "color": "#336699"}}]),
        );
        let mark = crate::diagnostics::newest_entry_id();
        let mut edits = vec![json!({"id": "clear_all_net_colors"})];
        if !lands {
            edits.push(json!({"id": "place_wire", "args": {"start": [5, 5], "end": [5, 5]}}));
        }
        let outcome = apply(&mut app, file, json!(edits));
        let said = said_since(mark);
        let cleared = format!("Cleared net-color overrides on {file}");
        assert_eq!(
            said.iter().any(|m| m.contains(&cleared)),
            lands,
            "{outcome:?} {said:?}"
        );
        assert_eq!(matches!(outcome, CommandOutcome::Answered(_)), lands);
    }
}
