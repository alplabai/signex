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

    let task = f.app.update(Message::FindReplaceMsg(
        FindReplaceMsg::ProjectReplacePrepared {
            generation,
            result: std::sync::Arc::new(Ok(prep)),
        },
    ));
    settle_task(&mut f.app, task);

    let matches = &f.app.ui_state.find_replace.matches;
    assert!(!matches.is_empty(), "the rescan found nothing");
    assert!(
        matches.iter().all(|m| m
            .sheet
            .as_deref()
            .is_some_and(|s| s.starts_with(dir_b.path()))),
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
    let task = f
        .app
        .update(Message::Ui(crate::app::UiMsg::DragEnd(f.main)));
    settle_task(&mut f.app, task);
    assert_eq!(f.app.interaction_state.pointer_window, None);
}

// ---- History panel, F3 and palette input per window (#2067) ----

use crate::app::UiMsg;
use crate::panels::history::HistoryRenderMode;

#[test]
fn a_windows_history_panel_shows_the_windows_own_file() {
    let mut f = fixture();
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);

    let window_ctx = &f.app.document_state.window_panel_ctx[&f.window];
    assert_eq!(
        window_ctx.history.active_path.as_deref(),
        Some(f.window_path.as_path())
    );
    assert_eq!(
        f.app
            .document_state
            .panel_ctx
            .history
            .active_path
            .as_deref(),
        Some(f.main_path.as_path()),
        "main's History moved"
    );
}

fn dock_restore(f: &Fixture, message: crate::panels::PanelMsg) -> Message {
    f.app.scope_dock_message(
        f.window,
        Message::Dock(crate::dock::DockMessage::Panel(message)),
    )
}

/// Present the window's History as a loaded list, as a finished load would.
fn load_window_history(f: &mut Fixture) {
    let history = f
        .app
        .document_state
        .window_history
        .get_mut(&f.window)
        .unwrap();
    assert_eq!(
        history.active_path.as_deref(),
        Some(f.window_path.as_path())
    );
    history.mode = HistoryRenderMode::Ready;
}

#[test]
fn a_restore_click_in_a_window_acts_on_the_windows_file_only() {
    let _ = crate::diagnostics::init_logging();
    let mut f = fixture();
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);
    load_window_history(&mut f);
    f.app.document_state.mark_history_edit_dirty(&f.window_path);
    let before = f.app.document_state.engines[&f.window_path].revision();
    let click = || crate::panels::PanelMsg::HistoryRestoreClicked { sha: "abc".into() };

    // The window's sheet has unsaved edits: the first click only arms the
    // confirmation, in the window's own History state.
    assert!(f.app.document_state.is_document_dirty(&f.window_path));
    let sent = dock_restore(&f, click());
    assert!(is_scoped(&sent, f.window));
    let task = f.app.update(sent);
    settle_task(&mut f.app, task);
    assert_eq!(
        f.app.document_state.window_history[&f.window]
            .pending_restore
            .as_deref(),
        Some("abc")
    );
    assert_eq!(f.app.document_state.history.pending_restore, None);
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
    assert_eq!(
        f.app.document_state.engines[&f.window_path].revision(),
        before
    );

    // The confirming click goes on to the window's file: the refusal (no
    // project holds it here) names it, never main's.
    let sent = dock_restore(&f, click());
    let task = f.app.update(sent);
    settle_task(&mut f.app, task);
    assert!(said("b.snxsch does not belong to an open project"));
    assert!(!said("a.snxsch does not belong to an open project"));
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

#[test]
fn a_window_restore_cancel_clears_the_windows_confirmation_only() {
    let mut f = fixture();
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);
    load_window_history(&mut f);
    f.app.document_state.mark_history_edit_dirty(&f.window_path);
    f.app.document_state.history.pending_restore = Some("main-sha".into());
    let click = crate::panels::PanelMsg::HistoryRestoreClicked { sha: "abc".into() };
    let task = f.app.update(dock_restore(&f, click));
    settle_task(&mut f.app, task);
    assert_eq!(
        f.app.document_state.window_history[&f.window]
            .pending_restore
            .as_deref(),
        Some("abc"),
        "the click did not arm the window's confirmation"
    );

    let cancel = dock_restore(&f, crate::panels::PanelMsg::HistoryRestoreCancelled);
    let task = f.app.update(cancel);
    settle_task(&mut f.app, task);

    assert_eq!(
        f.app.document_state.window_history[&f.window].pending_restore,
        None
    );
    assert_eq!(
        f.app.document_state.history.pending_restore.as_deref(),
        Some("main-sha")
    );
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

