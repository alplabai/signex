//! Both halves of the #99 part-2a cache contract for
//! `auto_mount_project_libraries`.
//!
//! **Cold path.** `auto_mount` no longer chases each `open_library` with a
//! `refresh_components` that recomputed the identical five fields off the
//! identical adapter calls — that duplicate roughly doubled every project
//! open. `open_library` is now the sole thing standing between a mounted
//! library and a populated panel, so
//! `open_library_primes_every_cache` pins that it fills all five, and
//! `refresh_components_after_open_changes_nothing` pins that the dropped
//! call really was a no-op there.
//!
//! **Warm path.** `open_library` early-returns at
//! `library/state/methods.rs:83-85` when the library is already mounted and
//! never reaches `reload_tables`, so for an already-open library the
//! refresh is the *only* thing that rescans anything.
//! `auto_mount_refreshes_an_already_mounted_library` pins that — it is the
//! case a blanket deletion would have silently broken.
//!
//! That test also pins the limit of what the refresh can do: it rescans the
//! primitive *directories*, but cannot see rows added to the `.snxlib`
//! itself, because the mounted `LocalGitAdapter` serves tables from an
//! in-memory parse taken at `open()`. See the comments on its assertions.
//!
//! Between them: trimming `reload_tables` / `reload_primitives` out of the
//! open path fails here, and so does deleting the warm-path refresh, rather
//! than the UI silently rendering a stale or empty library.

mod support;

use std::path::{Path, PathBuf};

use signex_app::library::commands::auto_mount_project_libraries;
use signex_app::library::state::LibraryState;
use signex_types::project::{LibraryEntry, LibraryEntryKind, ProjectData};

use support::{Scale, append_component, generate_library};

/// Deliberately small — this runs in the default `cargo test` (debug)
/// profile, where generation is >10× slower than release. Ten of each is
/// enough to tell "populated" from "empty", which is all the test claims.
const SYMBOLS: usize = 10;
const FOOTPRINTS: usize = 10;

#[test]
fn open_library_primes_every_cache() {
    // Arrange — a real `.snxlib` written by the real Signex writers.
    let scale = Scale::new("cache", SYMBOLS, FOOTPRINTS);
    assert!(
        scale.sims > 0,
        "scale must generate at least one sim, else the cached_sims assertion is vacuous"
    );
    let tmp = tempfile::Builder::new()
        .prefix("signex-open-cache-")
        .tempdir()
        .expect("tempdir");
    // NOTE: `generate_library` returns the `.snxlib` FILE path, and
    // `open_library` wants exactly that — handing it the directory fails at
    // runtime in `validate_file_path`, not at compile time.
    let snxlib = generate_library(tmp.path(), "cache", &scale).expect("generate_library");

    // Act — the open path on its own, with no `refresh_components` chaser.
    let mut state = LibraryState::default();
    state
        .open_library(snxlib.clone())
        .expect("open_library should mount the generated library");

    // Assert — all five caches, by count, not merely non-empty.
    let lib = state
        .library_at(&snxlib)
        .expect("library should be registered in open_libraries");

    assert_eq!(
        lib.tables.keys().collect::<Vec<_>>(),
        vec!["ICs"],
        "open_library must populate `tables` via reload_tables"
    );
    assert_eq!(
        lib.tables.get("ICs").map(Vec::len),
        Some(SYMBOLS),
        "every generated component row must be cached"
    );
    assert_eq!(
        lib.cached_components.len(),
        SYMBOLS,
        "open_library must populate `cached_components` (picker summary tier)"
    );
    assert_eq!(
        lib.cached_symbols.len(),
        scale.symbols,
        "open_library must populate `cached_symbols` via reload_primitives"
    );
    assert_eq!(
        lib.cached_footprints.len(),
        scale.footprints,
        "open_library must populate `cached_footprints` via reload_primitives"
    );
    assert_eq!(
        lib.cached_sims.len(),
        scale.sims,
        "open_library must populate `cached_sims` via reload_primitives"
    );
}

/// The stronger claim behind the deletion: a `refresh_components` run
/// straight after `open_library` is a no-op on the cache contents. If this
/// ever stops holding, the dropped call in `auto_mount_project_libraries`
/// was load-bearing after all.
#[test]
fn refresh_components_after_open_changes_nothing() {
    let scale = Scale::new("noop", SYMBOLS, FOOTPRINTS);
    let tmp = tempfile::Builder::new()
        .prefix("signex-open-noop-")
        .tempdir()
        .expect("tempdir");
    let snxlib = generate_library(tmp.path(), "noop", &scale).expect("generate_library");

    let mut state = LibraryState::default();
    state.open_library(snxlib.clone()).expect("open_library");

    let before = snapshot(&state, &snxlib);
    state
        .refresh_components(&snxlib)
        .expect("refresh_components on a mounted library");
    let after = snapshot(&state, &snxlib);

    assert_eq!(
        before, after,
        "refresh_components recomputed the same five caches open_library had already filled"
    );
}

