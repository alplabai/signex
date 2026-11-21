//! Reload from disk on the drifted-file card (#1826), end to end through
//! `update`: what the reloaded engine holds, which engines it leaves alone,
//! and the refusal when the sheet was edited while the file was read.

use std::path::{Path, PathBuf};

use signex_types::schematic::{Point, SchematicSheet, Wire};

use super::restore_commit_tests::collect_messages;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{FileMsg, Message, Signex};
use crate::persist::{ContentHash, DiskDrift};

/// The wire only the file on disk holds.
const ON_DISK: u128 = 0xD15C;
/// The wire only the open sheet holds.
const IN_MEMORY: u128 = 0x3E3;

fn wire(id: u128, y: f64) -> Wire {
    Wire {
        uuid: uuid::Uuid::from_u128(id),
        start: Point::new(10.16, y),
        end: Point::new(30.48, y),
        stroke_width: 0.0,
        color: None,
        locked: false,
        unknown_columns: Default::default(),
    }
}

fn sheet_with(id: u128) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".into(),
        wires: vec![wire(id, 10.16)],
        ..Default::default()
    }
}

/// Writes the on-disk revision of `path` and returns its bytes.
fn write_disk_revision(path: &Path) -> String {
    let text = signex_types::format::SnxSchematic::new(sheet_with(ON_DISK))
        .write_string()
        .expect("encode");
    std::fs::write(path, &text).expect("write");
    text
}

fn wire_ids(app: &Signex, path: &Path) -> Vec<uuid::Uuid> {
    app.document_state.engines[path]
        .document()
        .wires
        .iter()
        .map(|w| w.uuid)
        .collect()
}

/// A save of `path` found the file changed: the card is up and the sheet modified.
fn save_found_drift(app: &mut Signex, path: &Path) {
    app.document_state.mark_document_dirty(path);
    app.handle_save_found_disk_drift(path.to_path_buf(), DiskDrift::Changed);
    assert!(app.document_state.error_notice.is_some(), "the card is up");
}

/// Click Reload and run the read; the completion is returned unapplied.
fn click_reload(app: &mut Signex) -> Vec<Message> {
    let task = app.handle_disk_conflict_reload();
    collect_messages(task)
}

fn deliver(app: &mut Signex, messages: Vec<Message>) {
    for message in messages {
        assert!(
            matches!(message, Message::File(FileMsg::DiskReloadFinished { .. })),
            "expected the reload completion, got {message:?}"
        );
        let _ = app.update(message);
    }
}

fn open(app: &mut Signex, path: &Path, id: u128) {
    let title = path
        .file_stem()
        .expect("stem")
        .to_string_lossy()
        .into_owned();
    app.open_schematic_tab(path.to_path_buf(), title, sheet_with(id));
}

#[test]
fn reloading_the_active_sheet_installs_the_file_and_leaves_it_saved() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("reload_active.snxsch");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    open(&mut app, &path, IN_MEMORY);
    let text = write_disk_revision(&path);
    save_found_drift(&mut app, &path);

    let messages = click_reload(&mut app);
    deliver(&mut app, messages);

    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(path.as_path())
    );
    assert_eq!(wire_ids(&app, &path), vec![uuid::Uuid::from_u128(ON_DISK)]);
    assert!(!app.document_state.is_document_dirty(&path), "matches disk");
    assert_eq!(app.document_state.document_saves.failure(&path), None);
    assert_eq!(
        app.document_state.document_saves.baseline(&path),
        Some(ContentHash::of(text.as_bytes())),
        "the next save compares with the file just read"
    );
    assert!(
        app.document_state.error_notice.is_none(),
        "the card is gone"
    );
}

#[test]
fn reloading_a_background_sheet_replaces_only_its_engine() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("reload_background.snxsch");
    let other = dir.path().join("reload_foreground.snxsch");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    open(&mut app, &path, IN_MEMORY);
    write_disk_revision(&path);
    save_found_drift(&mut app, &path);
    // The user moves to another sheet before choosing Reload.
    open(&mut app, &other, IN_MEMORY);
    let active_tab = app.document_state.active_tab;
    let foreground_uuid = app.document_state.engines[&other].document().uuid;
    let foreground_revision = app.document_state.engines[&other].revision();

    let messages = click_reload(&mut app);
    deliver(&mut app, messages);

    assert_eq!(wire_ids(&app, &path), vec![uuid::Uuid::from_u128(ON_DISK)]);
    assert!(!app.document_state.is_document_dirty(&path));
    assert_eq!(app.document_state.active_tab, active_tab, "focus stays");
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(other.as_path())
    );
    let engine = &app.document_state.engines[&other];
    assert_eq!(engine.document().uuid, foreground_uuid, "the same engine");
    assert_eq!(engine.revision(), foreground_revision, "and unedited");
    assert_eq!(
        wire_ids(&app, &other),
        vec![uuid::Uuid::from_u128(IN_MEMORY)]
    );
}

#[test]
fn an_edit_made_while_the_file_is_read_refuses_the_reload_and_survives() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("reload_edited_meanwhile.snxsch");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    open(&mut app, &path, IN_MEMORY);
    write_disk_revision(&path);
    save_found_drift(&mut app, &path);
    let needle = "[reload] reload_edited_meanwhile.snxsch was edited while it reloaded; the \
                  edits were kept, nothing was reloaded";
    let before = crate::diagnostics::logged_count(needle);

    let messages = click_reload(&mut app);
    // The user draws a wire after the read started.
    let edit = wire(0xED17, 20.32);
    assert!(app.apply_engine_command(
        signex_engine::Command::PlaceWireSegment { wire: edit.clone() },
        true,
        true,
    ));
    deliver(&mut app, messages);

    assert_eq!(crate::diagnostics::logged_count(needle), before + 1);
    assert_eq!(
        wire_ids(&app, &path),
        vec![uuid::Uuid::from_u128(IN_MEMORY), edit.uuid],
        "the edit is kept and the file was not installed"
    );
    assert!(app.document_state.is_document_dirty(&path));
    assert!(
        app.document_state.document_saves.failure(&path).is_some(),
        "the next save still asks"
    );
    assert!(
        app.document_state.error_notice.is_some(),
        "the card comes back so the user chooses again"
    );
}
