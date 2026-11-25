//! Live ERC (#1288): an edit starts a wait, the wait starts a quiet check,
//! and the check brings the findings on what the edit touched, and only
//! those, to the canvas and the ERC panel. Run ERC still checks everything.

use std::path::PathBuf;

use iced::advanced::subscription::{Hasher, into_recipes};
use signex_types::schematic::{Point, SchematicSheet, Wire};
use uuid::Uuid;

use super::{LiveErc, LiveErcMsg};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::{EditMsg, ErcMsg, Message, Signex, TabInfo, TabKind};

fn wire(a: Point, b: Point) -> Wire {
    Wire {
        uuid: Uuid::now_v7(),
        start: a,
        end: b,
        ..Default::default()
    }
}

/// One open sheet holding a lone wire whose two ends dangle, never checked.
fn app_with_a_dangling_wire() -> (Signex, PathBuf, Wire) {
    let dir = PathBuf::from("/w/live");
    let path = dir.join("top.snxsch");
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    let old = wire(Point::new(0.0, 50.0), Point::new(10.0, 50.0));
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(old.clone());
    open_with(&mut app.document_state, &path, sheet);
    app.document_state.tabs.push(TabInfo {
        title: "top".to_owned(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.ui_state.live_erc.enabled = true;
    // The first update after the open takes the sheet as it is.
    app.sync_live_erc();
    (app, path, old)
}

/// Place `new` on the active sheet as a person's edit, through `update`'s
/// tail as the app runs it.
fn draw(app: &mut Signex, new: &Wire) {
    app.apply_engine_command(
        signex_engine::Command::PlaceWireSegment { wire: new.clone() },
        true,
        true,
    );
    app.sync_live_erc();
}

fn on(new: &Wire, v: &signex_erc::Violation) -> bool {
    v.primary.is_some_and(|p| p.uuid == new.uuid)
        || [new.start, new.end]
            .iter()
            .any(|end| signex_net::pt_key(end) == signex_net::pt_key(&v.location))
}

fn subscription_ids(app: &Signex) -> Vec<u64> {
    into_recipes(app.live_erc_subscription())
        .iter()
        .map(|recipe| {
            let mut hasher = Hasher::default();
            recipe.hash(&mut hasher);
            std::hash::Hasher::finish(&hasher)
        })
        .collect()
}

#[test]
fn live_erc_is_on_at_launch_unless_the_preference_is_off() {
    let (app, _task) = Signex::new();
    assert_eq!(
        app.ui_state.live_erc.enabled,
        crate::fonts::read_live_erc_pref_from(None),
        "a machine with no saved choice gets the default"
    );
    assert!(crate::fonts::read_live_erc_pref_from(None));
    assert!(app.ui_state.live_erc.runs_here);
}

#[test]
fn an_edit_starts_the_wait_and_a_later_edit_restarts_it() {
    let (mut app, _path, _old) = app_with_a_dangling_wire();
    assert!(
        subscription_ids(&app).is_empty(),
        "nothing owed before an edit"
    );

    draw(
        &mut app,
        &wire(Point::new(100.0, 0.0), Point::new(110.0, 0.0)),
    );
    let first = subscription_ids(&app);
    assert_eq!(first.len(), 1, "one timer while a check is owed");

    draw(
        &mut app,
        &wire(Point::new(200.0, 0.0), Point::new(210.0, 0.0)),
    );
    let second = subscription_ids(&app);
    assert_eq!(second.len(), 1);
    assert_ne!(first, second, "a new edit restarts the timer");
}

#[test]
fn no_wait_starts_with_live_erc_off_or_without_a_window() {
    let offs: [fn(&mut LiveErc); 2] = [
        |live| live.set_enabled(false),
        |live| live.runs_here = false,
    ];
    for off in offs {
        let (mut app, _path, _old) = app_with_a_dangling_wire();
        off(&mut app.ui_state.live_erc);
        draw(
            &mut app,
            &wire(Point::new(100.0, 0.0), Point::new(110.0, 0.0)),
        );
        assert!(subscription_ids(&app).is_empty());
        assert!(app.ui_state.live_erc.pending().is_none());
    }
}

#[test]
fn a_live_check_marks_only_what_the_edit_touched() {
    let (mut app, path, old) = app_with_a_dangling_wire();
    let new = wire(Point::new(100.0, 0.0), Point::new(110.0, 0.0));
    draw(&mut app, &new);
    assert!(app.run_live_erc_blocking());

    let cached = &app.ui_state.erc_violations_by_path[&path];
    assert!(!cached.is_empty(), "the new wire's dangling ends are found");
    assert!(
        cached.iter().all(|v| on(&new, v)),
        "quiet: the old wire nobody touched raises nothing"
    );
    // The canvas markers and the ERC panel's rows (its counts) follow.
    let markers = &app.interaction_state.active_canvas().erc_markers;
    assert_eq!(markers.len(), cached.len());
    assert_eq!(app.build_erc_diagnostic_entries().len(), cached.len());
    assert!(!app.ui_state.erc_running, "the panel never showed a run");

    // Run ERC stays the full check.
    app.run_erc_blocking();
    let full = &app.ui_state.erc_violations_by_path[&path];
    assert!(full.iter().any(|v| on(&old, v)), "{full:?}");
}

#[test]
fn an_undo_takes_the_findings_of_the_undone_edit_away() {
    let (mut app, path, _old) = app_with_a_dangling_wire();
    let new = wire(Point::new(100.0, 0.0), Point::new(110.0, 0.0));
    draw(&mut app, &new);
    assert!(app.run_live_erc_blocking());
    assert!(!app.ui_state.erc_violations_by_path[&path].is_empty());

    let waiting = app.ui_state.live_erc.pending();
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let after = app.ui_state.live_erc.pending();
    assert!(after.is_some() && after != waiting, "an undo is an edit");
    assert!(app.run_live_erc_blocking());
    assert!(app.ui_state.erc_violations_by_path[&path].is_empty());
    assert!(app.interaction_state.active_canvas().erc_markers.is_empty());
}

#[test]
fn a_finding_already_shown_on_an_untouched_wire_stays() {
    let (mut app, path, old) = app_with_a_dangling_wire();
    app.run_erc_blocking();
    let before = app.ui_state.erc_violations_by_path[&path].len();
    assert!(before > 0);
    draw(
        &mut app,
        &wire(Point::new(100.0, 0.0), Point::new(110.0, 0.0)),
    );
    assert!(app.run_live_erc_blocking());
    let after = &app.ui_state.erc_violations_by_path[&path];
    assert_eq!(after.iter().filter(|v| on(&old, v)).count(), before);
    assert!(after.len() > before, "the new wire's findings join them");
}

#[test]
fn a_run_erc_started_meanwhile_drops_the_live_check() {
    let (mut app, path, _old) = app_with_a_dangling_wire();
    draw(
        &mut app,
        &wire(Point::new(100.0, 0.0), Point::new(110.0, 0.0)),
    );
    let (generation, inputs) = app.begin_live_erc();
    let outcome = Ok(super::job::run_live_erc_job(inputs));
    let _ = app.begin_erc_run();
    assert!(!app.apply_live_erc_result(generation, &outcome));
    assert!(!app.ui_state.erc_violations_by_path.contains_key(&path));
}

#[test]
fn a_superseded_wait_starts_nothing_and_a_run_in_flight_waits_again() {
    let (mut app, _path, _old) = app_with_a_dangling_wire();
    draw(
        &mut app,
        &wire(Point::new(100.0, 0.0), Point::new(110.0, 0.0)),
    );
    let token = app.ui_state.live_erc.pending().expect("a wait");

    let stale = Message::Erc(ErcMsg::Live(LiveErcMsg::DebounceElapsed(token - 1)));
    let _ = app.update(stale);
    assert_eq!(
        app.ui_state.live_erc.pending(),
        Some(token),
        "still waiting"
    );
    assert!(!app.ui_state.live_erc.is_running());

    app.ui_state.erc_running = true;
    let due = Message::Erc(ErcMsg::Live(LiveErcMsg::DebounceElapsed(token)));
    let _ = app.update(due);
    let again = app.ui_state.live_erc.pending().expect("waits again");
    assert_ne!(again, token);
    assert!(!app.ui_state.live_erc.is_running());

    app.ui_state.erc_running = false;
    let due = Message::Erc(ErcMsg::Live(LiveErcMsg::DebounceElapsed(again)));
    let task = app.update(due);
    assert!(app.ui_state.live_erc.is_running(), "the check is out");
    assert!(task.units() > 0, "the check runs off the UI thread");
    assert!(app.ui_state.live_erc.pending().is_none());
}

#[test]
fn a_failed_worker_is_said_once_and_changes_nothing() {
    let (mut app, path, _old) = app_with_a_dangling_wire();
    let line = "Live ERC worker failed: probe";
    let before = crate::diagnostics::logged_count(line);
    let generation = app.ui_state.live_erc.begin();
    assert!(!app.apply_live_erc_result(generation, &Err(line.to_string())));
    assert_eq!(crate::diagnostics::logged_count(line), before + 1);
    assert!(!app.ui_state.erc_violations_by_path.contains_key(&path));
    assert!(!app.ui_state.live_erc.is_running());
}

#[test]
fn a_sheet_first_seen_in_its_editing_update_is_checked_whole() {
    let (mut app, path, old) = app_with_a_dangling_wire();
    app.ui_state.live_erc.baselines.clear();
    draw(
        &mut app,
        &wire(Point::new(100.0, 0.0), Point::new(110.0, 0.0)),
    );
    assert!(matches!(
        app.ui_state.live_erc.baselines.get(&path),
        Some(None)
    ));
    assert!(app.run_live_erc_blocking());
    let cached = &app.ui_state.erc_violations_by_path[&path];
    assert!(cached.iter().any(|v| on(&old, v)), "{cached:?}");
}

#[test]
fn a_closed_sheet_loses_its_baseline() {
    let (mut app, path, _old) = app_with_a_dangling_wire();
    assert!(app.ui_state.live_erc.baselines.contains_key(&path));
    app.document_state.engines.remove(&path);
    app.sync_live_erc();
    assert!(!app.ui_state.live_erc.baselines.contains_key(&path));
}

#[test]
fn the_toggle_message_flips_the_preference_and_saves_it() {
    let (mut app, _path, _old) = app_with_a_dangling_wire();
    draw(
        &mut app,
        &wire(Point::new(100.0, 0.0), Point::new(110.0, 0.0)),
    );
    let task = app.update(Message::Erc(ErcMsg::Live(LiveErcMsg::Toggled)));
    assert!(!app.ui_state.live_erc.enabled);
    assert!(task.units() > 0, "the preference write");
    assert!(
        app.ui_state.live_erc.pending().is_none(),
        "turning it off drops the wait"
    );
    let _ = app.update(Message::Erc(ErcMsg::Live(LiveErcMsg::Toggled)));
    assert!(app.ui_state.live_erc.enabled);
    assert!(
        !app.ui_state.live_erc.baselines.is_empty(),
        "turning it on takes the open sheets as they are"
    );
}

#[test]
fn a_finding_on_a_net_another_sheet_renamed_is_rechecked_there() {
    let outcome = super::LiveErcOutcome {
        by_path: Default::default(),
        scopes: Default::default(),
        changed_names: ["VCC".to_string()].into(),
        read: Default::default(),
        page_frames: Default::default(),
    };
    let scope = outcome.scope_of(std::path::Path::new("/w/live/other.snxsch"));
    assert!(!scope.whole_sheet);
    assert!(scope.objects.is_empty() && scope.points.is_empty());
    assert!(scope.net_names.contains("VCC"));
}