#[test]
fn a_restore_click_on_a_list_the_window_has_not_loaded_is_refused() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let (app, _boot) = Signex::new();
    let mut f = fixture_in(app, dir.path());
    // Main lists a.snxsch. The window's context is built the way UndockTab
    // builds it: no finish_update, so no load for the window has run.
    f.app.document_state.history.active_path = Some(f.main_path.clone());
    f.app.document_state.history.mode = HistoryRenderMode::Ready;
    f.app.document_state.panel_ctx.history = f.app.document_state.history.clone();
    f.app.refresh_window_panel_ctxs(None);
    assert_ne!(
        f.app.panel_ctx_for(f.window).history.active_path.as_deref(),
        Some(f.main_path.as_path()),
        "the window's History panel lists main's file"
    );
    // b.snxsch is clean, so an unguarded click restores at once.
    f.app.document_state.window_history.insert(
        f.window,
        crate::panels::history::HistoryPanelState {
            active_path: Some(f.main_path.clone()),
            mode: HistoryRenderMode::Ready,
            ..Default::default()
        },
    );
    let sent = dock_restore(
        &f,
        crate::panels::PanelMsg::HistoryRestoreClicked { sha: "abc".into() },
    );
    let task = f.app.update(sent);
    settle_task(&mut f.app, task);
    assert!(
        said(&format!(
            "The history of {} is not loaded",
            f.window_path.display()
        )),
        "the click was not refused"
    );
    assert!(!said(&format!(
        "{} does not belong to an open project",
        f.window_path.display()
    )));
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

#[test]
fn a_window_without_any_history_state_refuses_a_restore_click() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let (app, _boot) = Signex::new();
    let mut f = fixture_in(app, dir.path());
    f.app.document_state.window_history.remove(&f.window);
    let sent = dock_restore(
        &f,
        crate::panels::PanelMsg::HistoryRestoreClicked { sha: "abc".into() },
    );
    let task = f.app.update(sent);
    settle_task(&mut f.app, task);
    assert!(said(&format!(
        "The history of {} is not loaded",
        f.window_path.display()
    )));
}

#[test]
fn a_restore_whose_list_was_issued_for_another_file_is_refused_on_main_too() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let (app, _boot) = Signex::new();
    let mut f = fixture_in(app, dir.path());
    // Main shows a.snxsch, but its list was issued for b.snxsch.
    f.app.document_state.history.active_path = Some(f.window_path.clone());
    f.app.document_state.history.mode = HistoryRenderMode::Ready;
    let click = Message::Dock(crate::dock::DockMessage::Panel(
        crate::panels::PanelMsg::HistoryRestoreClicked { sha: "abc".into() },
    ));
    let task = f.app.update(click);
    settle_task(&mut f.app, task);
    assert!(said(&format!(
        "The history of {} is not loaded",
        f.main_path.display()
    )));
    assert!(!said(&format!(
        "{} does not belong to an open project",
        f.main_path.display()
    )));

    // Not Ready is refused even for the right file.
    f.app.document_state.history.active_path = Some(f.main_path.clone());
    f.app.document_state.history.mode = HistoryRenderMode::Loading;
    let click = Message::Dock(crate::dock::DockMessage::Panel(
        crate::panels::PanelMsg::HistoryRestoreClicked { sha: "abc".into() },
    ));
    let task = f.app.update(click);
    settle_task(&mut f.app, task);
    assert!(!said(&format!(
        "{} does not belong to an open project",
        f.main_path.display()
    )));
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

