//! #2037: the Find dialog, command palette, Projects and tab menus, Components
//! drag ghost and queued descents of an undocked window act on the sheet that
//! window shows.

use super::*;
use crate::app::command::CommandArgs;
use crate::app::handlers::document_files::restore_commit_tests::collect_messages;
use crate::app::path_ops::settle_task;
use crate::app::{ContextMenuMsg, ProjectMsg};
use crate::find_replace::FindReplaceMsg;
use crate::keymap::AppCommandId;

const FIND_FOOTER: &str = "Targets: labels, notes, designators, values";

fn command(
    app: &mut Signex,
    id: &str,
    window: iced::window::Id,
) -> (crate::app::command::CommandOutcome, iced::Task<Message>) {
    app.dispatch_command_reporting(
        &AppCommandId::new(id).unwrap(),
        CommandArgs::none(),
        Some(window),
    )
}

// ---- Find ----

#[test]
fn find_opened_in_a_window_searches_and_selects_in_that_windows_sheet() {
    let mut f = fixture_with_a_labelled_a3_window();
    f.app
        .interaction_state
        .canvases
        .get_mut(&f.window)
        .unwrap()
        .selected
        .clear();

    let (_outcome, task) = command(&mut f.app, "find", f.window);
    settle_task(&mut f.app, task);
    assert_eq!(f.app.ui_state.find_replace.window, Some(f.window));
    let task = f
        .app
        .update(Message::FindReplaceMsg(FindReplaceMsg::QueryChanged(
            "WINDOW_NET".into(),
        )));
    settle_task(&mut f.app, task);

    let find = &f.app.ui_state.find_replace;
    assert_eq!(find.matches.len(), 1, "the window's label was not found");
    assert_eq!(
        find.matches[0].sheet.as_deref(),
        Some(f.window_path.as_path())
    );
    let selected = &f.app.interaction_state.canvases[&f.window].selected;
    assert_eq!(selected.len(), 1, "the hit was not selected in the window");
    assert_eq!(selected[0].uuid, label_id());
    assert!(
        f.app.interaction_state.canvas.selected.is_empty(),
        "the main window's selection changed"
    );
}

#[test]
fn the_find_dialog_paints_in_the_window_it_was_opened_in_only() {
    let mut f = fixture();
    let (_outcome, task) = command(&mut f.app, "find", f.window);
    settle_task(&mut f.app, task);
    assert!(has(&f.app, f.window, FIND_FOOTER));
    assert!(!has(&f.app, f.main, FIND_FOOTER));

    // Opened again from the main window, it moves there.
    let (_outcome, task) = command(&mut f.app, "find", f.main);
    settle_task(&mut f.app, task);
    assert!(has(&f.app, f.main, FIND_FOOTER));
    assert!(!has(&f.app, f.window, FIND_FOOTER));
}

#[test]
fn closing_the_window_closes_the_find_dialog_it_opened() {
    let mut f = fixture();
    let (_outcome, task) = command(&mut f.app, "find", f.window);
    settle_task(&mut f.app, task);
    assert!(f.app.ui_state.find_replace.open);

    f.app.close_overlays_owned_by(f.window);

    assert!(!f.app.ui_state.find_replace.open);
    assert_eq!(f.app.ui_state.find_replace.window, None);
}

// ---- Command palette ----

#[test]
fn a_palette_command_runs_on_the_sheet_of_the_window_it_was_opened_in() {
    use crate::app::command_palette::CommandAction;
    let mut f = fixture();
    select_wire_in(f.app.interaction_state.canvases.get_mut(&f.window).unwrap());
    assert!(f.app.interaction_state.canvas.selected.is_empty());

    let (_outcome, task) = command(&mut f.app, "open_command_palette", f.window);
    settle_task(&mut f.app, task);
    assert_eq!(f.app.ui_state.command_palette.window, Some(f.window));
    f.app.ui_state.command_palette.query = "delete".into();
    f.app.refresh_command_palette_results();
    let row = f
        .app
        .ui_state
        .command_palette
        .results
        .iter()
        .position(|entry| {
            matches!(&entry.action, CommandAction::Command(id) if id.as_str() == "delete_selection")
        })
        .expect("the window's selection enables Delete, so the palette offers it");
    f.app.ui_state.command_palette.selected_index = row;

    let task = f.app.update(Message::CommandPalette(
        crate::app::CommandPaletteMsg::ExecuteSelected,
    ));
    settle_task(&mut f.app, task);

    assert!(
        f.app.document_state.engines[&f.window_path]
            .document()
            .wires
            .is_empty(),
        "the window's wire was not deleted"
    );
}

