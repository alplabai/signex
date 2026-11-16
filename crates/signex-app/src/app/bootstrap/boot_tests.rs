//! `Signex::boot` finishes the journalled recovery-copy moves before the
//! startup opens (#1540).

use crate::app::Signex;
use crate::autosave::{pending_move, sidecar};

#[test]
fn boot_finishes_the_journalled_moves_before_any_startup_open_can_run() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let recovery = tmp.path().join("recovery");
    let (old, new) = (
        tmp.path().join("Foo/a.snxsch"),
        tmp.path().join("Bar/a.snxsch"),
    );
    std::fs::create_dir_all(new.parent().expect("dir")).expect("Bar");
    std::fs::write(&new, b"saved").expect("sheet");
    sidecar::write(&recovery, &old, b"crash edits").expect("copy");
    pending_move::record(&recovery, &old, &new).expect("entry");

    // `boot` is `new` plus `boot_from`; `new` takes the real recovery folder,
    // so the test hands `boot_from` an app pointed at its own.
    let (mut app, boot_task) = Signex::new();
    app.document_state.autosave.set_dir(Some(recovery.clone()));
    let (_app, _tasks) = Signex::boot_from(app, boot_task, vec![tmp.path().join("Bar/a.snxsch")]);

    // `boot_from` has returned and no task has run, so no startup open has
    // been handled yet: the move is already done.
    assert!(!sidecar::sidecar_path(&recovery, &old).exists());
    assert_eq!(
        std::fs::read(sidecar::sidecar_path(&recovery, &new)).expect("moved"),
        b"crash edits"
    );
    assert!(pending_move::owed(&recovery).is_empty());
}

/// The window title reads as it did before the catalog (#2059): no mark
/// while nothing is unsaved, the dot and the count otherwise.
#[test]
fn the_window_title_names_the_version_and_the_unsaved_count() {
    let i18n = signex_i18n::I18n::en_us();
    let version = env!("CARGO_PKG_VERSION");
    let title = |count| super::main_window_title(&i18n, count);
    assert_eq!(title(0), format!("Signex {version}"));
    assert_eq!(title(1), format!("• Signex {version} — 1 unsaved"));
    assert_eq!(title(12), format!("• Signex {version} — 12 unsaved"));
}
