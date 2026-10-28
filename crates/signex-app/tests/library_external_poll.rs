//! #531 review — how an external `.snxlib` edit reaches the UI.
//!
//! `view` reads only `OpenLibrary` caches, so something in `update` has to
//! notice that another process changed the file and refresh them. That is
//! the external-edit poll: a cheap `stat` comparison against the stamp the
//! caches were filled under (`libraries_changed`), followed by
//! `refresh_components` for the libraries it names.

mod support;

use signex_app::library::external_poll::libraries_changed;
use signex_app::library::state::LibraryState;

use support::{Scale, append_component, generate_library};

const SYMBOLS: usize = 4;
const FOOTPRINTS: usize = 4;

#[test]
fn poll_reports_an_external_edit_once_and_the_refresh_shows_it() {
    let scale = Scale::new("poll", SYMBOLS, FOOTPRINTS);
    let tmp = tempfile::Builder::new()
        .prefix("signex-ext-poll-")
        .tempdir()
        .expect("tempdir");
    let snxlib = generate_library(tmp.path(), "poll", &scale).expect("generate_library");

    let mut state = LibraryState::default();
    signex_app::library::commands::open_library(&mut state, snxlib.clone()).expect("open");
    let rows_before = state.library_at(&snxlib).expect("mounted").total_rows();

    assert!(
        libraries_changed(&state.watched_libraries()).is_empty(),
        "nothing changed since the caches were filled"
    );

    // Another process adds a component behind the mounted adapter's back.
    append_component(&snxlib, SYMBOLS).expect("append_component");

    let changed: Vec<_> = libraries_changed(&state.watched_libraries())
        .into_iter()
        .map(|(path, _stamp)| path)
        .collect();
    assert_eq!(
        changed,
        std::slice::from_ref(&snxlib),
        "the poll must name the library whose file changed"
    );

    state.refresh_components(&snxlib).expect("refresh");

    let rows_after = state.library_at(&snxlib).expect("mounted").total_rows();
    assert_eq!(
        rows_after,
        rows_before + 1,
        "the external row is now cached"
    );
    assert!(
        libraries_changed(&state.watched_libraries()).is_empty(),
        "the refresh re-stamped the cache, so the same edit is not reported twice"
    );
}

use signex_app::app::{Message, Signex};
use signex_app::library::external_poll::PollOutcome;
use signex_app::library::messages::LibraryMessage;

/// Deliver a poll outcome, then land the off-thread refreshes it started
/// (a test cannot run the `Task`; `land_pending_refresh` is its completion).
fn polled(app: &mut Signex, outcome: PollOutcome) {
    let changed = match &outcome {
        PollOutcome::Changed(changed) => changed.clone(),
        _ => Vec::new(),
    };
    let generations =
        |app: &Signex, path: &std::path::Path| app.library.refresh_generations.get(path).copied();
    let before: Vec<_> = changed
        .iter()
        .map(|(path, _)| generations(app, path))
        .collect();
    drop(
        app.update(Message::Library(LibraryMessage::ExternalEditsPolled(
            outcome,
        ))),
    );
    for ((path, _), generation) in changed.iter().zip(before) {
        if generations(app, path) != generation {
            app.land_pending_refresh(path);
        }
    }
}

/// A poll that lands while a refresh of the library is already on its way
/// starts nothing and reports nothing: the app's own write would otherwise
/// look like an outside edit, and a slow disk would never get to land.
#[test]
fn a_poll_during_an_in_flight_refresh_starts_nothing_and_logs_nothing() {
    let (_t, snxlib, mut app) = mounted("extpoll-inflight");
    append_component(&snxlib, SYMBOLS).expect("append_component");
    let changed = libraries_changed(&app.library.watched_libraries());
    assert_eq!(changed.len(), 1);
    app.library
        .begin_refresh(
            &snxlib,
            signex_app::library::refresh::RefreshFollowUp::Nothing,
        )
        .expect("a refresh is on its way");
    let generation = app.library.refresh_generations.get(&snxlib).copied();

    polled(&mut app, PollOutcome::Changed(changed));

    assert_eq!(
        app.library.refresh_generations.get(&snxlib).copied(),
        generation,
        "no second refresh was started"
    );
    assert_eq!(logged(&["extpoll-inflight changed on disk"]), 0);
}

/// #827: a tick while a timed-out pass's `stat` thread is still running
/// starts no second pass.
#[test]
fn no_new_poll_starts_behind_a_hung_stat() {
    let scale = Scale::new("hung", SYMBOLS, FOOTPRINTS);
    let tmp = tempfile::tempdir().expect("tempdir");
    let snxlib = generate_library(tmp.path(), "hung", &scale).expect("generate_library");
    let (mut app, _t) = Signex::new();
    signex_app::library::commands::open_library(&mut app.library, snxlib).expect("open");
    app.library
        .external_poll_stat_running
        .store(true, std::sync::atomic::Ordering::SeqCst);

    drop(app.update(Message::Library(LibraryMessage::ExternalEditsPollTicked)));
    assert!(
        !app.library.external_poll_in_flight,
        "the tick must be dropped"
    );
}

/// How many Messages-panel entries contain every one of `needles`.
fn logged(needles: &[&str]) -> usize {
    drop(signex_app::diagnostics::init_logging());
    signex_app::diagnostics::recent_entries()
        .iter()
        .filter(|e| needles.iter().all(|n| e.message.contains(n)))
        .count()
}

