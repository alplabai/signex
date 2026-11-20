//! #143 — the Preferences shell: the page list, the unsaved-changes dots,
//! the broken-file banner's Open folder, and the footer.

use signex_app::annotate_undo::AnnotateUndo;
use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::io_ports::Opener;
use signex_app::preferences::{PrefMsg, PrefNav};

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

/// The nav lists System (Appearance, Language, Keyboard shortcuts), then
/// Schematic, Validation and Library, as the mock does.
#[test]
fn the_nav_lists_the_pages_in_the_mock_order() {
    assert_eq!(
        PrefNav::ALL,
        [
            PrefNav::Appearance,
            PrefNav::Language,
            PrefNav::Keyboard,
            PrefNav::Annotation,
            PrefNav::Editing,
            PrefNav::SchematicDisplay,
            PrefNav::RuleProfiles,
            PrefNav::LibraryDistributors,
            PrefNav::ComponentClasses,
        ]
    );
}

/// Each page with unsaved drafts gets a dot, in nav order, and only those.
#[test]
fn the_dirty_dots_name_the_pages_with_unsaved_drafts() {
    // Arrange
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    assert!(
        app.ui_state.preferences_pages.dirty_pages.is_empty(),
        "opened clean"
    );
    let other_scope = match app.ui_state.annotate_undo {
        AnnotateUndo::Project => AnnotateUndo::PerSheet,
        AnnotateUndo::PerSheet => AnnotateUndo::Project,
    };
    let keeps_wires = app.ui_state.move_keeps_wires;

    // Act: Editing first, so the order comes from the nav, not the edits.
    let _ = app.update(inner(PrefMsg::DraftMoveKeepsWires(!keeps_wires)));
    let _ = app.update(inner(PrefMsg::DraftAnnotateUndo(other_scope)));

    // Assert
    assert_eq!(
        app.ui_state.preferences_pages.dirty_pages,
        [PrefNav::Annotation, PrefNav::Editing]
    );
    assert!(app.ui_state.preferences_dirty);

    // Act: putting one back clears its dot only.
    let _ = app.update(inner(PrefMsg::DraftMoveKeepsWires(keeps_wires)));

    // Assert
    assert_eq!(
        app.ui_state.preferences_pages.dirty_pages,
        [PrefNav::Annotation]
    );
}

/// The broken-file banner's Open folder hands the launch to a task; it is
/// never run inside `update`.
#[test]
fn open_folder_returns_a_task() {
    // Arrange: a headless opener, in case anything ever drives the task.
    let (mut app, _t) = Signex::new();
    app.ui_state.io.opener = Opener::Refuse;
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let baseline = app.update(inner(PrefMsg::Noop)).units();

    // Act
    let task = app.update(inner(PrefMsg::PrefsFolderOpenRequested));

    // Assert
    assert_eq!(task.units(), baseline + 1, "one background launch");
    assert!(!app.ui_state.preferences_dirty, "not an unsaved change");
}

/// The Annotation page's Related row opens the Editing page.
#[test]
fn the_related_link_opens_editing() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let _ = app.update(inner(PrefMsg::Nav(PrefNav::Annotation)));

    let _ = app.update(inner(PrefMsg::Nav(PrefNav::Editing)));

    assert_eq!(app.ui_state.preferences_nav, PrefNav::Editing);
}

/// Restore defaults has no backend, so the footer does not draw it (#143);
/// it returns with its backend.
#[test]
fn the_footer_has_no_restore_defaults() {
    let shell = include_str!("../../src/preferences/shell.rs");
    assert!(
        !shell.contains("PREFERENCES_RESTORE_DEFAULTS"),
        "the footer draws Restore defaults again"
    );
}