#[test]
fn the_palette_offers_the_main_windows_rows_when_opened_there() {
    let mut f = fixture();
    select_wire_in(f.app.interaction_state.canvases.get_mut(&f.window).unwrap());
    let (_outcome, task) = command(&mut f.app, "open_command_palette", f.main);
    settle_task(&mut f.app, task);
    assert_eq!(f.app.ui_state.command_palette.window, None);
    f.app.ui_state.command_palette.query = "delete".into();
    f.app.refresh_command_palette_results();
    assert!(
        !f.app
            .ui_state
            .command_palette
            .results
            .iter()
            .any(|entry| matches!(
                &entry.action,
                crate::app::command_palette::CommandAction::Command(id)
                    if id.as_str() == "delete_selection"
            )),
        "main has nothing selected, so Delete is not offered"
    );
}

// ---- Tab-strip and project-tree menus ----

#[test]
fn the_tab_menu_paints_in_the_main_window_only() {
    let mut f = fixture();
    let _ = f
        .app
        .update(Message::ContextMenu(ContextMenuMsg::ShowTab(0)));
    assert!(has(&f.app, f.main, "Close All Documents"));
    assert!(!has(&f.app, f.window, "Close All Documents"));
}

fn workspace_with_a_window() -> Fixture {
    let dir = tempfile::tempdir().unwrap().keep();
    let app = crate::app::handlers::menu::export::tests::app_workspace(
        &dir.to_string_lossy(),
        &["a.snxsch"],
    );
    let mut f = fixture_in_dirs(app, &dir, &dir);
    f.app.document_state.active_project = Some(f.app.document_state.projects[0].id);
    f.app.refresh_panel_ctx();
    let _ = f.app.update(Message::noop());
    f
}

fn project_row_key(f: &Fixture) -> signex_widgets::tree_view::TreeKey {
    signex_widgets::tree_view::key_of(f.app.panel_ctx_for(f.window).project_tree.as_slice(), &[0])
        .expect("the window's tree has the project row")
}

#[test]
fn the_projects_menu_right_clicked_in_a_window_belongs_to_that_window() {
    use signex_widgets::tree_view::TreeMsg;
    let mut f = workspace_with_a_window();
    let key = project_row_key(&f);
    let right_click = f.app.scope_dock_message(
        f.window,
        dock(crate::panels::PanelMsg::Tree(TreeMsg::ContextMenu(key))),
    );
    assert!(is_scoped(&right_click, f.window));
    let _ = f.app.update(right_click);

    let menu = f.app.interaction_state.project_tree_context_menu.as_ref();
    assert_eq!(menu.map(|m| m.window), Some(Some(f.window)));
    assert!(has(&f.app, f.window, "Validate Project"));
    assert!(!has(&f.app, f.main, "Validate Project"));
    // A pick in the window carries the window.
    let clicked =
        crate::layout_probe::messages_on_click(f.app.view(f.window), SIZE, "Validate Project");
    assert!(
        clicked.iter().any(|m| matches!(
            m,
            Message::ContextMenu(ContextMenuMsg::ScopedToWindow(w, _)) if *w == f.window
        )),
        "the pick was not scoped to the window: {clicked:?}"
    );
}

#[test]
fn a_projects_menu_pick_that_opens_a_tab_runs_on_the_main_window() {
    let mut f = workspace_with_a_window();
    let key = project_row_key(&f);
    let pick = Message::ContextMenu(ContextMenuMsg::ScopedToWindow(
        f.window,
        Box::new(ContextMenuMsg::ProjectTreeAction(
            crate::app::ProjectTreeAction::OpenProjectOptions(key),
        )),
    ));
    let task = f.app.update(pick);
    settle_task(&mut f.app, task);
    let state = &f.app.document_state;
    assert!(
        matches!(
            state.tabs.get(state.active_tab).map(|tab| &tab.kind),
            Some(crate::app::TabKind::Project(_))
        ),
        "the Project tab did not become the main window's active tab"
    );
}

