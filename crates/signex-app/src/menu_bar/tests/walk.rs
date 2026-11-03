//! No button without a function (#1922): every row of the bar, in each kind
//! of window state, is either greyed or does something.
//!
//! Rows that run a command id are held to the stronger rule, with the
//! `CommandOutcome` of #1928 as the proof: a live row's command `Ran`, and a
//! greyed row's command is `Disabled`, never the other way round, and never
//! `UnknownId` or `NoBridgeArm`.

use super::*;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::menu_bar::MenuMessage;
use signex_types::schematic::{SelectedItem, SelectedKind};

fn schematic_without_selection() -> Signex {
    app_with(sheet_of(vec![symbol(1, "R1", 10.0, 10.0)]))
}

/// A schematic with a selection, a copy on the clipboard and one undo step.
fn schematic_with_history() -> Signex {
    let mut app = app_with(sheet_of(vec![
        symbol(1, "R1", 10.0, 10.0),
        symbol(2, "R2", 30.0, 10.0),
        symbol(3, "R3", 50.0, 10.0),
    ]));
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(Uuid::from_u128(1), SelectedKind::Symbol)];
    let _ = app.update(Message::Edit(crate::app::EditMsg::Copy));
    let _ = app.update(Message::Edit(crate::app::EditMsg::DeleteSelected));
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(Uuid::from_u128(2), SelectedKind::Symbol)];
    app
}

fn dirty_schematic() -> Signex {
    let mut app = schematic_without_selection();
    let path = app.document_state.active_path.clone().expect("path");
    app.document_state.mark_document_dirty(&path);
    app
}

fn symbol_editor_on_part_two() -> Signex {
    let mut app = symbol_editor_app();
    let path = std::path::PathBuf::from("/w/menu-rows.snxsym");
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    editor.primitive_mut().part_count = 2;
    editor.active_part = 2;
    app
}

fn footprint_editor_with_history() -> Signex {
    let mut app = footprint_editor_app();
    let path = std::path::PathBuf::from("/w/menu-rows.snxfpt");
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .push_history();
    app
}

fn project_without_document() -> Signex {
    let mut app = bare_app();
    open_project(&mut app, "P.snxprj");
    app
}

const FIXTURES: &[Fixture] = &[
    ("no document", bare_app),
    ("project, no document", project_without_document),
    ("schematic, nothing selected", schematic_without_selection),
    ("schematic, selection and history", schematic_with_history),
    ("schematic, unsaved", dirty_schematic),
    ("symbol editor", symbol_editor_app),
    ("symbol editor, second part", symbol_editor_on_part_two),
    ("footprint editor, history", footprint_editor_with_history),
];

/// Every row of the bar as `(path, row)`, submenus included.
fn all_rows(app: &Signex, window: Option<iced::window::Id>) -> Vec<(String, MenuItem)> {
    fn walk(menu: &Menu, parent: &str, out: &mut Vec<(String, MenuItem)>) {
        for entry in &menu.entries {
            match entry {
                MenuEntry::Item(item) => {
                    out.push((format!("{parent}/{}", item.label), item.clone()))
                }
                MenuEntry::Submenu(sub) => walk(sub, &format!("{parent}/{}", sub.title), out),
                MenuEntry::Separator => {}
            }
        }
    }
    let ctx = MenuContext {
        commands: app.command_context(window),
        ..menu_context(app)
    };
    let mut out = Vec::new();
    for menu in &menu_bar_model(&ctx).menus {
        walk(menu, &menu.title, &mut out);
    }
    out
}

fn is_live(item: &MenuItem) -> bool {
    item.is_enabled()
}

/// A live row's command ran, a greyed row's command is disabled.
fn check_command_rows(
    state: &str,
    build: fn() -> Signex,
    window_of: impl Fn(&mut Signex) -> Option<iced::window::Id>,
) {
    let mut probe_app = build();
    let window = window_of(&mut probe_app);
    let mut checked = 0;
    for (path, item) in all_rows(&probe_app, window) {
        let ItemState::Live {
            action: MenuMessage::Command(id),
            ..
        } = &item.state
        else {
            continue;
        };
        let mut app = build();
        let window = window_of(&mut app);
        let (outcome, _task) = app.dispatch_command_reporting(id, CommandArgs::none(), window);
        if is_live(&item) {
            assert_eq!(
                outcome,
                CommandOutcome::Ran,
                "{path} in {state}: the row is live but its command did not run"
            );
        } else {
            assert!(
                matches!(outcome, CommandOutcome::Disabled { .. }),
                "{path} in {state}: the row is greyed but its command gave {outcome:?}"
            );
        }
        checked += 1;
    }
    assert!(
        checked >= 30,
        "{state}: only {checked} command rows checked"
    );
}