#[test]
fn a_history_answer_for_a_window_lands_in_that_windows_history() {
    let mut f = fixture();
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);
    let generation = f.app.document_state.window_history[&f.window].generation;

    let task = f.app.update(Message::Ui(UiMsg::HistoryLoaded {
        window: Some(f.window),
        generation,
        path: f.window_path.clone(),
        result: Err("boom".into()),
    }));
    settle_task(&mut f.app, task);

    assert_eq!(
        f.app.document_state.window_history[&f.window].mode,
        HistoryRenderMode::Error("boom".into())
    );
    assert_ne!(
        f.app.document_state.history.mode,
        HistoryRenderMode::Error("boom".into())
    );
}

#[test]
fn a_history_answer_for_a_closed_window_is_dropped_and_said() {
    let _ = crate::diagnostics::init_logging();
    let mut f = fixture();
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);
    let generation = f.app.document_state.window_history[&f.window].generation;
    f.app.ui_state.windows.remove(&f.window);
    f.app.interaction_state.canvases.remove(&f.window);
    f.app.close_overlays_owned_by(f.window);

    let task = f.app.update(Message::Ui(UiMsg::HistoryLoaded {
        window: Some(f.window),
        generation,
        path: f.window_path.clone(),
        result: Err("boom".into()),
    }));
    settle_task(&mut f.app, task);

    assert!(said("the window that asked for it has closed"));
    assert!(!f.app.document_state.window_history.contains_key(&f.window));
    assert_ne!(
        f.app.document_state.history.mode,
        HistoryRenderMode::Error("boom".into())
    );
}

#[test]
fn f3_in_a_window_without_find_searches_that_windows_sheet() {
    let mut f = fixture_with_a_labelled_a3_window();
    f.app
        .interaction_state
        .canvases
        .get_mut(&f.window)
        .unwrap()
        .selected
        .clear();
    // Find is open in the main window, with the label's text as its query.
    f.app.ui_state.find_replace.query = "WINDOW_NET".into();
    f.app.ui_state.find_replace.window = None;
    f.app.ui_state.find_replace.open = true;

    let (_outcome, task) = command(&mut f.app, "find_next", f.window);
    settle_task(&mut f.app, task);

    assert_eq!(f.app.ui_state.find_replace.window, Some(f.window));
    let selected = &f.app.interaction_state.canvases[&f.window].selected;
    assert_eq!(selected.len(), 1, "the window's sheet was not searched");
    assert_eq!(selected[0].uuid, label_id());
    assert!(f.app.interaction_state.canvas.selected.is_empty());
}

#[test]
fn typing_in_a_windows_palette_filters_its_rows_and_leaves_main_alone() {
    let mut f = fixture();
    let (_outcome, task) = command(&mut f.app, "open_command_palette", f.window);
    settle_task(&mut f.app, task);
    assert_eq!(f.app.ui_state.command_palette.window, Some(f.window));
    let all = f.app.ui_state.command_palette.total_matches;

    let task = f.app.update(Message::CommandPalette(
        crate::app::CommandPaletteMsg::QueryTypedInWindow(f.window, "zzzz_no_such_row".into()),
    ));
    settle_task(&mut f.app, task);

    let palette = &f.app.ui_state.command_palette;
    assert_eq!(palette.query, "zzzz_no_such_row");
    assert_eq!(palette.total_matches, 0);
    assert!(all > 0);
    assert_eq!(palette.window, Some(f.window), "the palette moved to main");
    assert!(has(&f.app, f.window, "No results"));
    assert!(!has(&f.app, f.main, "No results"));
    assert!(
        !has(&f.app, f.main, "zzzz_no_such_row"),
        "main's chrome input shows the window's query"
    );
}

fn entry(sha: &str) -> signex_widgets::HistoryEntry {
    signex_widgets::HistoryEntry {
        sha: sha.into(),
        author_name: String::new(),
        author_email: String::new(),
        time: chrono::Utc::now(),
        subject: String::new(),
    }
}

#[test]
fn a_pcb_window_has_its_own_history_and_restores_the_pcb_file() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let (app, _boot) = Signex::new();
    let mut f = fixture_in(app, dir.path());
    f.app.document_state.tabs[1].kind = crate::app::TabKind::Pcb;
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);
    load_window_history(&mut f);
    assert_eq!(
        f.app.panel_ctx_for(f.window).history.active_path.as_deref(),
        Some(f.window_path.as_path()),
        "the PCB window lists another file"
    );

    let sent = dock_restore(
        &f,
        crate::panels::PanelMsg::HistoryRestoreClicked { sha: "abc".into() },
    );
    let task = f.app.update(sent);
    settle_task(&mut f.app, task);

    assert!(said(&format!(
        "{} does not belong to an open project",
        f.window_path.display()
    )));
    assert!(!said(&format!(
        "{} does not belong to an open project",
        f.main_path.display()
    )));
}