#[test]
fn a_projects_menu_pick_that_collapses_the_tree_changes_the_windows_tree_only() {
    let mut f = workspace_with_a_window();
    assert!(f.app.panel_ctx_for(f.window).project_tree[0].expanded);
    assert!(f.app.document_state.panel_ctx.project_tree[0].expanded);
    let pick = Message::ContextMenu(ContextMenuMsg::ScopedToWindow(
        f.window,
        Box::new(ContextMenuMsg::ProjectTreeAction(
            crate::app::ProjectTreeAction::CollapseAll,
        )),
    ));
    let _ = f.app.update(pick);
    assert!(!f.app.panel_ctx_for(f.window).project_tree[0].expanded);
    assert!(f.app.document_state.panel_ctx.project_tree[0].expanded);
}

// ---- Components drag ghost ----

fn canvas_centre(f: &Fixture, window: Option<iced::window::Id>) -> (f32, f32) {
    let bounds = f.app.canvas_bounds_in(window);
    (
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    )
}

#[test]
fn a_row_dragged_over_a_windows_canvas_shows_its_ghost_there() {
    let mut f = fixture();
    pressed_row(&mut f);
    let (x, y) = canvas_centre(&f, Some(f.window));
    let epoch = f.app.interaction_state.placement_epoch;

    let _ = f.app.update(Message::Ui(crate::app::UiMsg::DragMovedIn {
        window: f.window,
        x,
        y,
    }));

    let drag = f.app.library.components_panel.drag.as_ref().unwrap();
    assert!(drag.armed, "the ghost was not armed for the window");
    assert!(f.app.interaction_state.placement_epoch > epoch);
}

#[test]
fn a_ghost_does_not_follow_a_pointer_that_is_in_another_window() {
    let mut f = fixture();
    pressed_row(&mut f);
    let (x, y) = canvas_centre(&f, Some(f.window));

    // The same coordinates, but in the main window: not the pressed one.
    let _ = f.app.update(Message::Ui(crate::app::UiMsg::DragMovedIn {
        window: f.main,
        x,
        y,
    }));

    assert!(!f.app.library.components_panel.drag.as_ref().unwrap().armed);
}

// ---- Descent race ----

fn queued_descent(d: &mut Descent) -> Message {
    let (_outcome, task) = command(&mut d.app, "placement_accept", d.window);
    collect_messages(task)
        .into_iter()
        .find(|m| matches!(m, Message::Project(ProjectMsg::DescendRequested { .. })))
        .expect("the descent was queued to the main window")
}

#[test]
fn a_queued_descent_is_refused_when_its_window_closed_meanwhile() {
    let _ = crate::diagnostics::init_logging();
    let mut d = descent_fixture(true);
    let queued = queued_descent(&mut d);
    let before = d.app.document_state.active_path.clone();
    d.app.ui_state.windows.remove(&d.window);
    d.app.interaction_state.canvases.remove(&d.window);

    let task = d.app.update(queued);
    settle_task(&mut d.app, task);

    assert_eq!(d.app.document_state.active_path, before);
    assert!(said("the window it started in has closed"));
}

#[test]
fn a_queued_descent_is_refused_when_its_window_shows_another_sheet_now() {
    let _ = crate::diagnostics::init_logging();
    let mut d = descent_fixture(true);
    let queued = queued_descent(&mut d);
    let before = d.app.document_state.active_path.clone();
    let other = d.child.clone();
    d.app.ui_state.windows.insert(
        d.window,
        WindowKind::UndockedTab {
            path: other,
            title: "child".into(),
        },
    );

    let task = d.app.update(queued);
    settle_task(&mut d.app, task);

    assert_eq!(d.app.document_state.active_path, before);
    assert!(said("the window shows another sheet now"));
}

