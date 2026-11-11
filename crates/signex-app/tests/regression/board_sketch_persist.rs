//! #2234 review - an uncommitted board sketch is never lost: Ctrl+S, Save
//! All, quit, the quit prompt's Save All and an undock apply it to the board
//! first; a refused bake holds each of them and says why. A live sketch is
//! the board's unsaved work, and Undo in the mode steps the sketch.

use signex_app::app::{
    EditMsg, FileMsg, Message, ProjectCloseChoice, ProjectMsg, RenameDialogState, RenameMsg,
    TabContextAction, UiMsg, WindowMsg,
};
use signex_app::keymap::ShortcutTrigger;
use signex_app::library::editor::footprint::SketchTool;
use signex_app::library::messages::FootprintEditorMsg;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::{BoardSketchAttrMsg, PanelMsg};
use signex_i18n::{ids, t};
use signex_types::format::SnxPcb;
use signex_types::pcb::PcbBoard;

use crate::board_sketch::{
    Fixture, click, edit, ids_of, in_sketch, is_line, live, open_board, panel, rectangle, send,
    snapshot,
};
use crate::menu_support::menu_row;

/// Enter sketch mode and draw a 40 x 30 mm outline the bake takes.
fn sketch_an_outline(f: &mut Fixture) {
    send(f, menu_row("board_sketch"));
    let drawn = rectangle(f, (0.0, 0.0), (40.0, 30.0));
    assert_eq!(drawn.len(), 4);
    panel(
        f,
        PanelMsg::BoardSketch(BoardSketchAttrMsg::OutlineToggled {
            id: drawn[0],
            on: true,
        }),
    );
}

/// Enter sketch mode and tag one open line as the outline: the bake
/// refuses it (no closed profile).
fn sketch_a_refused_outline(f: &mut Fixture) {
    send(f, menu_row("board_sketch"));
    edit(f, FootprintEditorMsg::SketchSetTool(SketchTool::Line));
    click(f, 0.0, 0.0);
    click(f, 10.0, 0.0);
    edit(f, FootprintEditorMsg::ToolEscape);
    let line = ids_of(f, is_line)[0];
    panel(
        f,
        PanelMsg::BoardSketch(BoardSketchAttrMsg::OutlineToggled { id: line, on: true }),
    );
}

fn on_disk(f: &Fixture) -> PcbBoard {
    SnxPcb::parse(&std::fs::read_to_string(&f.path).unwrap())
        .unwrap()
        .board
}

fn title(f: &Fixture) -> String {
    f.app.title(iced::window::Id::unique())
}

fn held_message(f: &Fixture) -> String {
    t!(f.app.ui_state.i18n, ids::BOARD_SKETCH_HELD, "board" => "board.snxpcb").into_string()
}

fn notice_says_held(f: &Fixture) -> bool {
    let held = held_message(f);
    f.app
        .document_state
        .error_notice
        .as_ref()
        .is_some_and(|notice| {
            notice.detail.contains(&held) && notice.detail.contains("closed profile")
        })
}

fn press(f: &mut Fixture, window: Option<iced::window::Id>, keys: &str) {
    let ShortcutTrigger::KeySequence(mut strokes) = ShortcutTrigger::parse(keys).unwrap() else {
        panic!("{keys} is a key");
    };
    send(
        f,
        Message::Ui(UiMsg::KeymapStroke {
            window,
            stroke: strokes
                .remove(0)
                .resolved_for(signex_app::keymap::Platform::current()),
        }),
    );
}

/// Undock the board's tab into its own window.
fn undock(f: &mut Fixture) -> Option<iced::window::Id> {
    let index = f.app.document_state.active_tab;
    let before: Vec<iced::window::Id> = f.app.ui_state.windows.keys().copied().collect();
    let _ = f.app.update(Message::Window(WindowMsg::UndockTab(index)));
    let window = f
        .app
        .ui_state
        .windows
        .keys()
        .copied()
        .find(|id| !before.contains(id))?;
    let _ = f.app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: f.path.clone(),
        id: window,
    }));
    Some(window)
}

#[test]
fn a_live_sketch_counts_as_unsaved_work() {
    let mut f = open_board();
    sketch_an_outline(&mut f);
    assert!(
        !f.app.document_state.has_dirty_documents(),
        "the board itself is untouched while drawing"
    );
    assert!(title(&f).contains("1 unsaved"), "{}", title(&f));
}

