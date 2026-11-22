//! Row 150: Preferences > Component Classes. The table's edits are drafts;
//! Apply stores them by the rule of the 2026-11-19 decision.

use signex_app::fonts::{ComponentClassEntry, default_component_classes};
use signex_app::preferences::{ClassesViewMsg, PrefMsg, PrefNav};

use super::support::{Prefs, opened};

fn class(key: &str, label: &str) -> ComponentClassEntry {
    ComponentClassEntry {
        key: key.to_string(),
        label: label.to_string(),
    }
}

fn abc() -> Vec<ComponentClassEntry> {
    vec![
        class("resistor", "Resistor"),
        class("capacitor", "Capacitor"),
        class("inductor", "Inductor"),
    ]
}

/// The page open on a saved list of three classes.
fn on_abc() -> Prefs {
    let mut prefs = opened();
    prefs.app.ui_state.component_classes = abc();
    prefs.app.ui_state.preferences_draft_component_classes = abc();
    prefs.send(PrefMsg::Nav(PrefNav::ComponentClasses));
    prefs
}

fn view(prefs: &mut Prefs, msg: ClassesViewMsg) {
    prefs.send(PrefMsg::ClassesView(msg));
}

fn draft(prefs: &Prefs) -> &[ComponentClassEntry] {
    &prefs.app.ui_state.preferences_draft_component_classes
}

fn stored(prefs: &Prefs) -> &[ComponentClassEntry] {
    &prefs.app.ui_state.component_classes
}

fn keys(prefs: &Prefs) -> Vec<&str> {
    draft(prefs).iter().map(|c| c.key.as_str()).collect()
}

#[test]
fn editing_a_key_and_a_label_is_a_draft_of_this_page() {
    let mut prefs = on_abc();

    prefs.send(PrefMsg::ComponentClassEditKey {
        index: 0,
        key: "res".to_string(),
    });
    prefs.send(PrefMsg::ComponentClassEditLabel {
        index: 0,
        label: "Res".to_string(),
    });

    assert_eq!(draft(&prefs)[0], class("res", "Res"));
    assert_eq!(stored(&prefs), abc());
    assert_eq!(prefs.dirty_pages(), [PrefNav::ComponentClasses]);
}

#[test]
fn apply_stores_the_edited_list_for_pickers_and_new_libraries() {
    let mut prefs = on_abc();
    prefs.send(PrefMsg::ComponentClassEditLabel {
        index: 1,
        label: "Cap".to_string(),
    });

    prefs.apply();

    let wanted = vec![
        class("resistor", "Resistor"),
        class("capacitor", "Cap"),
        class("inductor", "Inductor"),
    ];
    assert_eq!(stored(&prefs), wanted);
    assert_eq!(prefs.app.document_state.panel_ctx.component_classes, wanted);
    assert_eq!(
        prefs.app.library.new_library_defaults.component_classes,
        wanted
    );
    assert!(!prefs.is_dirty());
}

#[test]
fn cancel_puts_the_list_back() {
    let mut prefs = on_abc();
    view(&mut prefs, ClassesViewMsg::RemoveRequested(0));
    view(&mut prefs, ClassesViewMsg::AddRequested);

    prefs.cancel();

    assert_eq!(stored(&prefs), abc());
    assert_eq!(draft(&prefs), abc());
    assert!(!prefs.is_dirty());
}

#[test]
fn add_class_appends_an_empty_new_row_and_clears_the_filter() {
    let mut prefs = on_abc();
    view(&mut prefs, ClassesViewMsg::FilterChanged("zzz".to_string()));

    view(&mut prefs, ClassesViewMsg::AddRequested);

    assert_eq!(draft(&prefs).len(), 4);
    assert_eq!(draft(&prefs)[3], class("", ""));
    let page = &prefs.app.ui_state.preferences_pages.classes_view;
    assert_eq!(page.new_row, Some(3));
    assert!(page.filter.is_empty());
}

#[test]
fn the_row_x_removes_that_class() {
    let mut prefs = on_abc();

    view(&mut prefs, ClassesViewMsg::RemoveRequested(1));

    assert_eq!(keys(&prefs), ["resistor", "inductor"]);
    assert!(prefs.is_dirty());
}

