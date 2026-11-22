//! Row 143: the Preferences shell. Page list, nav search, the unsaved dots,
//! Apply, OK, Cancel and the broken-file banner.

use signex_app::annotate_undo::AnnotateUndo;
use signex_app::app::{Message, PreferencesMsg};
use signex_app::preferences::{PrefMsg, PrefNav};

use super::support::opened;

fn other_scope(scope: AnnotateUndo) -> AnnotateUndo {
    match scope {
        AnnotateUndo::Project => AnnotateUndo::PerSheet,
        AnnotateUndo::PerSheet => AnnotateUndo::Project,
    }
}

#[test]
fn preferences_opens_with_no_unsaved_page() {
    let prefs = opened();

    assert!(prefs.is_open());
    assert!(!prefs.is_dirty());
    assert!(prefs.dirty_pages().is_empty());
}

#[test]
fn a_nav_row_shows_its_page_and_is_not_an_unsaved_change() {
    let mut prefs = opened();

    for page in PrefNav::ALL {
        prefs.send(PrefMsg::Nav(*page));
        assert_eq!(prefs.app.ui_state.preferences_nav, *page);
    }

    assert!(!prefs.is_dirty());
}

#[test]
fn typing_in_the_nav_search_keeps_the_text_and_is_not_an_unsaved_change() {
    let mut prefs = opened();

    prefs.send(PrefMsg::NavSearchChanged("an".to_string()));

    assert_eq!(prefs.app.ui_state.preferences_nav_search, "an");
    assert!(!prefs.is_dirty());
}

#[test]
fn reopening_preferences_clears_the_nav_search() {
    let mut prefs = opened();
    prefs.send(PrefMsg::NavSearchChanged("an".to_string()));
    prefs.send(PrefMsg::Close);
    assert!(!prefs.is_open(), "a clean dialog closes");

    let _ = prefs.app.update(Message::Preferences(PreferencesMsg::Open));

    assert!(prefs.app.ui_state.preferences_nav_search.is_empty());
}

#[test]
fn a_changed_setting_puts_a_dot_on_its_page_only() {
    let mut prefs = opened();
    let scope = other_scope(prefs.app.ui_state.annotate_undo);

    prefs.send(PrefMsg::DraftAnnotateUndo(scope));

    assert!(prefs.is_dirty());
    assert_eq!(prefs.dirty_pages(), [PrefNav::Annotation]);
}

#[test]
fn putting_a_setting_back_removes_its_dot() {
    let mut prefs = opened();
    let saved = prefs.app.ui_state.annotate_undo;
    prefs.send(PrefMsg::DraftAnnotateUndo(other_scope(saved)));

    prefs.send(PrefMsg::DraftAnnotateUndo(saved));

    assert!(!prefs.is_dirty());
    assert!(prefs.dirty_pages().is_empty());
}

#[test]
fn apply_commits_the_change_clears_the_dots_and_keeps_the_dialog_open() {
    let mut prefs = opened();
    let scope = other_scope(prefs.app.ui_state.annotate_undo);
    prefs.send(PrefMsg::DraftAnnotateUndo(scope));

    prefs.apply();

    assert_eq!(prefs.app.ui_state.annotate_undo, scope);
    assert!(prefs.is_open());
    assert!(!prefs.is_dirty());
    assert!(prefs.dirty_pages().is_empty());
}

#[test]
fn ok_commits_the_change_and_closes_the_dialog() {
    let mut prefs = opened();
    let scope = other_scope(prefs.app.ui_state.annotate_undo);
    prefs.send(PrefMsg::DraftAnnotateUndo(scope));

    prefs.ok();

    assert_eq!(prefs.app.ui_state.annotate_undo, scope);
    assert!(!prefs.is_open());
}

