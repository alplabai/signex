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

    assert_eq!(
        libraries_changed(&state.watched_libraries()),
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
