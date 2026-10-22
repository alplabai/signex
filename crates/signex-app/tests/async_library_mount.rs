//! Issue #99 part 2c — the off-thread `.snxlib` mount.
//!
//! Two things need pinning, and they fail in different ways.
//!
//! **The payload.** Moving the mount off the UI thread only helps if the
//! library that lands is the same one `open_library` used to build
//! inline. `prepare_mount_then_mount_prepared_matches_open_library` pins
//! that against the synchronous path itself rather than against
//! hand-written counts, so it keeps meaning something if the caches
//! change shape.
//!
//! **The bookkeeping.** The rest is about a map that has to survive a
//! race: a project auto-mount and a user double-click can ask for the
//! same `.snxlib` at once, and the user can close a library while its
//! preparation is still running. Those paths have no loud symptom when
//! they go wrong — a lost tab, a library mounted twice, or a closed
//! library quietly reappearing — so each one gets a test.

mod support;

use std::path::PathBuf;

use signex_app::library::mount::{MountIntent, MountRequest, prepare_mount};
use signex_app::library::state::LibraryState;

use support::{Scale, generate_library};

/// Small on purpose — this runs in the default `cargo test` (debug)
/// profile. Ten of each is enough to tell "populated" from "empty".
const SYMBOLS: usize = 10;
const FOOTPRINTS: usize = 10;

/// A real `.snxlib` written by the real Signex writers. Returns the
/// tempdir too — dropping it deletes the library out from under the test.
fn fixture(tag: &'static str) -> (tempfile::TempDir, PathBuf) {
    let scale = Scale::new(tag, SYMBOLS, FOOTPRINTS);
    let tmp = tempfile::Builder::new()
        .prefix("signex-async-mount-")
        .tempdir()
        .expect("tempdir");
    // `generate_library` returns the `.snxlib` FILE path, which is
    // exactly what `prepare_mount` wants — a directory fails at runtime
    // in `validate_file_path`, not at compile time.
    let snxlib = generate_library(tmp.path(), tag, &scale).expect("generate_library");
    (tmp, snxlib)
}

/// The whole point of the change: preparing off-thread and mounting the
/// result must leave `LibraryState` in the state the synchronous
/// `open_library` left it in.
///
/// Compared against the synchronous path rather than against literal
/// counts, so it still means something after the cache shape changes.
#[test]
fn prepare_mount_then_mount_prepared_matches_open_library() {
    let (_tmp, snxlib) = fixture("parity");

    // Arrange — the synchronous reference.
    let mut sync_state = LibraryState::default();
    sync_state
        .open_library(snxlib.clone())
        .expect("synchronous open_library");
    let reference = sync_state
        .library_at(&snxlib)
        .expect("mounted synchronously");
    let expected = (
        reference.display_name.clone(),
        reference.library_id,
        reference.tables.len(),
        reference.cached_components.len(),
        reference.cached_symbols.len(),
        reference.cached_footprints.len(),
        reference.cached_sims.len(),
    );

    // Act — the off-thread path, run inline (the `spawn_blocking` wrapper
    // adds no behaviour of its own).
    let prepared = prepare_mount(&snxlib).expect("prepare_mount");
    assert_eq!(
        prepared.path(),
        snxlib.as_path(),
        "the prepared mount must carry the path it was asked for"
    );
    let mut async_state = LibraryState::default();
    async_state
        .mount_prepared(prepared)
        .expect("mount_prepared");

    // Assert — same library, same five caches.
    let landed = async_state
        .library_at(&snxlib)
        .expect("library must be registered after mount_prepared");
    let actual = (
        landed.display_name.clone(),
        landed.library_id,
        landed.tables.len(),
        landed.cached_components.len(),
        landed.cached_symbols.len(),
        landed.cached_footprints.len(),
        landed.cached_sims.len(),
    );
    assert_eq!(
        actual, expected,
        "the off-thread mount must land the same library and the same primed caches as open_library"
    );
    assert_eq!(
        async_state.open_libraries.len(),
        1,
        "mount_prepared must register exactly one entry"
    );
    assert_eq!(
        async_state.expanded.len(),
        1,
        "the left-dock expanded flags must stay in lock-step with open_libraries"
    );
}

/// A cold path with nothing in flight must tell the caller to spawn, and
/// must record the request so a second caller can see it.
#[test]
fn request_mount_asks_for_a_spawn_once_then_reports_in_flight() {
    let (_tmp, snxlib) = fixture("inflight");
    let mut state = LibraryState::default();

    assert_eq!(
        state.request_mount(&snxlib, MountIntent::Silent),
        MountRequest::Spawn,
        "the first request for an unmounted library must ask the caller to spawn"
    );
    assert_eq!(
        state.request_mount(&snxlib, MountIntent::Silent),
        MountRequest::InFlight,
        "a second request must NOT ask for another spawn — two adapters would race onto one path"
    );
}

