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
