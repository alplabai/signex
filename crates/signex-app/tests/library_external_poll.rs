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

fn polled(app: &mut Signex, outcome: PollOutcome) {
    drop(
        app.update(Message::Library(LibraryMessage::ExternalEditsPolled(
            outcome,
        ))),
    );
}

/// #827: a pass that times out (a hung `stat` on a dead mount) clears the
/// in-flight flag, is reported once rather than every tick, and the report
/// resets once a pass completes again.
#[test]
fn a_timed_out_poll_is_reported_once_and_recovers() {
    let (mut app, _t) = Signex::new();
    app.library.external_poll_in_flight = true;
    polled(&mut app, PollOutcome::TimedOut);
    assert!(!app.library.external_poll_in_flight);
    assert!(app.library.external_poll_stall_reported);

    polled(&mut app, PollOutcome::TimedOut);
    assert!(app.library.external_poll_stall_reported, "still stalled");

    polled(&mut app, PollOutcome::Changed(Vec::new()));
    assert!(!app.library.external_poll_stall_reported, "recovered");
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

/// #827: a poll result computed before an in-app write re-stamped the
/// library is stale; delivering it refreshes nothing and changes no stamp.
/// A class edit marks the library as edited by the app, and the refresh the
/// poll then triggers clears that mark.
#[test]
fn stale_and_own_edit_poll_results_are_not_external_edits() {
    let scale = Scale::new("own", SYMBOLS, FOOTPRINTS);
    let tmp = tempfile::tempdir().expect("tempdir");
    let snxlib = generate_library(tmp.path(), "own", &scale).expect("generate_library");
    let (mut app, _t) = Signex::new();
    signex_app::library::commands::open_library(&mut app.library, snxlib.clone()).expect("open");

    // The file changes; the poll sees it; then the app refreshes on its own
    // before the result arrives.
    append_component(&snxlib, SYMBOLS).expect("append_component");
    let changed = libraries_changed(&app.library.watched_libraries());
    assert_eq!(changed.len(), 1);
    app.library.refresh_components(&snxlib).expect("refresh");
    let stamp = app.library.library_at(&snxlib).unwrap().observed_stamp;
    polled(&mut app, PollOutcome::Changed(changed.clone()));
    assert_eq!(
        app.library.library_at(&snxlib).unwrap().observed_stamp,
        stamp
    );

    // A class edit leaves the stamp stale but flags it as the app's own.
    app.library.refresh_classes(&snxlib);
    assert!(app.library.library_at(&snxlib).unwrap().own_edit_unstamped);
    app.library.library_at_mut(&snxlib).unwrap().observed_stamp = None;
    let changed = libraries_changed(&app.library.watched_libraries());
    polled(&mut app, PollOutcome::Changed(changed));
    let lib = app.library.library_at(&snxlib).unwrap();
    assert!(!lib.own_edit_unstamped, "the refresh clears the mark");
    assert!(lib.observed_stamp.is_some());
}
