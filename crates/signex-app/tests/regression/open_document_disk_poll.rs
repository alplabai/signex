//! #1877 -- an open document changed on disk is reported on the idle poll,
//! before the user saves, once per change.

use std::fs;
use std::path::Path;

use signex_app::app::{CaseProbeOutcome, FileMsg, Message, Signex};
use signex_app::diagnostics;
use signex_app::library::LibraryMessage;
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

fn open(app: &mut Signex, path: &Path) {
    let _ = diagnostics::init_logging();
    let _ = app.update(Message::File(FileMsg::DirCaseProbed {
        dir: path.parent().unwrap().to_path_buf(),
        answer: CaseProbeOutcome::Decided(false),
    }));
    crate::settle_support::settle(
        app,
        Message::File(FileMsg::Opened(Some(path.to_path_buf()))),
    );
}

fn poll(app: &mut Signex) {
    crate::settle_support::settle(
        app,
        Message::Library(LibraryMessage::ExternalEditsPollTicked),
    );
}

fn card_title(app: &Signex) -> Option<String> {
    let notice = app.document_state.error_notice.as_ref()?;
    notice.action.is_some().then(|| notice.title.clone())
}

#[test]
fn a_sheet_changed_outside_raises_the_card_on_the_poll_without_a_save() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Poll1877.snxsch");
    fs::write(&path, sheet_text("A4")).unwrap();
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);

    poll(&mut app);
    assert!(card_title(&app).is_none(), "an untouched file is quiet");

    fs::write(&path, sheet_text("A3")).unwrap();
    poll(&mut app);
    assert_eq!(card_title(&app).as_deref(), Some("File Changed on Disk"));
    assert_eq!(fs::read_to_string(&path).unwrap(), sheet_text("A3"));
}

#[test]
fn the_same_change_is_reported_once() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("PollOnce1877.snxsch");
    fs::write(&path, sheet_text("A4")).unwrap();
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    fs::write(&path, sheet_text("A3")).unwrap();
    poll(&mut app);
    app.document_state.error_notice = None; // the user dismissed the card

    poll(&mut app);

    assert!(
        card_title(&app).is_none(),
        "no second notice for one change"
    );
}

#[test]
fn a_deleted_sheet_raises_the_card_on_the_poll() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("PollGone1877.snxsch");
    fs::write(&path, sheet_text("A4")).unwrap();
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    fs::remove_file(&path).unwrap();

    poll(&mut app);

    assert_eq!(card_title(&app).as_deref(), Some("File Missing on Disk"));
}

#[test]
fn a_change_met_while_another_notice_shows_is_reported_once_the_slot_frees() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("PollBusy1877.snxsch");
    fs::write(&path, sheet_text("A4")).unwrap();
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    fs::write(&path, sheet_text("A3")).unwrap();
    poll(&mut app);
    // A second change while the first card is still up must not be lost.
    fs::write(&path, sheet_text("A2")).unwrap();
    poll(&mut app);
    app.document_state.error_notice = None;

    poll(&mut app);

    assert_eq!(card_title(&app).as_deref(), Some("File Changed on Disk"));
}

#[test]
fn a_poll_while_a_reload_reads_raises_no_second_card() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("PollReload.snxsch");
    fs::write(&path, sheet_text("A4")).unwrap();
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    fs::write(&path, sheet_text("A0")).unwrap();
    app.document_state
        .engines
        .get_mut(&path)
        .expect("engine")
        .execute(signex_engine::Command::SetPaperSize {
            paper_size: "A3".to_string(),
        })
        .expect("edit");
    app.document_state.mark_document_dirty(&path);
    crate::settle_support::settle(&mut app, Message::File(FileMsg::Save));
    assert_eq!(
        card_title(&app).as_deref(),
        Some("File Changed on Disk"),
        "save-time card"
    );

    // The user picks Reload; its read is still running when the poll ticks.
    let reload = app.update(Message::Overlay(
        signex_app::app::OverlayMsg::DiskConflictReloadChosen,
    ));
    poll(&mut app);
    let during = card_title(&app);
    crate::settle_support::settle_task(&mut app, reload);

    assert_eq!(
        app.document_state.engines[&path].document().paper_size,
        "A0",
        "reloaded"
    );
    assert!(
        during.is_none() && card_title(&app).is_none(),
        "a card for the change being reloaded: during the read {during:?}, after it {:?}",
        card_title(&app)
    );
}

#[test]
fn a_save_landing_during_a_poll_read_raises_no_card() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("PollSave.snxsch");
    fs::write(&path, sheet_text("A4")).unwrap();
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    app.document_state
        .engines
        .get_mut(&path)
        .expect("engine")
        .execute(signex_engine::Command::SetPaperSize {
            paper_size: "A3".to_string(),
        })
        .expect("edit");
    app.document_state.mark_document_dirty(&path);

    // The poll starts against the original baseline, the save lands, then the
    // poll's read of the pre-save file is delivered.
    let polling = app.update(Message::Library(LibraryMessage::ExternalEditsPollTicked));
    crate::settle_support::settle(&mut app, Message::File(FileMsg::Save));
    crate::settle_support::settle_task(&mut app, polling);

    assert!(
        card_title(&app).is_none(),
        "a stale read is dropped, not reported"
    );
}