#[test]
fn cancel_drops_the_changes_of_every_page_and_closes_the_dialog() {
    let mut prefs = opened();
    let saved_scope = prefs.app.ui_state.annotate_undo;
    let saved_wires = prefs.app.ui_state.move_keeps_wires;
    let saved_wheel = prefs.app.ui_state.trackpad_pan;
    prefs.send(PrefMsg::DraftAnnotateUndo(other_scope(saved_scope)));
    prefs.send(PrefMsg::DraftMoveKeepsWires(!saved_wires));
    prefs.send(PrefMsg::DraftTrackpadPan(!saved_wheel));
    assert_eq!(prefs.dirty_pages().len(), 3);

    prefs.cancel();

    let ui = &prefs.app.ui_state;
    assert!(!ui.preferences_open);
    assert!(!ui.preferences_dirty);
    assert_eq!(ui.annotate_undo, saved_scope);
    assert_eq!(ui.move_keeps_wires, saved_wires);
    assert_eq!(ui.trackpad_pan, saved_wheel);
    assert_eq!(ui.preferences_draft_annotate_undo, saved_scope);
    assert_eq!(ui.preferences_draft_move_keeps_wires, saved_wires);
    assert_eq!(ui.preferences_draft_trackpad_pan, saved_wheel);
}

#[test]
fn closing_a_dialog_with_unsaved_changes_keeps_it_open_with_the_changes() {
    let mut prefs = opened();
    let scope = other_scope(prefs.app.ui_state.annotate_undo);
    prefs.send(PrefMsg::DraftAnnotateUndo(scope));

    prefs.send(PrefMsg::Close);
    let _ = prefs
        .app
        .update(Message::Preferences(PreferencesMsg::Close));

    assert!(prefs.is_open());
    assert!(prefs.is_dirty());
    assert_eq!(prefs.app.ui_state.preferences_draft_annotate_undo, scope);
}

#[test]
fn opening_preferences_again_keeps_the_unsaved_changes() {
    let mut prefs = opened();
    let scope = other_scope(prefs.app.ui_state.annotate_undo);
    prefs.send(PrefMsg::DraftAnnotateUndo(scope));

    let _ = prefs.app.update(Message::Preferences(PreferencesMsg::Open));

    assert!(prefs.is_dirty());
    assert_eq!(prefs.app.ui_state.preferences_draft_annotate_undo, scope);
}

#[test]
fn a_broken_preferences_file_raises_the_banner_and_a_fresh_file_clears_it() {
    let mut prefs = opened();
    let path = signex_app::fonts::prefs_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("the config folder");
    }
    std::fs::write(&path, b"{ this is not json").expect("break the file");
    prefs.send(PrefMsg::Close);
    let _ = prefs.app.update(Message::Preferences(PreferencesMsg::Open));
    assert!(
        prefs.app.ui_state.prefs_load_error.is_some(),
        "the banner shows"
    );

    prefs.send(PrefMsg::ResetPrefsFile);

    assert_eq!(prefs.app.ui_state.prefs_load_error, None);
    assert!(
        prefs
            .app
            .ui_state
            .preferences_prefs_status
            .starts_with("Preferences reset."),
        "{}",
        prefs.app.ui_state.preferences_prefs_status
    );
    assert!(signex_app::fonts::check_prefs_file().is_ok());
    let dir = path.parent().expect("the config folder");
    let kept: Vec<_> = std::fs::read_dir(dir)
        .expect("the config folder")
        .flatten()
        .map(|entry| entry.path())
        .filter(|file| {
            file != &path
                && file
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("prefs.json"))
        })
        .collect();
    let held = kept
        .iter()
        .any(|file| std::fs::read(file).is_ok_and(|bytes| bytes == b"{ this is not json"));
    for file in &kept {
        let _ = std::fs::remove_file(file);
    }
    assert!(held, "the unreadable file is kept beside the fresh one");
}

#[test]
fn start_a_fresh_file_leaves_a_healthy_file_alone() {
    let mut prefs = opened();
    let path = signex_app::fonts::prefs_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("the config folder");
    }
    std::fs::write(&path, b"{}").expect("a healthy file");

    prefs.send(PrefMsg::ResetPrefsFile);

    assert_eq!(std::fs::read(&path).expect("the file"), b"{}");
    assert_eq!(
        prefs.app.ui_state.preferences_prefs_status,
        "Your preferences file is readable again \u{2014} nothing was moved."
    );
}