#[test]
fn a_live_command_row_runs_and_a_greyed_one_is_disabled_in_every_state() {
    for (state, build) in FIXTURES {
        check_command_rows(state, *build, |_| None);
    }
}

#[test]
fn the_same_holds_in_an_undocked_windows_own_bar() {
    let undocked: fn() -> Signex =
        || super::window_native_rows::with_undocked_window(schematic_with_history()).0;
    check_command_rows("undocked window", undocked, |app| {
        app.ui_state.windows.keys().next().copied()
    });
}

/// Rows that are live in a state where their click only reports in the
/// Messages panel: `(row, state, the line)`.
const LOG_ONLY: &[(&str, &str, &str)] = &[
    (
        "Design/Annotation/Annotate Schematics Quietly",
        "project, no document",
        "no sheet is open, nothing to do",
    ),
    (
        "Design/Annotation/Force Annotate All Schematics",
        "project, no document",
        "no sheet is open, nothing to do",
    ),
];

/// Rows that are live where their click does nothing and says nothing,
/// found by [`every_live_row_does_something`] and left as they are (menu
/// content is the maintainer's call): `(row, states)`. The list may only
/// shrink.
const KNOWN_INERT: &[(&str, &[&str])] = &[
    // Acts on the schematic or PCB canvas only.
    (
        "View/Zoom In",
        &[
            "no document",
            "project, no document",
            "symbol editor",
            "symbol editor, second part",
            "footprint editor, history",
        ],
    ),
    (
        "View/Zoom Out",
        &[
            "no document",
            "project, no document",
            "symbol editor",
            "symbol editor, second part",
            "footprint editor, history",
        ],
    ),
    // Acts only in a symbol or footprint editor inside an open library.
    (
        "Tools/Document Options...",
        &[
            "no document",
            "project, no document",
            "schematic, nothing selected",
            "schematic, selection and history",
            "schematic, unsaved",
            "symbol editor",
            "symbol editor, second part",
            "footprint editor, history",
        ],
    ),
];

#[test]
fn every_live_row_does_something() {
    let _ = crate::diagnostics::init_logging();
    let mut inert: std::collections::BTreeMap<String, Vec<&str>> = Default::default();
    for (state, build) in FIXTURES {
        let probe_app = build();
        for (path, item) in all_rows(&probe_app, None) {
            // A panel row shows its panel; with the panel already open there
            // is nothing to do, and its check mark says so.
            if !is_live(&item) || item.checked == Some(true) {
                continue;
            }
            let log_only = LOG_ONLY.iter().find(|(p, s, _)| *p == path && s == state);
            let logged_before =
                log_only.map(|(_, _, needle)| crate::diagnostics::logged_count(needle));
            let mut app = build();
            let before = probe(&app);
            let task = click(&mut app, &path);
            if probe(&app) != before || has_work(&task) {
                assert!(log_only.is_none(), "{path} in {state} no longer only logs");
                continue;
            }
            match (log_only, logged_before) {
                (Some((_, _, needle)), Some(count)) => assert!(
                    crate::diagnostics::logged_count(needle) > count,
                    "{path} in {state}: nothing logged"
                ),
                _ => inert.entry(path).or_default().push(state),
            }
        }
    }
    let known: std::collections::BTreeMap<String, Vec<&str>> = KNOWN_INERT
        .iter()
        .map(|(path, states)| (path.to_string(), states.to_vec()))
        .collect();
    assert_eq!(inert, known, "live rows whose click does nothing");
}

/// Every command id a row carries resolves to a catalog command, and a row
/// that does not run its id is one of the two known exceptions.
#[test]
fn command_rows_name_real_commands_and_only_two_rows_keep_their_own_action() {
    let app = one_symbol_selected();
    let mut own_action = Vec::new();
    for (path, item) in all_rows(&app, None) {
        let Some(shortcut) = &item.shortcut else {
            continue;
        };
        assert!(
            crate::keymap::metadata_for(&shortcut.command_id).is_some(),
            "{path} names {} which is not in the catalog",
            shortcut.command_id.as_str()
        );
        let ItemState::Live { action, .. } = &item.state else {
            continue;
        };
        if !matches!(action, MenuMessage::Command(id) if *id == shortcut.command_id) {
            own_action.push(path);
        }
    }
    assert_eq!(
        own_action,
        ["View/Cycle Grid Size", "Design/Electrical Rules Check"]
    );
}
