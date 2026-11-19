//! #158 - Named unions: Create Union (the menu row and Ctrl+G) opens a
//! popover whose Create is one undo step; the Unions panel locks, hides and
//! breaks; hiding is a view setting, not an undo step; a locked union's
//! members cannot be deleted.

use signex_app::app::unions::{UnionPopoverMsg, UnionsMsg};
use signex_app::app::{EditMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::keymap::{AppCommandId, ShortcutProfileSet};
use signex_app::panels::PanelMsg;
use signex_types::schematic::SchematicSheet;

use crate::paste_support::sheet::{app_with_sheet, populated_sheet, select};

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state.active_engine().unwrap().document()
}

fn undo_position(app: &Signex) -> usize {
    app.document_state.active_engine().unwrap().undo_position()
}

fn unions(app: &mut Signex, msg: UnionsMsg) {
    let _ = app.update(Message::Edit(EditMsg::Unions(msg)));
}

fn panel(app: &mut Signex, msg: UnionsMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::Unions(msg))));
}

/// R1 and R2 grouped as "Input filter" through the popover.
fn app_with_union() -> Signex {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R1", 1), ("R2", 1)], false);
    unions(&mut app, UnionsMsg::CreateRequested);
    let popover = app
        .ui_state
        .unions
        .popover
        .as_ref()
        .expect("the popover opened");
    assert_eq!(popover.members.len(), 2);
    unions(
        &mut app,
        UnionsMsg::Popover(UnionPopoverMsg::NameEdited("Input filter".into())),
    );
    unions(
        &mut app,
        UnionsMsg::Popover(UnionPopoverMsg::TagInputEdited("#emc".into())),
    );
    unions(&mut app, UnionsMsg::Popover(UnionPopoverMsg::TagSubmitted));
    unions(&mut app, UnionsMsg::Popover(UnionPopoverMsg::Confirmed));
    app
}

#[test]
fn create_union_is_one_undo_step() {
    let mut app = app_with_sheet(populated_sheet());
    let before = undo_position(&app);
    select(&mut app, &[("R1", 1), ("R2", 1)], false);
    unions(&mut app, UnionsMsg::CreateRequested);
    // Create waits for a name.
    unions(&mut app, UnionsMsg::Popover(UnionPopoverMsg::Confirmed));
    assert!(doc(&app).unions.is_empty(), "no union without a name");
    drop(app);

    let mut app = app_with_union();
    assert!(app.ui_state.unions.popover.is_none(), "Create closes it");
    let made = &doc(&app).unions[0];
    assert_eq!(made.name, "Input filter");
    assert_eq!(made.tags, ["emc"]);
    assert_eq!(made.members.len(), 2);
    assert_eq!(undo_position(&app), before + 1, "one undo step");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(doc(&app).unions.is_empty());
}

#[test]
fn ctrl_g_creates_a_union_in_the_default_profile_only() {
    let id = AppCommandId::new("create_union").unwrap();
    // The built-ins start on the default profile.
    let mut set = ShortcutProfileSet::built_ins().expect("built-ins parse");
    let expected = if cfg!(target_os = "macos") {
        "\u{2318}G"
    } else {
        "Ctrl+G"
    };
    assert_eq!(
        set.compile_active().shortcut_label(&id).as_deref(),
        Some(expected)
    );
    // The classic profile keeps Ctrl+G for the search panel.
    set.set_active_profile("classic").expect("profile exists");
    assert_eq!(set.compile_active().shortcut_label(&id), None);
}

#[test]
fn hiding_a_union_is_not_an_undo_step_and_changes_no_document() {
    let mut app = app_with_union();
    let path = app.document_state.active_path.clone().unwrap();
    let union = doc(&app).unions[0].uuid;
    let (steps, sheet) = (undo_position(&app), doc(&app).clone());
    panel(
        &mut app,
        UnionsMsg::HideToggled {
            sheet: path.clone(),
            union,
        },
    );
    assert!(app.ui_state.unions.is_hidden(&path, union));
    assert_eq!(undo_position(&app), steps, "hide is not an undo step");
    assert_eq!(doc(&app), &sheet, "the document is unchanged");
    panel(
        &mut app,
        UnionsMsg::HideToggled {
            sheet: path.clone(),
            union,
        },
    );
    assert!(!app.ui_state.unions.is_hidden(&path, union));
}

#[test]
fn a_locked_union_keeps_its_members_until_unlocked() {
    let mut app = app_with_union();
    let path = app.document_state.active_path.clone().unwrap();
    let union = doc(&app).unions[0].uuid;
    panel(
        &mut app,
        UnionsMsg::LockToggled {
            sheet: path.clone(),
            union,
        },
    );
    assert!(doc(&app).unions[0].locked);
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert_eq!(doc(&app).symbols.len(), 5, "a locked union's member stays");
    panel(&mut app, UnionsMsg::LockToggled { sheet: path, union });
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert_eq!(doc(&app).symbols.len(), 4);
    assert_eq!(
        doc(&app).unions[0].members.len(),
        1,
        "the deleted part left its union"
    );
}

#[test]
fn break_union_removes_it_and_keeps_the_objects() {
    let mut app = app_with_union();
    select(&mut app, &[("R1", 1)], false);
    unions(&mut app, UnionsMsg::BreakRequested);
    assert!(doc(&app).unions.is_empty());
    assert_eq!(doc(&app).symbols.len(), 5);
}

#[test]
fn add_and_remove_selection_change_the_members() {
    let mut app = app_with_union();
    select(&mut app, &[("R3", 1)], false);
    unions(&mut app, UnionsMsg::AddSelectionRequested);
    assert_eq!(doc(&app).unions[0].members.len(), 3);
    select(&mut app, &[("R1", 1)], false);
    unions(&mut app, UnionsMsg::RemoveSelectionRequested);
    assert_eq!(doc(&app).unions[0].members.len(), 2);
}