#[test]
fn a_queued_descent_is_refused_when_its_sheet_symbol_is_gone() {
    let _ = crate::diagnostics::init_logging();
    let mut d = descent_fixture(true);
    let queued = queued_descent(&mut d);
    let before = d.app.document_state.active_path.clone();
    let Message::Project(ProjectMsg::DescendRequested { symbol, .. }) = &queued else {
        unreachable!();
    };
    d.app
        .document_state
        .engines
        .get_mut(&d.top)
        .unwrap()
        .execute(signex_engine::Command::DeleteSelection {
            items: vec![SelectedItem::new(*symbol, SelectedKind::ChildSheet)],
        })
        .unwrap();

    let task = d.app.update(queued);
    settle_task(&mut d.app, task);

    assert_eq!(d.app.document_state.active_path, before);
    assert!(said("the sheet symbol is gone from the sheet"));
}

#[test]
fn a_queued_descent_is_refused_when_its_sheet_symbol_points_elsewhere_now() {
    let _ = crate::diagnostics::init_logging();
    let mut d = descent_fixture(true);
    let queued = queued_descent(&mut d);
    let before = d.app.document_state.active_path.clone();
    let Message::Project(ProjectMsg::DescendRequested { symbol, .. }) = &queued else {
        unreachable!();
    };
    d.app
        .document_state
        .engines
        .get_mut(&d.top)
        .unwrap()
        .execute(signex_engine::Command::ModifyChildSheet {
            sheet_id: *symbol,
            name: None,
            filename: Some("other.snxsch".into()),
        })
        .unwrap();

    let task = d.app.update(queued);
    settle_task(&mut d.app, task);

    assert_eq!(d.app.document_state.active_path, before);
    assert!(said("the sheet symbol references another file now"));
}

#[test]
fn a_queued_descent_that_still_applies_opens_the_child() {
    let mut d = descent_fixture(true);
    let queued = queued_descent(&mut d);
    let task = d.app.update(queued);
    settle_task(&mut d.app, task);
    assert_eq!(d.app.document_state.active_path, Some(d.child.clone()));
}

// ---- Project Replace and Find hits started from a window (review of #2068) ----

fn net_label(id: u128, text: &str) -> signex_types::schematic::Label {
    signex_types::schematic::Label {
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::from_u128(id),
        text: text.into(),
        position: signex_types::schematic::Point::new(30.48, 30.48),
        rotation: 0.0,
        label_type: signex_types::schematic::LabelType::Net,
        shape: String::new(),
        font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
        font_family: None,
        justify: Default::default(),
        justify_v: Default::default(),
        locked: false,
    }
}

fn place_label(app: &mut Signex, path: &Path, id: u128, text: &str) {
    app.document_state
        .engines
        .get_mut(path)
        .unwrap()
        .execute(signex_engine::Command::PlaceLabel {
            label: net_label(id, text),
        })
        .unwrap();
}

#[test]
fn a_project_replace_started_in_a_window_finishes_in_that_windows_project() {
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let app = crate::app::handlers::menu::export::tests::app_workspace(
        &dir_a.path().to_string_lossy(),
        &["a.snxsch"],
    );
    let mut f = fixture_in_dirs(app, dir_a.path(), dir_b.path());
    let mut second = f.app.document_state.projects[0].clone();
    second.id = f.app.document_state.mint_project_id();
    second.path = dir_b.path().join("ProjectB.snxprj");
    second.data.name = "ProjectB".into();
    second.data.dir = dir_b.path().to_string_lossy().into_owned();
    second.data.schematic_root = Some("b.snxsch".into());
    second.data.sheets[0].filename = "b.snxsch".into();
    f.app.document_state.projects.push(second);
    let (main_path, window_path) = (f.main_path.clone(), f.window_path.clone());
    place_label(&mut f.app, &main_path, 21, "A_NET");
    place_label(&mut f.app, &window_path, 22, "B_NET");

    let (_outcome, task) = command(&mut f.app, "find_and_replace", f.window);
    settle_task(&mut f.app, task);
    let mut messages = Vec::new();
    for message in [
        FindReplaceMsg::ScopeChanged(crate::find_replace::FindScope::Project),
        FindReplaceMsg::QueryChanged("NET".into()),
        FindReplaceMsg::ReplacementChanged("NETX".into()),
    ] {
        messages.push(Message::FindReplaceMsg(message));
    }
    feed(&mut f.app, messages);
    let window = f.window;
    let mut prepared = None;
    let task = f.app.run_in_window_document(window, |app| {
        app.run_project_scan_blocking();
        let matches = app.ui_state.find_replace.matches.clone();
        prepared = app.prepare_project_replace_blocking(matches);
        iced::Task::none()
    });
    settle_task(&mut f.app, task);
    let (generation, prep) = prepared.expect("the replace was prepared");

    let task = f
        .app
        .update(Message::FindReplaceMsg(FindReplaceMsg::ProjectReplacePrepared {
            generation,
            result: std::sync::Arc::new(Ok(prep)),
        }));
    settle_task(&mut f.app, task);

    let matches = &f.app.ui_state.find_replace.matches;
    assert!(!matches.is_empty(), "the rescan found nothing");
    assert!(
        matches
            .iter()
            .all(|m| m.sheet.as_deref().is_some_and(|s| s.starts_with(dir_b.path()))),
        "a hit outside the window's project: {:?}",
        matches.iter().map(|m| m.sheet.clone()).collect::<Vec<_>>()
    );
}

