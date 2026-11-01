//! #1826 / #1827 / #1828 — a document save must write the file the document
//! is, and must not overwrite or invent what the user did not mean.
//!
//! Real files in a temp folder, the production `AtomicFileSink`, and every
//! task driven to completion the way the iced runtime would.

use std::fs;
use std::path::{Path, PathBuf};

use signex_app::app::{CaseProbeOutcome, FileMsg, Message, OverlayMsg, Signex, TabKind};
use signex_app::diagnostics;
use signex_app::menu_bar::MenuMessage;
use tempfile::TempDir;

fn sheet_text(paper: &str) -> String {
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.9".into(),
        paper_size: paper.into(),
        ..Default::default()
    };
    signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise sheet")
}

/// Write a sheet, open it through the real async open and settle.
fn open(app: &mut Signex, path: &Path) {
    let _ = diagnostics::init_logging();
    if let Some(dir) = path.parent() {
        let _ = app.update(Message::File(FileMsg::DirCaseProbed {
            dir: dir.to_path_buf(),
            answer: CaseProbeOutcome::Decided(false),
        }));
    }
    crate::settle_support::settle(
        app,
        Message::File(FileMsg::Opened(Some(path.to_path_buf()))),
    );
}

fn edit(app: &mut Signex, path: &Path, paper: &str) {
    app.document_state
        .engines
        .get_mut(path)
        .expect("engine")
        .execute(signex_engine::Command::SetPaperSize {
            paper_size: paper.to_string(),
        })
        .expect("edit");
    app.document_state.mark_document_dirty(path);
}

fn save(app: &mut Signex) {
    crate::settle_support::settle(app, Message::File(FileMsg::Save));
}

fn messages_mention(needle: &str) -> bool {
    diagnostics::recent_entries()
        .iter()
        .any(|entry| entry.message.contains(needle))
}

fn fixture(name: &str, paper: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join(name);
    fs::write(&path, sheet_text(paper)).expect("write sheet");
    (dir, path)
}

// ---- #1828 -----------------------------------------------------------

#[test]
fn save_after_the_folder_moved_does_not_recreate_it_and_stays_dirty() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("Proj");
    fs::create_dir(&project).unwrap();
    let path = project.join("MovedAway1828.snxsch");
    fs::write(&path, sheet_text("A4")).unwrap();
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    edit(&mut app, &path, "A3");
    fs::rename(&project, dir.path().join("Proj_moved")).unwrap();

    save(&mut app);

    assert!(!project.exists(), "the old folder must not be re-created");
    assert!(
        app.document_state.is_document_dirty(&path),
        "a refused save keeps the sheet modified"
    );
    assert!(
        messages_mention("MovedAway1828.snxsch"),
        "the refusal must reach Messages"
    );
}

#[test]
fn save_all_after_the_folder_moved_does_not_recreate_it_and_stays_dirty() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("Proj");
    fs::create_dir(&project).unwrap();
    let path = project.join("MovedAway1828b.snxsch");
    fs::write(&path, sheet_text("A4")).unwrap();
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    edit(&mut app, &path, "A3");
    fs::rename(&project, dir.path().join("Proj_moved")).unwrap();

    crate::settle_support::settle(&mut app, Message::Menu(MenuMessage::SaveAll));

    assert!(!project.exists(), "Save All must not re-create the folder");
    assert!(app.document_state.is_document_dirty(&path));
}

// ---- #1827 -----------------------------------------------------------

#[cfg(unix)]
#[test]
fn save_through_a_symlinked_sheet_keeps_the_link_and_updates_the_target() {
    let (dir, real) = fixture("Real1827.snxsch", "A4");
    let link = dir.path().join("Link1827.snxsch");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let (mut app, _boot) = Signex::new();
    open(&mut app, &link);
    edit(&mut app, &link, "A3");

    save(&mut app);

    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the symlink must survive a save"
    );
    assert!(
        fs::read_to_string(&real).unwrap().contains("A3"),
        "the link target must receive the edit"
    );
}

#[cfg(unix)]
#[test]
fn opening_a_symlink_to_an_open_sheet_shows_the_open_document() {
    let (dir, real) = fixture("Real1827b.snxsch", "A4");
    let link = dir.path().join("Link1827b.snxsch");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let (mut app, _boot) = Signex::new();
    open(&mut app, &real);
    open(&mut app, &link);

    let sheets = app
        .document_state
        .tabs
        .iter()
        .filter(|tab| matches!(tab.kind, TabKind::Schematic))
        .count();
    assert_eq!(sheets, 1, "one file is one document");
    assert_eq!(app.document_state.engines.len(), 1, "and one engine");
}

