//! #1822 — undoing back to the last saved state makes the document clean
//! again (tab dot, title count); redo and edits below the save point do not.

use super::super::super::*;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind, Symbol};

const NAME: &str = "save-point.snxsch";

fn app_with_symbol() -> (Signex, uuid::Uuid) {
    let (mut app, _task) = Signex::new();
    let id = uuid::Uuid::now_v7();
    let path = std::path::PathBuf::from(NAME);
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: id,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    // As if read from disk.
    engine.mark_saved();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(crate::app::TabInfo {
        title: NAME.to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    (app, id)
}

fn move_symbol(app: &mut Signex, id: uuid::Uuid) {
    let moved = app.apply_engine_command(
        signex_engine::Command::MoveSelection {
            items: vec![SelectedItem::new(id, SelectedKind::Symbol)],
            dx: 10.0,
            dy: 0.0,
            keep_wires: false,
        },
        true,
        true,
    );
    assert!(moved, "fixture: the move must change the document");
}

fn set_field(app: &mut Signex, id: uuid::Uuid, value: &str) {
    let _task = app.handle_parameter_manager_edit(id, "MPN".to_string(), value.to_string());
}

fn path() -> std::path::PathBuf {
    std::path::PathBuf::from(NAME)
}

/// What the tab dot and the title-bar "N unsaved" read.
fn shows_dirty(app: &Signex) -> bool {
    let tab_dot = app.document_state.dirty_document_paths().contains(&path());
    let title_count = app.document_state.dirty_document_count();
    assert_eq!(tab_dot, title_count == 1, "tab dot and title count agree");
    tab_dot
}

/// A save that landed for the current revision.
fn save(app: &mut Signex) {
    app.document_state.mark_document_clean(&path());
}

#[test]
fn undoing_the_only_edit_since_the_save_is_clean_and_redo_is_dirty() {
    let (mut app, id) = app_with_symbol();
    assert!(!shows_dirty(&app));
    move_symbol(&mut app, id);
    assert!(shows_dirty(&app));
    app.handle_undo_requested();
    assert!(!shows_dirty(&app), "back at the loaded state");
    app.handle_redo_requested();
    assert!(shows_dirty(&app));
}

#[test]
fn save_then_edit_then_undo_is_clean() {
    let (mut app, id) = app_with_symbol();
    move_symbol(&mut app, id);
    save(&mut app);
    assert!(!shows_dirty(&app));
    move_symbol(&mut app, id);
    assert!(shows_dirty(&app));
    app.handle_undo_requested();
    assert!(!shows_dirty(&app), "back at the saved state");
    app.handle_undo_requested();
    assert!(shows_dirty(&app), "below the saved state");
}

#[test]
fn an_edit_after_undoing_past_the_save_point_can_never_be_undone_clean() {
    let (mut app, id) = app_with_symbol();
    move_symbol(&mut app, id);
    save(&mut app);
    app.handle_undo_requested();
    move_symbol(&mut app, id);
    assert!(shows_dirty(&app));
    app.handle_undo_requested();
    assert!(
        shows_dirty(&app),
        "the saved state is gone from the history"
    );
    app.handle_redo_requested();
    assert!(shows_dirty(&app));
    save(&mut app);
    assert!(!shows_dirty(&app));
}

#[test]
fn a_cell_run_that_amends_the_saved_entry_stays_dirty() {
    let (mut app, id) = app_with_symbol();
    set_field(&mut app, id, "A");
    save(&mut app);
    set_field(&mut app, id, "AB");
    assert!(shows_dirty(&app));
    app.handle_undo_requested();
    assert!(
        shows_dirty(&app),
        "the file holds \"A\", which undo cannot restore"
    );
}

#[test]
fn undo_does_not_clear_a_mark_the_history_did_not_set() {
    let (mut app, id) = app_with_symbol();
    move_symbol(&mut app, id);
    app.handle_undo_requested();
    assert!(!shows_dirty(&app));
    // A net colour (or any other change outside the engine history).
    app.document_state.mark_document_dirty_display_only(&path());
    move_symbol(&mut app, id);
    app.handle_undo_requested();
    assert!(shows_dirty(&app), "the outside mark survives the undo");
    save(&mut app);
    assert!(!shows_dirty(&app));
}

// ---- flows through `Signex::update` with real files (#1822 review) ----

use crate::app::{EditMsg, FileMsg, Message};

/// Run `task` to completion, feeding every message back through `update`.
fn drive(app: &mut Signex, task: iced::Task<Message>) {
    use futures::StreamExt as _;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let messages = runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    });
    for message in messages {
        let follow_up = app.update(message);
        drive(app, follow_up);
    }
}