#[test]
fn a_windows_history_that_is_current_is_not_swapped_again() {
    let mut f = fixture();
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);
    // The window's state moves on; a current window is left alone, so its
    // context does not pick the change up.
    f.app
        .document_state
        .window_history
        .get_mut(&f.window)
        .unwrap()
        .entries = vec![entry("abc")];
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);
    assert!(
        f.app.document_state.window_panel_ctx[&f.window]
            .history
            .entries
            .is_empty(),
        "an unchanged window was swapped and its history cloned again"
    );

    // A changed dirty bit is picked up.
    f.app.document_state.mark_history_edit_dirty(&f.window_path);
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);
    assert!(f.app.document_state.window_history[&f.window].dirty);
}

#[test]
fn no_window_history_is_built_while_the_dock_shows_no_panel() {
    let mut f = fixture();
    for kind in crate::panels::ALL_PANELS {
        f.app.document_state.dock.close_panel(*kind);
    }
    f.app.document_state.dock.floating.clear();
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);
    assert!(f.app.document_state.window_history.is_empty());
    assert!(f.app.document_state.window_panel_ctx.is_empty());
}

#[test]
fn f3_moving_the_find_dialog_to_another_window_forgets_its_skip_report() {
    let mut f = fixture_with_a_labelled_a3_window();
    f.app.ui_state.find_replace.query = "WINDOW_NET".into();
    f.app.ui_state.find_replace.window = None;
    f.app.ui_state.find_replace.open = true;
    f.app
        .ui_state
        .find_replace
        .skips_reported
        .insert(std::path::PathBuf::from("skipped.snxsch"));

    let (_outcome, task) = command(&mut f.app, "find_next", f.window);
    settle_task(&mut f.app, task);

    assert_eq!(f.app.ui_state.find_replace.window, Some(f.window));
    assert!(f.app.ui_state.find_replace.skips_reported.is_empty());
}

#[test]
fn f3_in_main_takes_a_sheet_find_owned_by_a_window_over() {
    let mut f = fixture();
    f.app.ui_state.find_replace.query = "NO_SUCH_TEXT".into();
    f.app.ui_state.find_replace.window = Some(f.window);
    f.app.ui_state.find_replace.open = true;
    f.app.ui_state.find_replace.scope = crate::find_replace::FindScope::Sheet;
    f.app
        .ui_state
        .find_replace
        .skips_reported
        .insert(std::path::PathBuf::from("skipped.snxsch"));

    let (_outcome, task) = command(&mut f.app, "find_next", f.main);
    settle_task(&mut f.app, task);

    assert_eq!(f.app.ui_state.find_replace.window, None);
    assert!(f.app.ui_state.find_replace.skips_reported.is_empty());
}

#[test]
fn a_keystroke_for_a_palette_another_window_now_owns_is_ignored() {
    let mut f = fixture();
    let (_outcome, task) = command(&mut f.app, "open_command_palette", f.window);
    settle_task(&mut f.app, task);
    let other = iced::window::Id::unique();
    let before = f.app.ui_state.command_palette.query.clone();

    let task = f.app.update(Message::CommandPalette(
        crate::app::CommandPaletteMsg::QueryTypedInWindow(other, "late_keystroke".into()),
    ));
    settle_task(&mut f.app, task);

    assert_eq!(f.app.ui_state.command_palette.query, before);
    assert_eq!(f.app.ui_state.command_palette.window, Some(f.window));
}

#[test]
fn a_library_window_is_dirty_through_its_library_toml_not_mains_tab() {
    let mut f = fixture();
    // Main's active tab is a schematic; the window shows a library.
    f.app.document_state.tabs[1].kind = crate::app::TabKind::LibraryBrowser(f.window_path.clone());
    f.app
        .document_state
        .mark_history_edit_dirty(&f.window_path.join("library.toml"));
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);
    assert!(
        f.app.document_state.window_history[&f.window].dirty,
        "the window read main's tab for the library.toml check"
    );
}

