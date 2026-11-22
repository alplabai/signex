//! m11 error notice, m12 About, m13 Keyboard Shortcuts, m14 first-run tour.

use std::path::PathBuf;

use signex_app::about::{ExternalLink, version_info_text};
use signex_app::app::{FileMsg, Message, OverlayMsg, Signex};
use signex_app::canvas::CanvasEvent;
use signex_app::io_ports::{HeadlessAnswers, IoPorts};
use signex_app::keyboard_shortcuts_modal::ShortcutScope;
use signex_app::keymap::ShortcutContext;
use signex_app::menu_bar::MenuMessage;
use signex_app::preferences::PrefNav;
use signex_app::task_driver::TaskDriver;

use super::support::{Desk, app, desk, esc, is_dirty, paper, set_paper, sheet_text};
use crate::settle_support::settle;

fn overlay(app: &mut Signex, msg: OverlayMsg) {
    let _ = app.update(Message::Overlay(msg));
}

// ---- m11 Error notice -----------------------------------------------------

/// A sheet edited to A3 whose file another program changed to A2, after a
/// Save that was refused and raised the File Changed on Disk card.
fn conflict() -> (Desk, PathBuf, String) {
    let mut desk = desk();
    let path = desk.sheet("c.snxsch", "A4");
    let theirs = sheet_text("A2");
    std::fs::write(&path, &theirs).expect("the other program writes");
    set_paper(&mut desk.app, &path, "A3");
    settle(&mut desk.app, Message::File(FileMsg::Save));
    let notice = desk.app.document_state.error_notice.as_ref();
    assert!(
        notice.is_some_and(|notice| notice.action.is_some()),
        "the save did not raise the choice card"
    );
    (desk, path, theirs)
}

fn on_disk(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).expect("read the file")
}

#[test]
fn m11_a_save_over_a_changed_file_keeps_their_version_and_the_edit() {
    let (desk, path, theirs) = conflict();
    assert_eq!(on_disk(&path), theirs);
    assert_eq!(paper(&desk.app, &path), "A3");
    assert!(is_dirty(&desk.app, &path));
}

#[test]
fn m11_cancel_on_the_card_changes_neither_the_file_nor_the_document() {
    let (mut desk, path, theirs) = conflict();
    overlay(&mut desk.app, OverlayMsg::DismissErrorNotice);
    assert!(desk.app.document_state.error_notice.is_none());
    assert_eq!(on_disk(&path), theirs);
    assert_eq!(paper(&desk.app, &path), "A3");
    assert!(is_dirty(&desk.app, &path), "the edit is still unsaved");
}

#[test]
fn m11_overwrite_replaces_the_file_with_this_document() {
    let (mut desk, path, theirs) = conflict();
    settle(
        &mut desk.app,
        Message::Overlay(OverlayMsg::DiskConflictOverwriteChosen),
    );
    assert!(desk.app.document_state.error_notice.is_none());
    assert_ne!(on_disk(&path), theirs);
    assert_eq!(paper(&desk.app, &path), "A3");
    assert!(!is_dirty(&desk.app, &path));
}

#[test]
fn m11_reload_from_disk_loads_the_file_and_drops_the_edit() {
    let (mut desk, path, theirs) = conflict();
    settle(
        &mut desk.app,
        Message::Overlay(OverlayMsg::DiskConflictReloadChosen),
    );
    assert!(desk.app.document_state.error_notice.is_none());
    assert_eq!(on_disk(&path), theirs, "a reload writes nothing");
    assert_eq!(paper(&desk.app, &path), "A2");
    assert!(!is_dirty(&desk.app, &path));
}

#[test]
fn m11_save_as_writes_a_new_file_and_leaves_the_changed_one() {
    let (mut desk, path, theirs) = conflict();
    let copy = desk.path("copy.snxsch");
    let answers = HeadlessAnswers::default();
    answers.replace_all([
        ("path".to_string(), copy.to_string_lossy().into_owned()),
        ("overwrite".to_string(), "true".to_string()),
    ]);
    desk.app.ui_state.io = IoPorts::headless(answers);

    settle(
        &mut desk.app,
        Message::Overlay(OverlayMsg::DiskConflictSaveAsChosen),
    );

    assert_eq!(on_disk(&path), theirs);
    assert!(copy.exists(), "the new file is written");
    assert_eq!(desk.app.document_state.active_path, Some(copy.clone()));
    assert!(!is_dirty(&desk.app, &copy));
}

