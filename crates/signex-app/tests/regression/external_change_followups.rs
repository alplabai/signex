//! #1863 / #1864 / #1865 -- follow-ups to the external-change save check
//! (#1826): boards are compared too, a Reload does not drop edits made while
//! it reads, a gone folder is said so, and the disk records follow the
//! document (rename, Save As, close).
//!
//! Real files in a temp folder and every task driven to completion.

use std::fs;
use std::path::{Path, PathBuf};

use signex_app::app::{
    CaseProbeOutcome, ContextMenuMsg, FileMsg, Message, OverlayMsg, Signex, TabContextAction,
    TabDocument,
};
use signex_app::diagnostics;
use signex_types::pcb::PcbBoard;
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

fn board(thickness: f64) -> PcbBoard {
    PcbBoard {
        unknown: Default::default(),
        uuid: uuid::Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}

fn board_text(thickness: f64) -> String {
    signex_types::format::SnxPcb::new(board(thickness))
        .write_string()
        .expect("serialise board")
}

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

fn edit_board(app: &mut Signex, path: &Path, thickness: f64) {
    let tab = app
        .document_state
        .tabs
        .iter_mut()
        .find(|tab| tab.path == path)
        .expect("pcb tab");
    tab.cached_document = Some(TabDocument::Pcb(board(thickness)));
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

fn card_title(app: &Signex) -> Option<String> {
    let notice = app.document_state.error_notice.as_ref()?;
    notice.action.is_some().then(|| notice.title.clone())
}

fn fixture(name: &str, text: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join(name);
    fs::write(&path, text).expect("write fixture");
    (dir, path)
}

// ---- #1863: board saves are compared --------------------------------------

#[test]
fn a_board_changed_outside_is_not_overwritten_by_a_save() {
    let (_dir, path) = fixture("Board1863.snxpcb", &board_text(1.6));
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    let theirs = board_text(3.2);
    fs::write(&path, &theirs).unwrap();
    edit_board(&mut app, &path, 2.4);

    save(&mut app);

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        theirs,
        "theirs survives"
    );
    assert!(app.document_state.is_document_dirty(&path));
    assert_eq!(card_title(&app).as_deref(), Some("File Changed on Disk"));
    assert!(messages_mention("Board1863.snxpcb"));
}

#[test]
fn overwriting_a_changed_board_writes_the_open_one_and_the_next_save_goes_through() {
    let (_dir, path) = fixture("BoardOver1863.snxpcb", &board_text(1.6));
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    fs::write(&path, board_text(3.2)).unwrap();
    edit_board(&mut app, &path, 2.4);
    save(&mut app);

    crate::settle_support::settle(
        &mut app,
        Message::Overlay(OverlayMsg::DiskConflictOverwriteChosen),
    );

    assert_eq!(fs::read_to_string(&path).unwrap(), board_text(2.4));
    edit_board(&mut app, &path, 2.0);
    save(&mut app);
    assert_eq!(fs::read_to_string(&path).unwrap(), board_text(2.0));
    assert!(card_title(&app).is_none(), "no second card");
}

#[test]
fn reloading_a_changed_board_loads_the_file_and_drops_the_edits() {
    let (_dir, path) = fixture("BoardReload1863.snxpcb", &board_text(1.6));
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    fs::write(&path, board_text(3.2)).unwrap();
    edit_board(&mut app, &path, 2.4);
    save(&mut app);

    crate::settle_support::settle(
        &mut app,
        Message::Overlay(OverlayMsg::DiskConflictReloadChosen),
    );

    let shown = app
        .document_state
        .tabs
        .iter()
        .find(|tab| tab.path == path)
        .and_then(|tab| tab.cached_document.as_ref())
        .and_then(TabDocument::as_pcb)
        .map(|b| b.thickness);
    assert_eq!(shown, Some(3.2));
    assert!(!app.document_state.is_document_dirty(&path));
}

// ---- #1864: reload keeps edits made during its read ------------------------

#[test]
fn an_edit_made_while_reload_reads_is_kept_and_the_card_returns() {
    let (_dir, path) = fixture("ReloadRace1864.snxsch", &sheet_text("A4"));
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    fs::write(&path, sheet_text("A0")).unwrap();
    edit(&mut app, &path, "A3");
    save(&mut app);

    let task = app.update(Message::Overlay(OverlayMsg::DiskConflictReloadChosen));
    edit(&mut app, &path, "A2");
    crate::settle_support::settle_task(&mut app, task);

    assert_eq!(
        app.document_state.engines[&path].document().paper_size,
        "A2",
        "the edit made during the read must survive"
    );
    assert!(app.document_state.is_document_dirty(&path));
    assert_eq!(card_title(&app).as_deref(), Some("File Changed on Disk"));
    assert!(messages_mention("edited while it reloaded"));
}

// ---- #1864: a gone folder is not a missing file -----------------------------

#[test]
fn a_save_after_the_folder_vanished_says_so_and_offers_no_recreate() {
    let dir = TempDir::new().unwrap();
    let folder = dir.path().join("Proj");
    fs::create_dir(&folder).unwrap();
    let path = folder.join("Gone1864.snxsch");
    fs::write(&path, sheet_text("A4")).unwrap();
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    edit(&mut app, &path, "A3");
    fs::remove_dir_all(&folder).unwrap();

    save(&mut app);

    assert_eq!(card_title(&app).as_deref(), Some("Folder Missing on Disk"));
    assert!(!folder.exists(), "the folder must not be re-created");
    assert!(app.document_state.is_document_dirty(&path));
}

// ---- #1865: disk records follow the document --------------------------------

#[test]
fn closing_a_tab_forgets_its_baseline() {
    let (_dir, path) = fixture("Close1865.snxsch", &sheet_text("A4"));
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    assert!(app.document_state.document_saves.baseline(&path).is_some());

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(0),
    )));

    assert!(
        app.document_state.document_saves.baseline(&path).is_none(),
        "a later engine for the file must not compare with a stale baseline"
    );
}