fn find_hit_on(sheet: &Path, label: uuid::Uuid) -> crate::find_replace::FindMatch {
    crate::find_replace::FindMatch {
        sheet: Some(sheet.to_path_buf()),
        hit: signex_engine::FindHit {
            kind: signex_engine::FindKind::NetLabel,
            item: SelectedItem::new(label, SelectedKind::Label),
            target: signex_engine::TextTarget::Label(label),
            text: "FAR_NET".into(),
            placement: None,
            single: false,
            position: Point::new(30.48, 30.48),
        },
        placement_label: None,
    }
}

#[test]
fn a_find_hit_on_another_sheet_clicked_in_a_window_opens_in_the_main_window() {
    let mut f = fixture();
    let far = PathBuf::from("c.snxsch");
    let engine =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    f.app.document_state.engines.insert(far.clone(), engine);
    f.app.document_state.tabs.push(tab(&far));
    place_label(&mut f.app, &far, 31, "FAR_NET");
    let (_outcome, task) = command(&mut f.app, "find", f.window);
    settle_task(&mut f.app, task);
    f.app.ui_state.find_replace.matches = vec![find_hit_on(&far, uuid::Uuid::from_u128(31))];

    let task = f
        .app
        .update(Message::FindReplaceMsg(FindReplaceMsg::SelectResult(0)));
    settle_task(&mut f.app, task);

    assert_eq!(f.app.document_state.active_path, Some(far));
    assert_eq!(f.app.interaction_state.active_canvas().selected.len(), 1);
}

#[test]
fn find_next_in_a_window_opens_a_hit_on_another_sheet_in_the_main_window() {
    let mut f = fixture();
    let far = PathBuf::from("c.snxsch");
    let engine =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    f.app.document_state.engines.insert(far.clone(), engine);
    f.app.document_state.tabs.push(tab(&far));
    place_label(&mut f.app, &far, 31, "FAR_NET");
    let (_outcome, task) = command(&mut f.app, "find", f.window);
    settle_task(&mut f.app, task);
    let find = &mut f.app.ui_state.find_replace;
    find.scope = crate::find_replace::FindScope::Project;
    find.matches = vec![find_hit_on(&far, uuid::Uuid::from_u128(31))];

    let task = f
        .app
        .update(Message::FindReplaceMsg(FindReplaceMsg::NextRequested));
    settle_task(&mut f.app, task);

    assert_eq!(f.app.document_state.active_path, Some(far));
}

// ---- Palette gate, pointer mirror ----

#[test]
fn the_command_palette_paints_in_the_window_it_was_opened_in_only() {
    let mut f = fixture();
    let (_outcome, task) = command(&mut f.app, "open_command_palette", f.window);
    settle_task(&mut f.app, task);
    f.app.ui_state.command_palette.query = "zzzz_no_such_row".into();
    f.app.refresh_command_palette_results();
    assert!(has(&f.app, f.window, "No results"));
    assert!(!has(&f.app, f.main, "No results"));
}

#[test]
fn a_drag_end_forgets_the_window_the_pointer_was_last_in() {
    let mut f = fixture();
    f.app.interaction_state.pointer_window = Some(f.window);
    let task = f.app.update(Message::Ui(crate::app::UiMsg::DragEnd(f.main)));
    settle_task(&mut f.app, task);
    assert_eq!(f.app.interaction_state.pointer_window, None);
}
