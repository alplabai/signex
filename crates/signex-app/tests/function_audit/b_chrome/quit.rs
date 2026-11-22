//! m16 exit prompt. The main window id is cleared where a test watches the
//! exit, so closing the app is `iced::exit`, which the task driver reports.

use std::path::PathBuf;

use signex_app::app::{
    ContextMenuMsg, Message, ProjectCloseChoice, ProjectMsg, Signex, TabContextAction, WindowMsg,
};
use signex_app::task_driver::TaskDriver;

use super::support::{Desk, desk, esc, is_dirty, paper, set_paper};
use crate::settle_support::settle;

/// One sheet edited to A3 over its saved A4 file.
fn edited() -> (Desk, PathBuf, Vec<u8>) {
    let mut desk = desk();
    let path = desk.sheet("power.snxsch", "A4");
    let saved = std::fs::read(&path).expect("read the file");
    set_paper(&mut desk.app, &path, "A3");
    (desk, path, saved)
}

fn request_quit(app: &mut Signex) {
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
}

fn choose(driver: &mut TaskDriver, app: &mut Signex, choice: ProjectCloseChoice) {
    driver
        .send(app, Message::Project(ProjectMsg::AppQuitConfirm(choice)))
        .expect("the choice settles");
}

#[test]
fn m16_exit_with_nothing_unsaved_closes_without_asking() {
    let mut desk = desk();
    desk.sheet("power.snxsch", "A4");
    desk.app.ui_state.main_window_id = None;
    let mut driver = TaskDriver::new();

    driver
        .send(&mut desk.app, Message::Window(WindowMsg::CloseMainWindow))
        .expect("the quit settles");

    assert!(desk.app.ui_state.app_quit_confirm.is_none());
    assert!(driver.has_exited());
}

#[test]
fn m16_exit_with_an_unsaved_sheet_asks_and_lists_it() {
    let (mut desk, path, saved) = edited();
    request_quit(&mut desk.app);
    let prompt = desk.app.ui_state.app_quit_confirm.as_ref().expect("prompt");
    assert_eq!(prompt.dirty_paths, [path.clone()]);
    assert_eq!(std::fs::read(&path).expect("read"), saved);
}

#[test]
fn m16_cancel_stays_open_with_the_edit_and_the_file_unchanged() {
    let (mut desk, path, saved) = edited();
    desk.app.ui_state.main_window_id = None;
    request_quit(&mut desk.app);
    let mut driver = TaskDriver::new();

    choose(&mut driver, &mut desk.app, ProjectCloseChoice::Cancel);

    assert!(!driver.has_exited());
    assert!(desk.app.ui_state.app_quit_confirm.is_none());
    assert_eq!(paper(&desk.app, &path), "A3");
    assert!(is_dirty(&desk.app, &path));
    assert_eq!(std::fs::read(&path).expect("read"), saved);
}

#[test]
fn m16_esc_is_cancel() {
    let (mut desk, path, saved) = edited();
    request_quit(&mut desk.app);

    esc(&mut desk.app);

    assert!(desk.app.ui_state.app_quit_confirm.is_none());
    assert!(is_dirty(&desk.app, &path));
    assert_eq!(std::fs::read(&path).expect("read"), saved);
}

#[test]
fn m16_discard_all_exits_and_leaves_the_file_as_saved() {
    let (mut desk, path, saved) = edited();
    desk.app.ui_state.main_window_id = None;
    request_quit(&mut desk.app);
    let mut driver = TaskDriver::new();

    choose(&mut driver, &mut desk.app, ProjectCloseChoice::DiscardAll);

    assert!(driver.has_exited());
    assert_eq!(std::fs::read(&path).expect("read"), saved);
}

#[test]
fn m16_save_all_writes_the_sheet_and_exits() {
    let (mut desk, path, saved) = edited();
    desk.app.ui_state.main_window_id = None;
    request_quit(&mut desk.app);
    let mut driver = TaskDriver::new();

    choose(&mut driver, &mut desk.app, ProjectCloseChoice::SaveAll);

    assert!(driver.has_exited());
    assert_ne!(std::fs::read(&path).expect("read"), saved);
    assert!(!is_dirty(&desk.app, &path));
}

#[test]
fn m16_save_all_that_cannot_write_stays_open_with_the_edit() {
    let (mut desk, path, _) = edited();
    desk.app.ui_state.main_window_id = None;
    request_quit(&mut desk.app);
    // Another program changed the file, so the save is refused.
    std::fs::write(&path, super::support::sheet_text("A2")).expect("their write");
    let mut driver = TaskDriver::new();

    choose(&mut driver, &mut desk.app, ProjectCloseChoice::SaveAll);

    assert!(!driver.has_exited(), "exited with the edit unsaved");
    assert_eq!(paper(&desk.app, &path), "A3");
    assert!(is_dirty(&desk.app, &path));
}

/// Row m16 ("The unsaved list changed"): Discard All discards only what the
/// user was shown.
#[test]
fn m16_discard_all_is_refused_when_a_new_edit_appeared_behind_the_prompt() {
    let (mut desk, path, _) = edited();
    let other = desk.sheet("usb-c.snxsch", "A4");
    desk.app.ui_state.main_window_id = None;
    request_quit(&mut desk.app);
    let listed = desk
        .app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("prompt")
        .dirty_paths
        .clone();
    assert_eq!(listed, [path.clone()]);
    set_paper(&mut desk.app, &other, "A3");
    let mut driver = TaskDriver::new();

    choose(&mut driver, &mut desk.app, ProjectCloseChoice::DiscardAll);

    assert!(!driver.has_exited());
    let prompt = desk.app.ui_state.app_quit_confirm.as_ref().expect("prompt");
    assert!(prompt.dirty_paths.contains(&path));
    assert!(prompt.dirty_paths.contains(&other));
    assert!(desk.app.ui_state.quit_confirm_refreshed);
    assert!(is_dirty(&desk.app, &path) && is_dirty(&desk.app, &other));
}

#[test]
fn m16_a_dirty_sheet_whose_tab_was_closed_is_still_listed() {
    let (mut desk, path, _) = edited();
    let index = desk
        .app
        .document_state
        .tabs
        .iter()
        .position(|tab| tab.path == path)
        .expect("the tab");
    settle(
        &mut desk.app,
        Message::ContextMenu(ContextMenuMsg::TabAction(TabContextAction::Close(index))),
    );
    assert!(desk.app.document_state.tabs.is_empty());

    request_quit(&mut desk.app);

    let prompt = desk.app.ui_state.app_quit_confirm.as_ref().expect("prompt");
    assert_eq!(prompt.dirty_paths, [path]);
}