#[test]
fn closing_a_dirty_tab_keeps_the_parked_drafts_baseline() {
    let (_dir, path) = fixture("Parked1865.snxsch", &sheet_text("A4"));
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    edit(&mut app, &path, "A3");

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(0),
    )));

    assert!(app.document_state.engines.contains_key(&path));
    assert!(app.document_state.document_saves.baseline(&path).is_some());
}

#[cfg(unix)]
#[test]
fn a_symlink_to_a_renamed_sheet_shows_the_open_tab() {
    let mut f = crate::rename_preserves_references::fixture();
    let child = f.child.clone();
    open(&mut f.app, &child);
    crate::rename_preserves_references::submit_rename(&mut f.app, &child, "renamed.snxsch");
    let renamed = child.with_file_name("renamed.snxsch");
    let link = child.with_file_name("Link1865.snxsch");
    std::os::unix::fs::symlink(&renamed, &link).unwrap();

    open(&mut f.app, &link);

    let tabs = f
        .app
        .document_state
        .tabs
        .iter()
        .filter(|tab| tab.path == renamed || tab.path == link)
        .count();
    assert_eq!(
        tabs, 1,
        "the link is the renamed sheet, not a second document"
    );
}

#[cfg(unix)]
#[test]
fn a_symlink_to_a_save_as_target_shows_the_open_tab() {
    let (dir, path) = fixture("SaveAsFrom1865.snxsch", &sheet_text("A4"));
    let target = dir.path().join("SaveAsTo1865.snxsch");
    let link = dir.path().join("SaveAsLink1865.snxsch");
    let (mut app, _boot) = Signex::new();
    open(&mut app, &path);
    crate::settle_support::settle(
        &mut app,
        Message::File(FileMsg::SaveAs {
            from_path: path.clone(),
            to_path: target.clone(),
        }),
    );
    assert!(target.exists(), "Save As wrote the target");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    open(&mut app, &link);

    let tabs = app
        .document_state
        .tabs
        .iter()
        .filter(|tab| tab.path == target || tab.path == link)
        .count();
    assert_eq!(
        tabs, 1,
        "the link is the Save As result, not a second document"
    );
}