#[test]
fn ctrl_s_writes_the_sketch_and_keeps_the_mode() {
    let mut f = open_board();
    sketch_an_outline(&mut f);
    send(&mut f, Message::File(FileMsg::Save));
    let saved = on_disk(&f);
    assert_eq!(saved.outline.len(), 4, "the outline is on disk");
    assert!(saved.sketch.is_some(), "the sketch is on disk");
    assert!(in_sketch(&f), "a save keeps the mode on");
    assert!(!f.app.document_state.has_dirty_documents());
    assert!(!title(&f).contains("unsaved"), "{}", title(&f));
}

/// Review N2: on a board with no other change, the live sketch alone
/// enables Save, so Ctrl+S and File ▸ Save write it.
#[test]
fn ctrl_s_and_the_save_row_write_a_sketch_on_a_clean_board() {
    let mut f = open_board();
    sketch_an_outline(&mut f);
    assert!(
        !f.app.document_state.is_document_dirty(&f.path),
        "only the sketch is owed"
    );
    press(&mut f, None, "Mod+S");
    assert!(on_disk(&f).sketch.is_some(), "Ctrl+S wrote the sketch");

    let mut f = open_board();
    sketch_an_outline(&mut f);
    send(&mut f, crate::menu_support::menu_row("save_document"));
    assert!(on_disk(&f).sketch.is_some(), "File ▸ Save wrote the sketch");
}

#[test]
fn save_all_writes_the_sketch() {
    let mut f = open_board();
    sketch_an_outline(&mut f);
    send(&mut f, Message::Menu(MenuMessage::SaveAll));
    let saved = on_disk(&f);
    assert_eq!(saved.outline.len(), 4);
    assert!(saved.sketch.is_some());
}

/// The board has no Save As: the request writes nothing and keeps the copy.
#[test]
fn save_as_on_the_board_keeps_the_sketch() {
    let mut f = open_board();
    let disk = std::fs::read(&f.path).unwrap();
    sketch_an_outline(&mut f);
    send(&mut f, Message::Menu(MenuMessage::SaveAs));
    assert!(in_sketch(&f));
    assert_eq!(ids_of(&f, is_line).len(), 4, "the copy is kept");
    assert_eq!(std::fs::read(&f.path).unwrap(), disk);
}

#[test]
fn quit_applies_the_sketch_and_asks_about_the_board() {
    let mut f = open_board();
    sketch_an_outline(&mut f);
    send(&mut f, Message::Window(WindowMsg::CloseMainWindow));
    let confirm = f
        .app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("the quit prompt opens");
    assert!(confirm.dirty_paths.contains(&f.path), "it lists the board");
    assert_eq!(live(&f).outline.len(), 4, "the sketch reached the board");

    send(
        &mut f,
        Message::Project(ProjectMsg::AppQuitConfirm(ProjectCloseChoice::SaveAll)),
    );
    assert_eq!(on_disk(&f).outline.len(), 4, "Save All wrote it");
}

/// A sketch drawn behind the quit prompt is applied by its Save All too.
#[test]
fn the_quit_prompt_save_all_applies_a_later_sketch() {
    let mut f = open_board();
    sketch_an_outline(&mut f);
    send(&mut f, Message::Window(WindowMsg::CloseMainWindow));
    assert!(f.app.ui_state.app_quit_confirm.is_some());
    rectangle(&mut f, (50.0, 0.0), (60.0, 10.0));
    send(
        &mut f,
        Message::Project(ProjectMsg::AppQuitConfirm(ProjectCloseChoice::SaveAll)),
    );
    let stored = on_disk(&f).sketch.expect("the sketch is on disk");
    assert_eq!(
        stored
            .sketch
            .entities
            .iter()
            .filter(|e| is_line(&e.kind))
            .count(),
        8
    );
}