#[test]
fn a_new_window_context_does_not_start_with_mains_history_rows() {
    let mut f = fixture();
    f.app.document_state.history.active_path = Some(f.main_path.clone());
    f.app.document_state.panel_ctx.history = f.app.document_state.history.clone();
    let ctx = f.app.new_window_panel_ctx(f.window);
    assert_eq!(ctx.history.active_path, None);
}

fn own_a_sheet_find(f: &mut Fixture) {
    f.app.ui_state.find_replace.query = "NO_SUCH_TEXT".into();
    f.app.ui_state.find_replace.window = Some(f.window);
    f.app.ui_state.find_replace.open = true;
    f.app.ui_state.find_replace.scope = crate::find_replace::FindScope::Sheet;
}

#[test]
fn find_next_from_mains_menu_takes_a_window_owned_sheet_find_over_like_f3() {
    let mut f = fixture();
    own_a_sheet_find(&mut f);
    let task = f
        .app
        .run_menu_command(&AppCommandId::new("find_next").unwrap(), None);
    settle_task(&mut f.app, task);
    assert_eq!(
        f.app.ui_state.find_replace.window, None,
        "menu Find Next stepped the window's Find"
    );
}

#[test]
fn find_next_from_the_palette_takes_a_window_owned_sheet_find_over_like_f3() {
    use crate::app::command_palette::CommandAction;
    let mut f = fixture();
    let (_outcome, task) = command(&mut f.app, "open_command_palette", f.main);
    settle_task(&mut f.app, task);
    own_a_sheet_find(&mut f);
    f.app.ui_state.command_palette.query = "find next".into();
    f.app.refresh_command_palette_results();
    let row = f
        .app
        .ui_state
        .command_palette
        .results
        .iter()
        .position(|e| matches!(&e.action, CommandAction::Command(id) if id.as_str() == "find_next"))
        .expect("the palette offers Find Next");
    f.app.ui_state.command_palette.selected_index = row;
    let task = f.app.update(Message::CommandPalette(
        crate::app::CommandPaletteMsg::ExecuteSelected,
    ));
    settle_task(&mut f.app, task);
    assert_eq!(
        f.app.ui_state.find_replace.window, None,
        "palette Find Next stepped the window's Find"
    );
}

#[test]
fn a_restore_after_a_real_history_load_for_a_window_passes_the_guard() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let (app, _boot) = Signex::new();
    let mut f = fixture_in(app, dir.path());
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);
    let generation = f.app.document_state.window_history[&f.window].generation;
    let task = f.app.update(Message::Ui(UiMsg::HistoryLoaded {
        window: Some(f.window),
        generation,
        path: f.window_path.clone(),
        result: Ok(crate::panels::history::HistoryLoad::Entries(Vec::new())),
    }));
    settle_task(&mut f.app, task);
    let sent = dock_restore(
        &f,
        crate::panels::PanelMsg::HistoryRestoreClicked { sha: "abc".into() },
    );
    let task = f.app.update(sent);
    settle_task(&mut f.app, task);
    assert!(
        !said(&format!(
            "The history of {} is not loaded",
            f.window_path.display()
        )),
        "the guard refused a click on a loaded list"
    );
    assert!(said(&format!(
        "{} does not belong to an open project",
        f.window_path.display()
    )));
}

#[test]
fn a_history_panel_opened_after_an_undock_never_lists_mains_rows() {
    let mut f = fixture();
    f.app.document_state.dock = crate::dock::DockArea::new();
    let task = f.app.finish_update();
    settle_task(&mut f.app, task);
    let task = f
        .app
        .update(Message::Overlay(crate::app::OverlayMsg::OpenPanel(
            crate::panels::PanelKind::History,
        )));
    settle_task(&mut f.app, task);
    assert_eq!(
        f.app.panel_ctx_for(f.window).history.active_path.as_deref(),
        Some(f.window_path.as_path()),
        "the window's History panel does not list the window's file"
    );
}