/// The subtle half of the design. A project auto-mount records `Silent`;
/// the user then double-clicks the same `.snxlib` while that preparation
/// is still running. The tab must still open, so the intent escalates.
#[test]
fn a_double_click_during_a_silent_mount_upgrades_the_intent() {
    let (_tmp, snxlib) = fixture("upgrade");
    let mut state = LibraryState::default();

    state.request_mount(&snxlib, MountIntent::Silent);
    state.request_mount(&snxlib, MountIntent::OpenBrowserTab);

    assert_eq!(
        state.take_mount_intent(&snxlib),
        Some(MountIntent::OpenBrowserTab),
        "the completion handler must see the upgraded intent, or the user's tab never opens"
    );
}

/// The reverse must not happen: a background auto-mount arriving after
/// the user asked for a tab must not silence it.
#[test]
fn a_silent_request_never_downgrades_an_open_browser_tab_intent() {
    let (_tmp, snxlib) = fixture("nodowngrade");
    let mut state = LibraryState::default();

    state.request_mount(&snxlib, MountIntent::OpenBrowserTab);
    state.request_mount(&snxlib, MountIntent::Silent);

    assert_eq!(
        state.take_mount_intent(&snxlib),
        Some(MountIntent::OpenBrowserTab),
        "intents only ever escalate; a later Silent request must not swallow the user's tab"
    );
}

/// An already-mounted library is neither a spawn nor an in-flight wait —
/// the caller has to act immediately. Collapsing this into either of the
/// other two outcomes loses the tab or opens it twice.
#[test]
fn request_mount_reports_already_mounted_and_records_nothing() {
    let (_tmp, snxlib) = fixture("warm");
    let mut state = LibraryState::default();
    state.open_library(snxlib.clone()).expect("cold mount");

    assert_eq!(
        state.request_mount(&snxlib, MountIntent::OpenBrowserTab),
        MountRequest::AlreadyMounted,
        "a mounted library must report AlreadyMounted so the caller finishes now"
    );
    assert_eq!(
        state.take_mount_intent(&snxlib),
        None,
        "AlreadyMounted must record nothing — there is no completion coming to consume it"
    );
}

/// Closing a library while its preparation is in flight has to cancel it.
/// Without the tombstone the completion would re-mount a library the user
/// just closed, and it would reappear with no gesture behind it.
#[test]
fn close_library_cancels_a_mount_still_being_prepared() {
    let (_tmp, snxlib) = fixture("cancel");
    let mut state = LibraryState::default();

    state.request_mount(&snxlib, MountIntent::OpenBrowserTab);
    state.close_library(&snxlib);

    assert_eq!(
        state.take_mount_intent(&snxlib),
        None,
        "close_library must remove the pending entry so the completion discards its payload"
    );
}

/// `take_mount_intent` consumes. A second completion for the same path
/// must not find an intent and act on it twice.
#[test]
fn take_mount_intent_is_one_shot() {
    let (_tmp, snxlib) = fixture("oneshot");
    let mut state = LibraryState::default();

    state.request_mount(&snxlib, MountIntent::Silent);

    assert_eq!(state.take_mount_intent(&snxlib), Some(MountIntent::Silent));
    assert_eq!(
        state.take_mount_intent(&snxlib),
        None,
        "the intent must be consumed, so a duplicate completion is a no-op"
    );
}

/// `mount_prepared` carries the same idempotence guard as
/// `open_library`: a synchronous call site can mount the same path while
/// a preparation is in flight, and the late arrival must not duplicate
/// the entry.
#[test]
fn mount_prepared_does_not_duplicate_an_already_mounted_library() {
    let (_tmp, snxlib) = fixture("idempotent");

    let prepared = prepare_mount(&snxlib).expect("prepare_mount");
    let mut state = LibraryState::default();
    // Something else won the race and mounted it synchronously.
    state
        .open_library(snxlib.clone())
        .expect("synchronous mount");

    state
        .mount_prepared(prepared)
        .expect("a late arrival must be tolerated, not an error");

    assert_eq!(
        state.open_libraries.len(),
        1,
        "the late prepared mount must be dropped, not pushed as a second entry"
    );
    assert_eq!(
        state.expanded.len(),
        1,
        "expanded flags must not drift from open_libraries"
    );
}
