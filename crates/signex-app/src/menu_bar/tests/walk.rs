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

/// A PCB tab with an empty board, the active document.
fn pcb_tab() -> Signex {
    let mut app = bare_app();
    let path = std::path::PathBuf::from("/w/menu-rows.snxpcb");
    let board = signex_types::pcb::PcbBoard {
        unknown: Default::default(),
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    };
    app.document_state.tabs.push(TabInfo {
        title: "board".into(),
        path,
        cached_document: Some(crate::app::TabDocument::Pcb(board)),
        project_id: None,
        kind: TabKind::Pcb,
    });
    app.document_state.active_tab = 0;
    app.interaction_state
        .pcb_canvas
        .set_viewport(iced::Rectangle::new(
            iced::Point::ORIGIN,
            iced::Size::new(800.0, 600.0),
        ));
    assert!(app.has_active_pcb(), "the fixture shows a board");
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
    ("PCB tab", pcb_tab),
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
    check_command_rows("undocked window", undocked_schematic, the_undocked_window);
}

/// Rows that are live where their click does nothing and says nothing,
/// found by [`every_live_row_does_something`] and left as they are (menu
/// content is the maintainer's call): `(row, states)`. Empty: no button is
/// dead. The list may only shrink.
const KNOWN_INERT: &[(&str, &[&str])] = &[];

/// Live rows of `build`'s app whose click moved nothing, as `(row, state)`.
fn inert_rows(
    state: &'static str,
    build: fn() -> Signex,
    window_of: impl Fn(&mut Signex) -> Option<iced::window::Id>,
) -> Vec<(String, &'static str)> {
    let mut probe_app = build();
    let window = window_of(&mut probe_app);
    let mut inert = Vec::new();
    for (path, item) in all_rows(&probe_app, window) {
        // A panel row shows its panel; with the panel already open there
        // is nothing to do, and its check mark says so.
        if !is_live(&item) || item.checked == Some(true) {
            continue;
        }
        let mut app = build();
        let window = window_of(&mut app);
        let before = probe(&app);
        let task = match window {
            Some(window) => super::window_native_rows::click_in_window(&mut app, window, &path),
            None => click(&mut app, &path),
        };
        if probe(&app) == before && !has_work(&task) {
            inert.push((path, state));
        }
    }
    inert
}

fn undocked_schematic() -> Signex {
    super::window_native_rows::with_undocked_window(schematic_with_history()).0
}

fn the_undocked_window(app: &mut Signex) -> Option<iced::window::Id> {
    app.ui_state.windows.keys().next().copied()
}

/// Rows that cannot show an effect in a fixture because it lacks what a load
/// leaves behind, not because the row is dead: a PCB fit needs the renderer
/// snapshot a real board load builds.
const FIXTURE_LIMITS: &[(&str, &str)] = &[("View/Fit All", "PCB tab")];

#[test]
fn every_live_row_does_something() {
    let mut inert = Vec::new();
    for (state, build) in FIXTURES {
        inert.extend(inert_rows(state, *build, |_| None));
    }
    inert.extend(inert_rows(
        "undocked window",
        undocked_schematic,
        the_undocked_window,
    ));
    inert.retain(|(path, state)| !FIXTURE_LIMITS.contains(&(path.as_str(), state)));
    let known: Vec<(String, &str)> = KNOWN_INERT
        .iter()
        .flat_map(|(path, states)| states.iter().map(move |s| (path.to_string(), *s)))
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

/// #1994: no menu row is a pointer gesture, so none closes the first-run
/// tour card.
#[test]
fn no_menu_row_closes_the_first_run_tour_card() {
    for (state, build) in FIXTURES {
        let probe_app = build();
        for (path, item) in all_rows(&probe_app, None) {
            if !is_live(&item) {
                continue;
            }
            let mut app = build();
            app.ui_state.first_run_tour_open = true;
            let _ = click(&mut app, &path);
            assert!(
                app.ui_state.first_run_tour_open,
                "{path} in {state} closed the tour card"
            );
        }
    }
}