#[test]
fn m11_copy_puts_the_heading_on_the_clipboard_and_keeps_the_card() {
    let (mut desk, _path, _theirs) = conflict();
    let heading = desk
        .app
        .document_state
        .error_notice
        .as_ref()
        .expect("the card")
        .title_text(&desk.app.ui_state.i18n);
    let mut driver = TaskDriver::new();
    driver
        .send(
            &mut desk.app,
            Message::Overlay(OverlayMsg::ErrorNoticeCopyPressed),
        )
        .expect("the copy settles");
    assert!(
        driver
            .clipboard_text()
            .is_some_and(|text| text.contains(&heading)),
        "the clipboard lacks the heading"
    );
    assert!(desk.app.document_state.error_notice.is_some());
}

#[test]
fn m11_esc_dismisses_a_plain_notice() {
    let mut app = app();
    app.ui_state.io = IoPorts::headless(HeadlessAnswers::default());
    settle(
        &mut app,
        Message::Overlay(OverlayMsg::OpenLink(ExternalLink::Documentation)),
    );
    assert!(app.document_state.error_notice.is_some());
    esc(&mut app);
    assert!(app.document_state.error_notice.is_none());
}

// ---- m12 About ------------------------------------------------------------

#[test]
fn m12_about_opens_and_close_closes_it() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenAbout));
    assert!(app.ui_state.about_open);
    overlay(&mut app, OverlayMsg::CloseAbout);
    assert!(!app.ui_state.about_open);
}

#[test]
fn m12_esc_closes_about() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenAbout));
    esc(&mut app);
    assert!(!app.ui_state.about_open);
}

#[test]
fn m12_copy_version_info_copies_version_license_and_os() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenAbout));
    let mut driver = TaskDriver::new();
    driver
        .send(&mut app, Message::Overlay(OverlayMsg::CopyAboutInfo))
        .expect("the copy settles");
    let expected = version_info_text();
    assert_eq!(driver.clipboard_text(), Some(expected.as_str()));
    assert!(expected.contains(env!("CARGO_PKG_VERSION")));
    assert!(expected.contains("License"));
    assert!(expected.contains("OS"));
    assert!(app.ui_state.about_open, "copying keeps the dialog open");
}

#[test]
fn m12_a_link_that_cannot_open_says_so_on_the_error_card() {
    for link in [
        ExternalLink::Documentation,
        ExternalLink::IssueTracker,
        ExternalLink::License,
    ] {
        let mut app = app();
        // Launches are refused here, so no browser ever opens.
        app.ui_state.io = IoPorts::headless(HeadlessAnswers::default());
        settle(&mut app, Message::Overlay(OverlayMsg::OpenLink(link)));
        assert!(app.document_state.error_notice.is_some(), "{link:?}");
    }
}

// ---- m13 Keyboard Shortcuts -----------------------------------------------

#[test]
fn m13_the_sheet_opens_on_the_active_editors_scope_with_no_filter() {
    let mut desk = desk();
    desk.sheet("a.snxsch", "A4");
    desk.app.ui_state.shortcut_sheet.filter = "left over".to_string();

    let _ = desk
        .app
        .update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));

    let sheet = &desk.app.ui_state.shortcut_sheet;
    assert!(desk.app.ui_state.keyboard_shortcuts_open);
    assert_eq!(sheet.scope, ShortcutScope::Schematic);
    assert!(sheet.filter.is_empty());
    assert!(desk.app.ui_state.shortcut_sections.is_some());
}

#[test]
fn m13_with_no_editor_the_sheet_opens_on_all() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    assert_eq!(app.ui_state.shortcut_sheet.scope, ShortcutScope::All);
}