fn mounted(name: &'static str) -> (tempfile::TempDir, std::path::PathBuf, Signex) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let snxlib = generate_library(tmp.path(), name, &Scale::new(name, SYMBOLS, FOOTPRINTS))
        .expect("generate_library");
    let (mut app, _t) = Signex::new();
    signex_app::library::commands::open_library(&mut app.library, snxlib.clone()).expect("open");
    (tmp, snxlib, app)
}

/// #827: a stalled pass is reported once across repeated timeouts, and
/// "resumed" once when a pass completes again.
#[test]
fn a_stalled_or_failing_poll_is_reported_once_and_recovers() {
    let stalls = || logged(&["took longer than"]);
    let resumes = || logged(&["external-edit checks resumed"]);
    let (stalls_before, resumes_before) = (stalls(), resumes());
    let (mut app, _t) = Signex::new();
    app.library.external_poll_in_flight = true;

    polled(&mut app, PollOutcome::TimedOut);
    polled(&mut app, PollOutcome::TimedOut);
    assert!(!app.library.external_poll_in_flight);
    assert_eq!(stalls() - stalls_before, 1, "one warning for two timeouts");

    polled(&mut app, PollOutcome::Changed(Vec::new()));
    assert_eq!(resumes() - resumes_before, 1);

    // #869 review: a pass that fails every tick is reported once too, not
    // every 2 s, and the next completed pass says checks resumed.
    let failures = || logged(&["external edits failed: stat exploded"]);
    let failures_before = failures();
    polled(&mut app, PollOutcome::Failed("stat exploded".into()));
    polled(&mut app, PollOutcome::Failed("stat exploded".into()));
    assert_eq!(
        failures() - failures_before,
        1,
        "one warning for two failures"
    );

    polled(&mut app, PollOutcome::Changed(Vec::new()));
    assert_eq!(resumes() - resumes_before, 2);
}

/// Positive control: a real external edit is refreshed and reported once.
#[test]
fn an_external_edit_is_reported_in_the_messages_panel() {
    let (_t, snxlib, mut app) = mounted("extpoll-real");
    let before = logged(&["extpoll-real changed on disk; reloaded"]);
    append_component(&snxlib, SYMBOLS).expect("append_component");
    let changed = libraries_changed(&app.library.watched_libraries());
    polled(&mut app, PollOutcome::Changed(changed));
    assert_eq!(
        logged(&["extpoll-real changed on disk; reloaded"]) - before,
        1
    );
}

/// #827 item 1: a result computed before an in-app refresh re-stamped the
/// library is stale, and reports nothing.
#[test]
fn a_stale_poll_result_is_not_reported_as_an_external_edit() {
    let (_t, snxlib, mut app) = mounted("extpoll-stale");
    append_component(&snxlib, SYMBOLS).expect("append_component");
    let changed = libraries_changed(&app.library.watched_libraries());
    assert_eq!(changed.len(), 1);
    app.library.refresh_components(&snxlib).expect("refresh");

    polled(&mut app, PollOutcome::Changed(changed));
    assert_eq!(logged(&["extpoll-stale changed on disk"]), 0);
}

/// #827 item 1: the refresh after the app's own class write is silent, but
/// an external edit made after it is still reported.
#[test]
fn the_apps_own_class_write_is_silent_but_a_later_external_edit_is_not() {
    let (_t, snxlib, mut app) = mounted("extpoll-own");
    // Stand-in for the class write: the file changes, then `refresh_classes`
    // runs, as the class-edit dispatch does.
    append_component(&snxlib, SYMBOLS).expect("append_component");
    app.library.refresh_classes(&snxlib);
    let changed = libraries_changed(&app.library.watched_libraries());
    polled(&mut app, PollOutcome::Changed(changed));
    assert_eq!(
        logged(&["extpoll-own changed on disk"]),
        0,
        "own write is silent"
    );

    app.library.refresh_classes(&snxlib);
    append_component(&snxlib, SYMBOLS + 1).expect("append_component");
    let changed = libraries_changed(&app.library.watched_libraries());
    polled(&mut app, PollOutcome::Changed(changed));
    assert_eq!(
        logged(&["extpoll-own changed on disk; reloaded"]),
        1,
        "an edit after the own write is external"
    );
}

/// A refresh read that finishes before a library rename but whose report
/// reaches `update` after it: the report names the old path. It must clear
/// the in-flight mark of the renamed library, or the poll skips that
/// library for the rest of the session.
#[test]
fn a_read_reported_after_a_rename_does_not_pin_the_poll_off() {
    use signex_app::library::refresh::RefreshFollowUp;
    use signex_app::library::state::LibrarySnapshot;

    let (tmp, snxlib, mut app) = mounted("extpoll-renamed");
    let start = app
        .library
        .begin_refresh(&snxlib, RefreshFollowUp::Nothing)
        .expect("a refresh is on its way");
    let snapshot = LibrarySnapshot::read(start.adapter.as_ref(), &snxlib);
    let generation = start.generation;
    drop(start); // the read is done; relocating needs the adapter unshared
    let renamed = tmp.path().join("renamed.snxlib");
    std::fs::rename(&snxlib, &renamed).expect("rename the file");
    app.library
        .relocate_library(&snxlib, &renamed)
        .expect("relocate");
    assert!(app.library.refresh_in_flight(&renamed));

    let finish = app.library.finish_refresh(&snxlib, generation, snapshot);

    assert_eq!(finish.root, renamed);
    assert!(
        !app.library.refresh_in_flight(&renamed),
        "the poll must be allowed to look at the renamed library again"
    );
}
