//! #1213 / #1214 / #1215 — Add Existing with a `.snxlib`: the mount failure
//! path, a bound row surviving an in-place add, and the project's volume case
//! rule deciding what "the same library" and "inside the project" mean.

#[path = "../../../../../tests/support/mod.rs"]
mod support;

use super::add_tests::{add_existing, run_task, two_projects};
use crate::app::state::scope::CaseFold;
use crate::app::{Message, ProjectMsg};
use signex_types::project::LibraryEntryKind;

#[test]
fn a_library_that_fails_to_mount_reaches_the_messages_panel() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, d1, _d2) = two_projects();
    let broken = d1.path().join("libs").join("Broken.snxlib");
    std::fs::create_dir_all(broken.parent().expect("parent")).expect("mkdir");
    std::fs::write(&broken, b"this is not a library").expect("write");

    add_existing(&mut app, broken.clone());

    assert_eq!(
        app.document_state.projects[0].data.libraries.len(),
        1,
        "the entry stays registered so the user can fix the file"
    );
    assert!(app.library.library_at(&broken).is_none());
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Failed to mount library")
                && e.message.contains(&broken.display().to_string())),
        "the mount failure must reach the Messages panel"
    );
}

/// #1214: copying only the `.snxlib` left its rows pointing at primitives that
/// were never copied. Registered in place, every row still resolves its symbol.
#[test]
fn a_row_bound_to_a_symbol_still_resolves_after_an_outside_library_is_added() {
    let (mut app, _d1, _d2) = two_projects();
    let outside = tempfile::tempdir().expect("outside dir");
    let scale = support::Scale::new("bound", 3, 3);
    let lib = support::generate_library(outside.path(), "bound", &scale).expect("generate");

    add_existing(&mut app, lib.clone());

    let mounted = app.library.library_at(&lib).expect("mounted in place");
    let rows: Vec<_> = mounted.tables.values().flatten().collect();
    assert_eq!(rows.len(), 3);
    for row in rows {
        assert!(
            mounted
                .cached_symbols
                .iter()
                .any(|s| s.uuid == row.symbol_ref.uuid),
            "row {:?} lost its symbol",
            row.internal_pn
        );
    }
}

/// One file spelled two ways on a case-insensitive volume is one library, not
/// two mounts over one `.snxlib` (save-overwrite risk). The project is given
/// the insensitive rule explicitly so this holds on any host filesystem.
#[test]
fn a_case_variant_of_a_registered_library_is_not_registered_twice() {
    let (mut app, d1, _d2) = two_projects();
    app.document_state.projects[0].case_fold = CaseFold::case_only();
    let lib = support::generate_library(
        &d1.path().join("libs"),
        "Lib",
        &support::Scale::new("c", 1, 1),
    )
    .expect("generate");
    add_existing(&mut app, lib);

    // Handed straight to registration: on a case-sensitive host the variant
    // does not exist on disk, and registration must not need it to.
    let variant = d1.path().join("LIBS").join("Lib").join("Lib.snxlib");
    let task = app.update(Message::Project(ProjectMsg::AddExistingFilesCopied {
        project_id: app.document_state.projects[0].id,
        results: Ok(vec![Ok(variant)]),
    }));
    run_task(&mut app, task);

    let data = &app.document_state.projects[0].data;
    assert_eq!(data.libraries.len(), 1, "{:?}", data.libraries);
    assert_eq!(data.libraries[0].kind, LibraryEntryKind::ProjectLocal);
    let in_project = app
        .library
        .open_libraries
        .iter()
        .filter(|l| l.root.starts_with(d1.path()))
        .count();
    assert_eq!(in_project, 1, "one mount over the file, not two");
}
