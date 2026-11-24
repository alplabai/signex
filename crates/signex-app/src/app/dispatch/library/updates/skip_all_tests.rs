//! Skip All against Cancel in the Library Updates modal (mockup row 135,
//! accepted 2026-11-24): Skip All remembers the offered versions per sheet,
//! Cancel forgets nothing.

use std::path::PathBuf;

use crate::app::{Message, Signex};
use crate::library::messages::LibraryMessage;
use crate::library::updates_dialog::{BumpKind, LibraryUpdateEntry};
use signex_library::RowId;
use uuid::Uuid;

fn entry(lib_id: &str, latest: &str) -> LibraryUpdateEntry {
    LibraryUpdateEntry {
        symbol_uuid: Uuid::now_v7(),
        ref_des: "R1".to_string(),
        internal_pn: String::new(),
        lib_id: lib_id.to_string(),
        library_id: Uuid::nil(),
        library_name: "Lib".to_string(),
        row_id: RowId::from_uuid(Uuid::nil()),
        library_path: PathBuf::from("/tmp/x.snxlib"),
        current_version: "1.0.0".to_string(),
        latest_version: latest.to_string(),
        bump_kind: BumpKind::Patch,
        selected: false,
        conflicts: Vec::new(),
    }
}

fn scanned(app: &mut Signex, sheet: &std::path::Path, entries: Vec<LibraryUpdateEntry>) {
    app.record_scan_outcome(sheet.to_path_buf(), entries, false, false);
}

#[test]
fn skip_all_keeps_the_skipped_version_out_of_the_next_scan() {
    let (mut app, _boot) = Signex::new();
    let sheet = PathBuf::from("/p/a.snxsch");
    scanned(&mut app, &sheet, vec![entry("L:R", "1.0.1")]);
    assert!(app.library.library_updates.is_some(), "the modal opens");

    let _ = app.update(Message::Library(LibraryMessage::LibraryUpdatesSkipAll));
    assert!(app.library.library_updates.is_none());

    scanned(&mut app, &sheet, vec![entry("L:R", "1.0.1")]);
    assert!(
        app.library.library_updates.is_none(),
        "the skipped version is not offered again"
    );
    assert!(
        app.library.skipped_updates_for.contains(&sheet),
        "the skipped update is still pending"
    );

    scanned(&mut app, &sheet, vec![entry("L:R", "1.0.2")]);
    let open = app
        .library
        .library_updates
        .as_ref()
        .expect("a newer version is offered");
    assert_eq!(open.entries.len(), 1);
    assert_eq!(open.entries[0].latest_version, "1.0.2");
}

#[test]
fn a_skipped_part_leaves_the_other_parts_in_the_modal() {
    let (mut app, _boot) = Signex::new();
    let sheet = PathBuf::from("/p/a.snxsch");
    scanned(&mut app, &sheet, vec![entry("L:R", "1.0.1")]);
    let _ = app.update(Message::Library(LibraryMessage::LibraryUpdatesSkipAll));

    scanned(
        &mut app,
        &sheet,
        vec![entry("L:R", "1.0.1"), entry("L:C", "2.0.0")],
    );
    let open = app.library.library_updates.as_ref().expect("a modal");
    let lib_ids: Vec<&str> = open.entries.iter().map(|e| e.lib_id.as_str()).collect();
    assert_eq!(lib_ids, ["L:C"]);
    assert!(
        open.scan_failed,
        "Apply must not clear the pending flag the skipped part still holds"
    );
}

#[test]
fn skip_all_on_one_sheet_leaves_another_sheet_asking() {
    let (mut app, _boot) = Signex::new();
    let a = PathBuf::from("/p/a.snxsch");
    let b = PathBuf::from("/p/b.snxsch");
    scanned(&mut app, &a, vec![entry("L:R", "1.0.1")]);
    let _ = app.update(Message::Library(LibraryMessage::LibraryUpdatesSkipAll));

    scanned(&mut app, &b, vec![entry("L:R", "1.0.1")]);
    let open = app.library.library_updates.as_ref().expect("a modal");
    assert_eq!(open.schematic_path, b);
}

#[test]
fn cancel_forgets_nothing_and_the_next_scan_asks_again() {
    let (mut app, _boot) = Signex::new();
    let sheet = PathBuf::from("/p/a.snxsch");
    scanned(&mut app, &sheet, vec![entry("L:R", "1.0.1")]);
    let _ = app.update(Message::Library(LibraryMessage::LibraryUpdatesCancel));
    assert!(app.library.library_updates.is_none());

    scanned(&mut app, &sheet, vec![entry("L:R", "1.0.1")]);
    assert!(
        app.library.library_updates.is_some(),
        "Cancel does not skip the version"
    );
}