struct Disk {
    app: Signex,
    id: uuid::Uuid,
    file: std::path::PathBuf,
    dir: tempfile::TempDir,
}

/// A one-symbol sheet written to disk and opened the way File > Open does.
fn opened_from_disk() -> Disk {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("flow.snxsch");
    let id = uuid::Uuid::now_v7();
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: id,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let text = signex_types::format::SnxSchematic::new(sheet.clone())
        .write_string()
        .expect("encode");
    std::fs::write(&file, text).expect("write");
    let (mut app, _boot) = Signex::new();
    app.open_schematic_tab(file.clone(), "flow".to_string(), sheet);
    Disk { app, id, file, dir }
}

impl Disk {
    fn dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.file)
    }
    fn undo(&mut self) {
        let task = self.app.update(Message::Edit(EditMsg::Undo));
        drive(&mut self.app, task);
    }
    fn redo(&mut self) {
        let task = self.app.update(Message::Edit(EditMsg::Redo));
        drive(&mut self.app, task);
    }
    fn edit(&mut self) {
        move_symbol(&mut self.app, self.id);
    }
    fn save(&mut self) {
        let task = self.app.update(Message::File(FileMsg::Save));
        drive(&mut self.app, task);
    }
}

#[test]
fn a_sheet_opened_from_disk_undoes_back_to_clean() {
    let mut d = opened_from_disk();
    assert!(!d.dirty());
    d.edit();
    assert!(d.dirty());
    d.undo();
    assert!(!d.dirty());
    d.redo();
    assert!(d.dirty());
}

#[test]
fn a_real_save_then_undo_redo_cycles_follow_the_saved_state() {
    let mut d = opened_from_disk();
    d.edit();
    d.save();
    assert!(!d.dirty(), "the save landed");
    d.edit();
    assert!(d.dirty());
    d.undo();
    assert!(!d.dirty(), "back at what the file holds");
    d.undo();
    assert!(d.dirty(), "below what the file holds");
    d.redo();
    assert!(!d.dirty());
    d.redo();
    assert!(d.dirty());
    let _keep = &d.dir;
}

#[test]
fn undoing_while_a_save_is_in_flight_is_dirty_once_the_write_lands() {
    let mut d = opened_from_disk();
    d.edit();
    // The save snapshots the edited sheet but has not run yet.
    let write = d.app.update(Message::File(FileMsg::Save));
    d.undo();
    assert!(!d.dirty(), "at the state the file held when Undo ran");
    drive(&mut d.app, write);
    assert!(
        d.dirty(),
        "the file now holds the edit that the document no longer has"
    );
    d.undo();
    assert!(d.dirty(), "no undo step reproduces the file");
    d.save();
    assert!(!d.dirty());
}

#[test]
fn save_as_with_an_undo_during_the_write_stays_dirty() {
    let mut d = opened_from_disk();
    d.edit();
    let target = d.dir.path().join("moved.snxsch");
    let write = d
        .app
        .save_active_document_as(&d.file.clone(), target.clone())
        .expect("save as starts");
    d.undo();
    drive(&mut d.app, write);
    assert!(
        d.app.document_state.is_document_dirty(&target),
        "the new file holds the edit; the document does not"
    );
    d.undo();
    assert!(d.app.document_state.is_document_dirty(&target));
}