// ---- #1826 -----------------------------------------------------------

fn conflict_card_is_up(app: &Signex) -> bool {
    app.document_state
        .error_notice
        .as_ref()
        .is_some_and(|notice| notice.action.is_some())
}

#[test]
fn save_over_an_externally_changed_sheet_asks_first_and_writes_nothing() {
    let (_dir, path) = fixture("Changed1826.snxsch", "A4");
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    let theirs = sheet_text("A0");
    fs::write(&path, &theirs).unwrap();
    edit(&mut app, &path, "A3");

    save(&mut app);

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        theirs,
        "their change survives"
    );
    assert!(app.document_state.is_document_dirty(&path));
    assert!(conflict_card_is_up(&app), "the user is asked");
    assert!(messages_mention("Changed1826.snxsch"));
}

#[test]
fn overwrite_writes_the_document_over_the_external_change() {
    let (_dir, path) = fixture("Overwrite1826.snxsch", "A4");
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    fs::write(&path, sheet_text("A0")).unwrap();
    edit(&mut app, &path, "A3");
    save(&mut app);

    crate::settle_support::settle(
        &mut app,
        Message::Overlay(OverlayMsg::DiskConflictOverwriteChosen),
    );

    assert!(fs::read_to_string(&path).unwrap().contains("A3"));
    assert!(!app.document_state.is_document_dirty(&path));
    // The next save is compared with what was just written, so it goes through.
    edit(&mut app, &path, "A2");
    save(&mut app);
    assert!(fs::read_to_string(&path).unwrap().contains("A2"));
    assert!(!conflict_card_is_up(&app));
}

#[test]
fn reload_replaces_the_document_with_the_file_and_drops_the_edits() {
    let (_dir, path) = fixture("Reload1826.snxsch", "A4");
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    let theirs = sheet_text("A0");
    fs::write(&path, &theirs).unwrap();
    edit(&mut app, &path, "A3");
    save(&mut app);

    crate::settle_support::settle(
        &mut app,
        Message::Overlay(OverlayMsg::DiskConflictReloadChosen),
    );

    assert_eq!(
        app.document_state.engines[&path].document().paper_size,
        "A0"
    );
    assert!(!app.document_state.is_document_dirty(&path));
    assert_eq!(fs::read_to_string(&path).unwrap(), theirs);
}

#[test]
fn cancelling_the_card_keeps_the_edits_and_the_file() {
    let (_dir, path) = fixture("Cancel1826.snxsch", "A4");
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    let theirs = sheet_text("A0");
    fs::write(&path, &theirs).unwrap();
    edit(&mut app, &path, "A3");
    save(&mut app);

    let _ = app.update(Message::Overlay(OverlayMsg::DismissErrorNotice));

    assert_eq!(
        app.document_state.engines[&path].document().paper_size,
        "A3"
    );
    assert!(app.document_state.is_document_dirty(&path));
    assert_eq!(fs::read_to_string(&path).unwrap(), theirs);
}

#[test]
fn save_after_the_sheet_file_was_deleted_asks_instead_of_recreating_it() {
    let (_dir, path) = fixture("Deleted1826.snxsch", "A4");
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    fs::remove_file(&path).unwrap();
    edit(&mut app, &path, "A3");

    save(&mut app);

    assert!(!path.exists(), "a deleted sheet is not silently re-created");
    assert!(app.document_state.is_document_dirty(&path));
    assert!(conflict_card_is_up(&app));
}

#[test]
fn two_quick_saves_of_an_untouched_file_are_not_drift() {
    let (_dir, path) = fixture("Quick1826.snxsch", "A4");
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    edit(&mut app, &path, "A3");
    save(&mut app);
    edit(&mut app, &path, "A2");
    save(&mut app);

    assert!(fs::read_to_string(&path).unwrap().contains("A2"));
    assert!(!conflict_card_is_up(&app));
    assert!(!app.document_state.is_document_dirty(&path));
}

#[test]
fn a_save_of_an_unchanged_file_after_open_goes_through() {
    let (_dir, path) = fixture("Plain1826.snxsch", "A4");
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    edit(&mut app, &path, "A3");

    save(&mut app);

    assert!(fs::read_to_string(&path).unwrap().contains("A3"));
    assert!(!conflict_card_is_up(&app));
}