#[test]
fn removing_a_row_that_is_not_there_changes_nothing() {
    let mut prefs = on_abc();

    view(&mut prefs, ClassesViewMsg::RemoveRequested(99));

    assert_eq!(draft(&prefs), abc());
    assert!(!prefs.is_dirty());
}

#[test]
fn dragging_a_row_sets_the_picker_order() {
    let mut prefs = on_abc();

    view(&mut prefs, ClassesViewMsg::DragStarted(0));
    view(&mut prefs, ClassesViewMsg::RowMoved { from: 0, to: 2 });

    assert_eq!(keys(&prefs), ["capacitor", "inductor", "resistor"]);
    assert_eq!(
        prefs.app.ui_state.preferences_pages.classes_view.dragging,
        None
    );
    assert!(prefs.is_dirty());

    prefs.apply();

    let order: Vec<&str> = stored(&prefs).iter().map(|c| c.key.as_str()).collect();
    assert_eq!(order, ["capacitor", "inductor", "resistor"]);
}

#[test]
fn a_drag_released_off_the_rows_moves_nothing() {
    let mut prefs = on_abc();

    view(&mut prefs, ClassesViewMsg::DragStarted(0));
    view(&mut prefs, ClassesViewMsg::DragEnded);

    assert_eq!(draft(&prefs), abc());
    assert!(!prefs.is_dirty());
}

#[test]
fn the_filter_narrows_the_view_and_is_not_an_unsaved_change() {
    let mut prefs = on_abc();

    view(&mut prefs, ClassesViewMsg::FilterChanged("cap".to_string()));

    assert_eq!(
        prefs.app.ui_state.preferences_pages.classes_view.filter,
        "cap"
    );
    assert_eq!(draft(&prefs), abc());
    assert!(!prefs.is_dirty());
}

#[test]
fn reset_asks_first_and_backing_out_keeps_the_list() {
    let mut prefs = on_abc();

    view(&mut prefs, ClassesViewMsg::ResetRequested);
    assert!(
        prefs
            .app
            .ui_state
            .preferences_pages
            .classes_view
            .reset_pending
    );
    assert_eq!(draft(&prefs), abc(), "asking replaces nothing");

    view(&mut prefs, ClassesViewMsg::ResetCancelled);

    assert!(
        !prefs
            .app
            .ui_state
            .preferences_pages
            .classes_view
            .reset_pending
    );
    assert_eq!(draft(&prefs), abc());
}

#[test]
fn a_confirmed_reset_restores_the_seed_classes_as_a_draft() {
    let mut prefs = on_abc();

    view(&mut prefs, ClassesViewMsg::ResetRequested);
    view(&mut prefs, ClassesViewMsg::ResetConfirmed);

    assert_eq!(draft(&prefs), default_component_classes());
    assert_eq!(stored(&prefs), abc(), "not stored before Apply");
    assert!(prefs.is_dirty());
}

#[test]
fn a_reset_confirm_that_was_never_asked_changes_nothing() {
    let mut prefs = on_abc();

    view(&mut prefs, ClassesViewMsg::ResetConfirmed);

    assert_eq!(draft(&prefs), abc());
}

#[test]
fn apply_drops_a_row_with_no_key() {
    let mut prefs = on_abc();
    view(&mut prefs, ClassesViewMsg::AddRequested);
    prefs.send(PrefMsg::ComponentClassEditLabel {
        index: 3,
        label: "No key".to_string(),
    });

    prefs.apply();

    assert_eq!(stored(&prefs), abc());
    assert_eq!(draft(&prefs), abc(), "the table shows what was stored");
}

#[test]
fn apply_merges_a_repeated_key_and_the_later_label_wins() {
    let mut prefs = on_abc();
    view(&mut prefs, ClassesViewMsg::AddRequested);
    prefs.send(PrefMsg::ComponentClassEditKey {
        index: 3,
        key: "resistor".to_string(),
    });
    prefs.send(PrefMsg::ComponentClassEditLabel {
        index: 3,
        label: "Thin film".to_string(),
    });

    prefs.apply();

    assert_eq!(
        stored(&prefs),
        vec![
            class("resistor", "Thin film"),
            class("capacitor", "Capacitor"),
            class("inductor", "Inductor"),
        ]
    );
}