#[test]
fn a_refused_bake_holds_save_save_all_quit_close_and_undock() {
    let mut f = open_board();
    let disk = std::fs::read(&f.path).unwrap();
    let before = snapshot(&f);
    sketch_a_refused_outline(&mut f);
    assert!(title(&f).contains("1 unsaved"), "{}", title(&f));

    send(&mut f, Message::File(FileMsg::Save));
    assert!(notice_says_held(&f), "Ctrl+S says why it waits");
    f.app.document_state.error_notice = None;

    send(&mut f, Message::Menu(MenuMessage::SaveAll));
    assert!(notice_says_held(&f), "Save All says why it waits");
    f.app.document_state.error_notice = None;

    send(&mut f, Message::Window(WindowMsg::CloseMainWindow));
    assert!(f.app.ui_state.app_quit_confirm.is_none(), "no quit prompt");
    assert!(notice_says_held(&f), "the quit says why it waits");
    f.app.document_state.error_notice = None;

    let tabs = f.app.document_state.tabs.len();
    let idx = f.app.document_state.active_tab;
    send(
        &mut f,
        Message::ContextMenu(signex_app::app::ContextMenuMsg::TabAction(
            TabContextAction::Close(idx),
        )),
    );
    assert_eq!(f.app.document_state.tabs.len(), tabs, "the tab stays open");
    assert!(notice_says_held(&f), "the close says why it waits");
    f.app.document_state.error_notice = None;

    assert!(undock(&mut f).is_none(), "the board stays docked");
    assert!(notice_says_held(&f), "the undock says why it waits");

    assert!(in_sketch(&f), "the mode and its copy stay");
    assert_eq!(ids_of(&f, is_line).len(), 1);
    assert_eq!(snapshot(&f), before, "the board is unchanged");
    assert_eq!(std::fs::read(&f.path).unwrap(), disk, "nothing was written");
    let held = held_message(&f);
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains(&held)),
        "Messages says it too"
    );
}

/// Undock leaves the mode with the sketch applied, so Ctrl+Z in the board's
/// window steps the board the user sees, not a hidden sketch.
#[test]
fn undock_applies_the_sketch_and_ctrl_z_there_steps_the_board() {
    let mut f = open_board();
    let before = snapshot(&f);
    sketch_an_outline(&mut f);
    let window = undock(&mut f).expect("the board undocks");
    assert!(!in_sketch(&f), "the mode ends on undock");
    assert_eq!(live(&f).outline.len(), 4, "the sketch reached the board");

    press(&mut f, Some(window), "Mod+Z");
    assert_eq!(
        snapshot(&f),
        before,
        "Ctrl+Z there undoes the applied sketch"
    );

    // The mode does not start on a board shown in its own window.
    send(&mut f, menu_row("board_sketch"));
    assert!(!in_sketch(&f));
    let refused = t!(f.app.ui_state.i18n, ids::BOARD_SKETCH_UNDOCKED).into_string();
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains(&refused)),
        "Messages says why"
    );
}

/// Undo and Redo in the mode step the sketch's own history and leave the
/// board alone, also after an Apply gave the board a step of its own.
#[test]
fn undo_and_redo_in_sketch_mode_step_the_sketch_not_the_board() {
    let mut f = open_board();
    sketch_an_outline(&mut f);
    send(
        &mut f,
        Message::Pcb(signex_app::app::PcbMsg::Sketch(
            signex_app::app::BoardSketchMsg::ApplyRequested,
        )),
    );
    let applied = snapshot(&f);
    assert_eq!(live(&f).outline.len(), 4);
    rectangle(&mut f, (50.0, 0.0), (60.0, 10.0));
    assert_eq!(ids_of(&f, is_line).len(), 8);

    send(&mut f, Message::Edit(EditMsg::Undo));
    assert_eq!(
        ids_of(&f, is_line).len(),
        4,
        "Undo removes the new rectangle"
    );
    assert_eq!(snapshot(&f), applied, "the board keeps the applied sketch");

    send(&mut f, Message::Edit(EditMsg::Redo));
    assert_eq!(ids_of(&f, is_line).len(), 8, "Redo brings it back");
    assert_eq!(snapshot(&f), applied);
}

/// A board renamed in sketch mode keeps its copy: the mode follows the file
/// and Ctrl+S writes the sketch under the new name.
#[test]
fn a_renamed_board_keeps_its_sketch() {
    let mut f = open_board();
    sketch_an_outline(&mut f);
    let renamed = f.path.with_file_name("renamed.snxpcb");
    f.app.ui_state.rename_dialog = Some(RenameDialogState {
        project_id: None,
        target_path: f.path.clone(),
        tree_path: vec![0],
        buffer: "renamed.snxpcb".to_string(),
        error: None,
        is_project_rename: false,
    });
    crate::settle_support::settle(&mut f.app, Message::Rename(RenameMsg::Submit));
    assert!(renamed.exists(), "the file moved");
    let session = f
        .app
        .interaction_state
        .board_sketch
        .as_ref()
        .expect("still in the mode");
    assert_eq!(session.board_path, renamed);
    assert_eq!(ids_of(&f, is_line).len(), 4, "the copy is kept");

    f.path = renamed;
    send(&mut f, Message::File(FileMsg::Save));
    assert_eq!(
        on_disk(&f).outline.len(),
        4,
        "the sketch is written to the new name"
    );
}