#[test]
fn m13_the_filter_field_sets_and_clears_the_filter() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    overlay(
        &mut app,
        OverlayMsg::ShortcutsFilterChanged("wire".to_string()),
    );
    assert!(app.ui_state.shortcut_sheet.is_filtered());
    overlay(&mut app, OverlayMsg::ShortcutsFilterCleared);
    assert!(!app.ui_state.shortcut_sheet.is_filtered());
}

#[test]
fn m13_a_blank_filter_does_not_count_as_a_filter() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    overlay(
        &mut app,
        OverlayMsg::ShortcutsFilterChanged("   ".to_string()),
    );
    assert!(!app.ui_state.shortcut_sheet.is_filtered());
}

#[test]
fn m13_the_scope_switch_hides_the_other_editors_rows() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    overlay(
        &mut app,
        OverlayMsg::ShortcutsScopePicked(ShortcutScope::Pcb),
    );
    let scope = app.ui_state.shortcut_sheet.scope;
    assert_eq!(scope, ShortcutScope::Pcb);
    assert!(scope.shows(ShortcutContext::Pcb));
    assert!(scope.shows(ShortcutContext::Global));
    assert!(!scope.shows(ShortcutContext::Schematic));
    assert!(!scope.shows(ShortcutContext::Footprint));

    let sections = app.ui_state.shortcut_sections.as_ref().expect("the rows");
    let count = |shown: ShortcutScope| -> usize {
        sections
            .iter()
            .map(|section| {
                section
                    .rows
                    .iter()
                    .filter(|row| shown.shows(row.context))
                    .count()
            })
            .sum()
    };
    assert!(
        count(scope) < count(ShortcutScope::All),
        "the PCB scope hides nothing"
    );
}

#[test]
fn m13_reopening_the_sheet_clears_the_filter() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    overlay(
        &mut app,
        OverlayMsg::ShortcutsFilterChanged("wire".to_string()),
    );
    overlay(&mut app, OverlayMsg::CloseKeyboardShortcuts);
    assert!(!app.ui_state.keyboard_shortcuts_open);

    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));

    assert!(app.ui_state.shortcut_sheet.filter.is_empty());
}

#[test]
fn m13_esc_closes_the_sheet() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    esc(&mut app);
    assert!(!app.ui_state.keyboard_shortcuts_open);
}

#[test]
fn m13_edit_in_preferences_closes_the_sheet_and_opens_the_keyboard_page() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    overlay(&mut app, OverlayMsg::ShortcutsPreferencesPressed);
    assert!(!app.ui_state.keyboard_shortcuts_open);
    assert!(app.ui_state.preferences_open);
    assert_eq!(app.ui_state.preferences_nav, PrefNav::Keyboard);
}

// ---- m14 First-run tour ---------------------------------------------------

fn with_tour() -> Signex {
    let mut app = app();
    app.ui_state.first_run_tour_open = true;
    app
}

#[test]
fn m14_got_it_closes_the_card() {
    let mut app = with_tour();
    overlay(&mut app, OverlayMsg::DismissFirstRunTour);
    assert!(!app.ui_state.first_run_tour_open);
}

#[test]
fn m14_esc_closes_the_card() {
    let mut app = with_tour();
    let window = app.ui_state.main_window_id;
    let _ = app.update(Message::Input(signex_app::app::InputMsg::EscapePressed {
        window,
    }));
    assert!(!app.ui_state.first_run_tour_open);
}

#[test]
fn m14_the_wheel_change_link_opens_appearance_and_keeps_the_card() {
    let mut app = with_tour();
    overlay(&mut app, OverlayMsg::FirstRunTourWheelChangePressed);
    assert!(app.ui_state.preferences_open);
    assert_eq!(app.ui_state.preferences_nav, PrefNav::Appearance);
    assert!(app.ui_state.first_run_tour_open, "the card stays open");
}

#[test]
fn m14_a_pointer_that_only_moves_does_not_close_the_card() {
    let mut app = with_tour();
    let _ = app.update(Message::CanvasEvent(CanvasEvent::CursorAt {
        x: 10.0,
        y: 10.0,
        zoom_pct: 100.0,
    }));
    assert!(app.ui_state.first_run_tour_open);
}