/// The warm path: a library that is *already* mounted when `auto_mount`
/// reaches it. `open_library` early-returns without reloading anything, so
/// the `already_open` refresh in `auto_mount_project_libraries` is the only
/// thing that can notice an out-of-band edit.
///
/// Deleting that refresh makes this test fail — which is the whole reason
/// it is not deleted. It also documents exactly how far the refresh
/// reaches: primitive directories yes, `.snxlib` rows no.
#[test]
fn auto_mount_refreshes_an_already_mounted_library() {
    // Arrange — project A mounts the library (the cold path).
    let scale = Scale::new("warm", SYMBOLS, FOOTPRINTS);
    let tmp = tempfile::Builder::new()
        .prefix("signex-warm-mount-")
        .tempdir()
        .expect("tempdir");
    let snxlib = generate_library(tmp.path(), "warm", &scale).expect("generate_library");

    let mut state = LibraryState::default();
    state.open_library(snxlib.clone()).expect("cold mount");
    assert_eq!(
        state
            .library_at(&snxlib)
            .expect("mounted")
            .cached_components
            .len(),
        SYMBOLS,
        "cold mount should have cached exactly what was generated"
    );

    // Act 1 — someone edits the library on disk behind our back.
    let new_pn = append_component(&snxlib, SYMBOLS).expect("append_component");

    // Act 2 — project B references the same `.snxlib`. `self.library` is
    // app-global, so this hits the already-mounted branch.
    let project = project_referencing(tmp.path(), std::slice::from_ref(&snxlib));
    let outcome = auto_mount_project_libraries(&mut state, &project);

    // Assert — the library is still mounted exactly once, and the caches
    // that *can* see the edit have seen it.
    assert_eq!(
        outcome.refreshed, 1,
        "the already-mounted library takes the warm path and is refreshed in place"
    );
    // Since #99 part 2c a cold mount is only *recorded* here and prepared
    // off-thread. An already-mounted library must never land in that
    // queue: doing so would re-open an adapter for a library that is
    // already resolvable, and the refresh above would be dead work.
    assert!(
        outcome.pending.is_empty(),
        "an already-mounted library must not be recorded as a pending cold mount, got {:?}",
        outcome.pending
    );
    assert_eq!(
        state.open_libraries.len(),
        1,
        "re-mounting must not duplicate the entry"
    );

    let lib = state.library_at(&snxlib).expect("still mounted");

    // The primitive caches DO refresh: `list_symbols` / `list_footprints` /
    // `list_sims` walk `symbols/` / `footprints/` / `sims/` on every call
    // (`adapters/local_git/primitives.rs::list_primitive_summaries`), so
    // they see files another process wrote. This is the payload of the
    // warm-path refresh — delete it and these two assertions fail.
    assert_eq!(
        lib.cached_symbols.len(),
        scale.symbols + 1,
        "warm auto-mount must rescan symbols/ and pick up the new .snxsym"
    );
    assert_eq!(
        lib.cached_footprints.len(),
        scale.footprints + 1,
        "warm auto-mount must rescan footprints/ and pick up the new .snxfpt"
    );

    // …and the row caches do NOT, which is a real limitation, pinned here
    // deliberately rather than papered over.
    //
    // `LocalGitAdapter` parses the `.snxlib` once at `open()` into
    // `RwLock<LibraryFile>` (`adapters/local_git/mod.rs:88-92`); `list_tables`
    // and `read_table`/`snapshot_table` serve from that in-memory copy and
    // never re-stat the file. `refresh_components` reaches the library
    // through `set.get(library_id)` — the adapter mounted at cold-open time
    // — so a row added to the `.snxlib` by another process is invisible to
    // it. Only re-opening the adapter sees it; `app/handlers/document_files/
    // history.rs:259` is the one place that does, via a `fresh_adapter`.
    //
    // If someone makes the adapter re-read on refresh (or `auto_mount`
    // remount), these two assertions flip to `SYMBOLS + 1` and this test
    // fails — which is the correct signal, not a regression.
    assert_eq!(
        lib.tables.get("ICs").map(Vec::len),
        Some(SYMBOLS),
        "known limitation: the mounted adapter serves .snxlib rows from its \
         in-memory snapshot, so an externally-added row is not visible"
    );
    assert_eq!(
        lib.cached_components.len(),
        SYMBOLS,
        "known limitation: `cached_components` is built from the same \
         in-memory table snapshot"
    );
    assert!(
        !lib.cached_components
            .iter()
            .any(|c| c.internal_pn.to_string() == new_pn),
        "the externally-added row {new_pn} is expected NOT to appear until \
         the adapter is re-opened"
    );
}

/// A `ProjectData` whose `libraries` are absolute `Shared` entries, so
/// `resolve_library_path` hands back exactly the paths given —
/// `open_library` matches its already-open check on `==`, so any rewriting
/// here would silently turn the warm path back into a cold one.
fn project_referencing(dir: &Path, libs: &[PathBuf]) -> ProjectData {
    ProjectData {
        name: "warm".to_string(),
        dir: dir.to_string_lossy().to_string(),
        schematic_root: None,
        pcb_file: None,
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: libs
            .iter()
            .map(|p| LibraryEntry {
                path: p.clone(),
                kind: LibraryEntryKind::Shared,
                library_id: None,
            })
            .collect(),
        enable_git: false,
    }
}

/// Order-independent fingerprint of the five cached fields — row ids,
/// summary part numbers and primitive uuids. Enough to catch a field that
/// only one of the two paths populates.
fn snapshot(state: &LibraryState, snxlib: &Path) -> Vec<String> {
    let lib = state.library_at(snxlib).expect("library mounted");
    let mut out = Vec::new();
    for (table, rows) in &lib.tables {
        for row in rows {
            out.push(format!("table:{table}:{}", row.row_id));
        }
    }
    for c in &lib.cached_components {
        out.push(format!("component:{}", c.internal_pn));
    }
    for s in &lib.cached_symbols {
        out.push(format!("symbol:{}", s.uuid));
    }
    for f in &lib.cached_footprints {
        out.push(format!("footprint:{}", f.uuid));
    }
    for s in &lib.cached_sims {
        out.push(format!("sim:{}", s.uuid));
    }
    out.sort();
    out
}
