//! Preferences ▸ Library: the Component Classes page's table. The
//! filter narrows the rows and never the list, a dragged row takes the place
//! it is dropped on, Reset to Defaults… asks first, Add Class appends a row
//! and focuses its key, and Save keeps merging a repeated key as before.

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::fonts::ComponentClassEntry;
use signex_app::preferences::{ClassesViewMsg, PrefMsg};

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

fn classes(msg: ClassesViewMsg) -> Message {
    inner(PrefMsg::ClassesView(msg))
}

fn keys(app: &Signex) -> Vec<String> {
    app.ui_state
        .preferences_draft_component_classes
        .iter()
        .map(|c| c.key.clone())
        .collect()
}

fn class(key: &str, label: &str) -> ComponentClassEntry {
    ComponentClassEntry {
        key: key.into(),
        label: label.into(),
    }
}

fn opened_with(list: Vec<ComponentClassEntry>) -> Signex {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    app.ui_state.preferences_draft_component_classes = list;
    app
}

fn abc() -> Vec<ComponentClassEntry> {
    vec![
        class("a", "A"),
        class("b", "B"),
        class("c", "C"),
        class("d", "D"),
    ]
}

#[test]
fn a_dropped_row_takes_the_place_it_was_dropped_on_and_marks_the_page_unsaved() {
    let mut app = opened_with(abc());
    app.ui_state.component_classes = abc();

    let _ = app.update(classes(ClassesViewMsg::DragStarted(0)));
    assert_eq!(
        app.ui_state.preferences_classes_view.dragging,
        Some(0),
        "the grip starts a drag"
    );
    let _ = app.update(classes(ClassesViewMsg::RowMoved { from: 0, to: 2 }));

    assert_eq!(keys(&app), ["b", "c", "a", "d"]);
    assert_eq!(app.ui_state.preferences_classes_view.dragging, None);
    assert!(
        app.ui_state.preferences_dirty,
        "the order is the picker order"
    );

    // A release off the rows ends the drag and moves nothing.
    let _ = app.update(classes(ClassesViewMsg::DragStarted(3)));
    let _ = app.update(classes(ClassesViewMsg::DragEnded));
    assert_eq!(app.ui_state.preferences_classes_view.dragging, None);
    assert_eq!(keys(&app), ["b", "c", "a", "d"]);
}

#[test]
fn the_filter_narrows_the_view_and_never_the_stored_order() {
    let mut app = opened_with(abc());
    app.ui_state.component_classes = abc();

    let _ = app.update(classes(ClassesViewMsg::FilterChanged("C".into())));

    assert_eq!(app.ui_state.preferences_classes_view.filter, "C");
    assert_eq!(keys(&app), ["a", "b", "c", "d"]);
    assert!(
        !app.ui_state.preferences_dirty,
        "filtering is not an unsaved change"
    );
}

#[test]
fn reset_to_defaults_replaces_the_list_only_after_it_is_confirmed() {
    let edited = vec![class("only", "Only")];
    let mut app = opened_with(edited.clone());

    // A stray confirm without the question changes nothing.
    let _ = app.update(classes(ClassesViewMsg::ResetConfirmed));
    assert_eq!(app.ui_state.preferences_draft_component_classes, edited);

    let _ = app.update(classes(ClassesViewMsg::ResetRequested));
    assert!(app.ui_state.preferences_classes_view.reset_pending);
    assert_eq!(
        app.ui_state.preferences_draft_component_classes, edited,
        "asking replaces nothing"
    );

    let _ = app.update(classes(ClassesViewMsg::ResetCancelled));
    assert!(!app.ui_state.preferences_classes_view.reset_pending);
    assert_eq!(app.ui_state.preferences_draft_component_classes, edited);

    let _ = app.update(classes(ClassesViewMsg::ResetRequested));
    let _ = app.update(classes(ClassesViewMsg::ResetConfirmed));
    assert_eq!(
        app.ui_state.preferences_draft_component_classes,
        signex_app::fonts::default_component_classes()
    );
    assert!(!app.ui_state.preferences_classes_view.reset_pending);
}

#[test]
fn add_class_appends_a_new_row_clears_the_filter_and_asks_for_its_key_focus() {
    let mut app = opened_with(abc());
    let _ = app.update(classes(ClassesViewMsg::FilterChanged("zzz".into())));

    let task = app.update(classes(ClassesViewMsg::AddRequested));

    assert_eq!(app.ui_state.preferences_draft_component_classes.len(), 5);
    assert_eq!(
        app.ui_state.preferences_draft_component_classes[4],
        class("", "")
    );
    let view = &app.ui_state.preferences_classes_view;
    assert_eq!(view.new_row, Some(4), "the appended row is the new row");
    assert!(view.filter.is_empty(), "a filter would hide the new row");
    assert!(
        task.units() > 0,
        "the focus and scroll are returned as tasks"
    );
}

#[test]
fn the_new_row_marker_follows_its_row_through_a_move_and_a_remove() {
    let mut app = opened_with(abc());
    let _ = app.update(classes(ClassesViewMsg::AddRequested));
    assert_eq!(app.ui_state.preferences_classes_view.new_row, Some(4));

    let _ = app.update(classes(ClassesViewMsg::DragStarted(4)));
    let _ = app.update(classes(ClassesViewMsg::RowMoved { from: 4, to: 1 }));
    assert_eq!(app.ui_state.preferences_classes_view.new_row, Some(1));

    let _ = app.update(classes(ClassesViewMsg::RemoveRequested(0)));
    assert_eq!(keys(&app), ["", "b", "c", "d"]);
    assert_eq!(app.ui_state.preferences_classes_view.new_row, Some(0));

    let _ = app.update(classes(ClassesViewMsg::RemoveRequested(0)));
    assert_eq!(keys(&app), ["b", "c", "d"]);
    assert_eq!(app.ui_state.preferences_classes_view.new_row, None);
}

/// The inline error does not change Save: a repeated key is still merged
/// into one class, the later label winning, as before the table.
#[test]
fn save_still_merges_a_repeated_key_the_page_flags() {
    let _serial = crate::preferences_prefs_recovery::serial();
    let _guard = crate::preferences_prefs_recovery::PrefsPathGuard::capture();
    let mut app = opened_with(vec![
        class("sensor", "Sensor"),
        class("sensor", "Hall sensor"),
    ]);

    {
        let task = app.update(inner(PrefMsg::Save));
        crate::drive::drive(&mut app, task);
    }

    assert_eq!(
        app.ui_state.component_classes,
        vec![class("sensor", "Hall sensor")]
    );
}
