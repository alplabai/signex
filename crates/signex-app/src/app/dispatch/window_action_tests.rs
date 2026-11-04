//! #1910 / #1966: an action taken in an undocked window acts on the document
//! that window shows, and the surfaces it opens paint in that window only.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::state::WindowKind;
use crate::app::{ContextMenuMsg, TabInfo, TabKind};
use crate::layout_probe::{lay_out_texts, messages_on_click, messages_on_click_at};
use signex_types::schematic::{Point, SelectedItem, SelectedKind, Wire};

const SIZE: iced::Size = iced::Size::new(1400.0, 900.0);

fn tab(path: &Path) -> TabInfo {
    TabInfo {
        title: path.file_stem().unwrap().to_string_lossy().into_owned(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    }
}

fn wire_id() -> uuid::Uuid {
    uuid::Uuid::from_u128(7)
}

fn sheet_with_a_wire() -> signex_engine::Engine {
    let mut engine =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    engine
        .execute(signex_engine::Command::PlaceWireSegment {
            wire: Wire {
                uuid: wire_id(),
                start: Point::new(10.16, 10.16),
                end: Point::new(20.32, 10.16),
                ..Default::default()
            },
        })
        .unwrap();
    engine
}

struct Fixture {
    app: Signex,
    main: iced::window::Id,
    window: iced::window::Id,
    main_path: PathBuf,
    window_path: PathBuf,
}

/// The main window shows an empty `a.snxsch`; an undocked window shows
/// `b.snxsch`, which holds one wire.
fn fixture() -> Fixture {
    let (app, _boot) = Signex::new();
    fixture_in(app, Path::new(""))
}

/// [`fixture`] on `app`, with the two sheets in `dir`.
fn fixture_in(app: Signex, dir: &Path) -> Fixture {
    fixture_in_dirs(app, dir, dir)
}

/// [`fixture`] on `app`, with the main sheet in `main_dir` and the window's
/// in `window_dir`.
fn fixture_in_dirs(mut app: Signex, main_dir: &Path, window_dir: &Path) -> Fixture {
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    let main_path = main_dir.join("a.snxsch");
    let window_path = window_dir.join("b.snxsch");
    let empty =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    app.document_state.engines.insert(main_path.clone(), empty);
    app.document_state
        .engines
        .insert(window_path.clone(), sheet_with_a_wire());
    app.document_state.tabs.push(tab(&main_path));
    app.document_state.tabs.push(tab(&window_path));
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(main_path.clone());
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: window_path.clone(),
            title: "b".to_string(),
        },
    );
    let mut slot = crate::canvas::CanvasSlot::new();
    slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet(
            app.document_state.engines[&window_path].document(),
        ),
    ));
    app.interaction_state.canvases.insert(window, slot);
    Fixture {
        app,
        main,
        window,
        main_path,
        window_path,
    }
}

fn texts(app: &Signex, window: iced::window::Id) -> Vec<String> {
    lay_out_texts(app.view(window), SIZE)
        .into_iter()
        .map(|t| t.text)
        .collect()
}

fn has(app: &Signex, window: iced::window::Id, label: &str) -> bool {
    texts(app, window).iter().any(|t| t == label)
}

/// Feed `messages` through `update`, as the runtime does.
fn feed(app: &mut Signex, messages: Vec<Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

fn is_scoped(message: &Message, window: iced::window::Id) -> bool {
    matches!(
        message,
        Message::Window(WindowMsg::ActionTakenInWindow { window: w, .. }) if *w == window
    )
}

#[test]
fn only_a_document_window_scopes_what_it_sends() {
    let f = fixture();
    let menu = || Message::ContextMenu(ContextMenuMsg::Close);
    assert!(is_scoped(
        &f.app.scope_to_window(f.window, menu()),
        f.window
    ));
    assert!(
        matches!(
            f.app.scope_to_window(f.main, menu()),
            Message::ContextMenu(_)
        ),
        "the main window sends its messages as they are"
    );
    let gone = iced::window::Id::unique();
    assert!(
        matches!(f.app.scope_to_window(gone, menu()), Message::ContextMenu(_)),
        "a window that is not a document window sends its messages as they are"
    );
}

#[test]
fn a_context_menu_opened_in_an_undocked_window_paints_there_only() {
    let mut f = fixture();
    let open = f.app.scope_to_window(
        f.window,
        Message::ContextMenu(ContextMenuMsg::Show(300.0, 300.0)),
    );
    let _ = f.app.update(open);

    assert_eq!(f.app.interaction_state.menu_window, Some(f.window));
    assert!(has(&f.app, f.window, "Find Similar Objects..."));
    assert!(
        !has(&f.app, f.main, "Find Similar Objects..."),
        "the menu also painted in the main window"
    );
}

#[test]
fn a_context_menu_opened_in_the_main_window_stays_out_of_the_undocked_one() {
    let mut f = fixture();
    f.app.interaction_state.menu_window = Some(f.window);
    let _ = f
        .app
        .update(Message::ContextMenu(ContextMenuMsg::Show(300.0, 300.0)));

    assert_eq!(f.app.interaction_state.menu_window, None);
    assert!(has(&f.app, f.main, "Find Similar Objects..."));
    assert!(!has(&f.app, f.window, "Find Similar Objects..."));
}

#[test]
fn a_context_menu_row_in_an_undocked_window_runs_against_that_window() {
    let mut f = fixture();
    let open = f.app.scope_to_window(
        f.window,
        Message::ContextMenu(ContextMenuMsg::Show(300.0, 300.0)),
    );
    let _ = f.app.update(open);

    let clicked = messages_on_click(f.app.view(f.window), SIZE, "Find");
    assert!(
        !clicked.is_empty() && clicked.iter().all(|m| is_scoped(m, f.window)),
        "the row's click was not tagged with its window: {clicked:?}"
    );
    feed(&mut f.app, clicked);

    let find = &f.app.ui_state.find_replace;
    assert!(find.open, "Find did not open");
    assert_eq!(
        find.window,
        Some(f.window),
        "Find opened for another window"
    );
    assert!(has(&f.app, f.window, "Scope"));
    assert!(
        !has(&f.app, f.main, "Scope"),
        "Find painted in the main window"
    );
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

#[test]
fn the_active_bar_of_an_undocked_window_sends_window_tagged_messages() {
    let f = fixture();
    let centre = SIZE.width / 2.0;
    let mut sent = Vec::new();
    // The bar is centred under the chrome; sweep across it.
    for step in -30..=30 {
        let point = iced::Point::new(centre + step as f32 * 6.0, 76.0);
        sent.extend(messages_on_click_at(
            f.app.view(f.window),
            SIZE,
            None,
            point,
        ));
    }
    let bar: Vec<_> = sent
        .iter()
        .filter(|m| match m {
            Message::ActiveBar(_) => true,
            Message::Window(WindowMsg::ActionTakenInWindow { message, .. }) => {
                matches!(**message, Message::ActiveBar(_))
            }
            _ => false,
        })
        .collect();
    assert!(!bar.is_empty(), "no Active Bar button answered: {sent:?}");
    assert!(
        bar.iter().all(|m| is_scoped(m, f.window)),
        "an Active Bar click acted on the main window: {bar:?}"
    );
}

#[test]
fn select_all_from_an_undocked_active_bar_selects_in_that_window_sheet() {
    let mut f = fixture();
    let select_all = f.app.scope_to_window(
        f.window,
        Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
            crate::active_bar::ActiveBarAction::SelectAll,
        )),
    );
    let _ = f.app.update(select_all);

    let in_window = &f.app.interaction_state.canvas_for_window(f.window).selected;
    assert_eq!(
        in_window,
        &vec![SelectedItem::new(wire_id(), SelectedKind::Wire)],
        "the window's own wire was not selected"
    );
    assert!(f.app.interaction_state.canvas.selected.is_empty());
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

#[test]
fn an_active_bar_dropdown_opened_in_an_undocked_window_paints_there_only() {
    let mut f = fixture();
    let open = f.app.scope_to_window(
        f.window,
        Message::ActiveBar(crate::active_bar::ActiveBarMsg::ToggleMenu(
            crate::active_bar::ActiveBarMenu::Align,
        )),
    );
    let _ = f.app.update(open);

    assert_eq!(f.app.interaction_state.menu_window, Some(f.window));
    let dropdown = |app: &Signex, window| {
        lay_out_texts(app.view(window), SIZE)
            .iter()
            .filter(|t| t.text.starts_with("Align"))
            .count()
    };
    assert!(dropdown(&f.app, f.window) > 0, "no dropdown in its window");
    assert_eq!(dropdown(&f.app, f.main), 0, "dropdown in the main window");
}

#[test]
fn find_opened_in_an_undocked_window_belongs_to_it() {
    let mut f = fixture();
    let open = f
        .app
        .scope_to_window(f.window, Message::Overlay(crate::app::OverlayMsg::OpenFind));
    let _ = f.app.update(open);
    assert_eq!(f.app.ui_state.find_replace.window, Some(f.window));
    assert!(!has(&f.app, f.main, "Scope"));

    let _ = f
        .app
        .update(Message::Overlay(crate::app::OverlayMsg::OpenFind));
    assert_eq!(f.app.ui_state.find_replace.window, None);
    assert!(has(&f.app, f.main, "Scope"));
    assert!(!has(&f.app, f.window, "Scope"));
}

#[test]
fn find_similar_paints_in_the_window_that_shows_its_sheet() {
    let mut f = fixture();
    f.app.ui_state.find_similar = Some(crate::find_similar::FindSimilarState::new(
        "Wire W7".to_string(),
        Vec::new(),
        Some(f.window_path.clone()),
    ));

    assert!(has(&f.app, f.window, "Find Similar Objects"));
    assert!(!has(&f.app, f.main, "Find Similar Objects"));
}

#[test]
fn move_selection_by_xy_moves_the_sheet_of_the_window_it_was_opened_in() {
    let mut f = fixture();
    f.app
        .interaction_state
        .canvases
        .get_mut(&f.window)
        .unwrap()
        .selected = vec![SelectedItem::new(wire_id(), SelectedKind::Wire)];
    let open = f.app.scope_to_window(
        f.window,
        Message::MoveSelection(crate::app::MoveSelectionMsg::Open),
    );
    let _ = f.app.update(open);
    assert_eq!(f.app.ui_state.move_selection.window, Some(f.window));
    f.app.ui_state.move_selection.dx = "5".to_string();
    f.app.ui_state.move_selection.dy = "0".to_string();

    let _ = f
        .app
        .update(Message::MoveSelection(crate::app::MoveSelectionMsg::Apply));

    let engine = &f.app.document_state.engines[&f.window_path];
    let wire = &engine.document().wires[0];
    assert!(
        (wire.start.x - 15.16).abs() < 1e-6,
        "the window's wire did not move: {:?}",
        wire.start
    );
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

#[test]
fn the_menu_bar_and_dock_layout_of_an_undocked_window_are_never_scoped() {
    // Scoping them runs a tab switch inside the window swap and loads the
    // wrong sheet into the window's canvas, so they send plain messages.
    let f = fixture();
    let mut sent = Vec::new();
    for y in (8..SIZE.height as i32).step_by(12) {
        for x in [8, 40, 80, 140, 220, SIZE.width as i32 - 40] {
            sent.extend(messages_on_click_at(
                f.app.view(f.window),
                SIZE,
                None,
                iced::Point::new(x as f32, y as f32),
            ));
        }
    }
    let is_chrome = |m: &Message| {
        matches!(m, Message::Menu(_))
            || matches!(
                m,
                Message::Dock(
                    crate::dock::DockMessage::TabDragStart(..)
                        | crate::dock::DockMessage::TabHoverEnter(..)
                )
            )
    };
    assert!(sent.iter().any(is_chrome), "no chrome message: {sent:?}");
    assert!(
        sent.iter().all(|m| !matches!(
            m,
            Message::Window(WindowMsg::ActionTakenInWindow { message, .. }) if is_chrome(message)
        )),
        "a menu or dock layout message from an undocked window was scoped: {sent:?}"
    );
}

#[test]
fn a_follow_up_that_edits_the_document_keeps_the_window_it_came_from() {
    use futures::StreamExt as _;
    let f = fixture();
    let select_all = || {
        Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
            crate::active_bar::ActiveBarAction::SelectAll,
        ))
    };
    let task = Task::batch([
        Task::done(select_all()).map({
            let window = f.window;
            move |m| tag_followup(window, m)
        }),
        Task::done(Message::Library(
            crate::library::LibraryMessage::ComponentsPanelRowPressed(
                crate::library::state::ComponentsPanelRowKey {
                    library_path: PathBuf::from("l.snxlib"),
                    table: "t".to_string(),
                    row_id: signex_library::RowId(uuid::Uuid::nil()),
                },
            ),
        ))
        .map({
            let window = f.window;
            move |m| tag_followup(window, m)
        }),
    ]);
    let mut stream = iced_runtime::task::into_stream(task).expect("a stream");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut out = Vec::new();
    runtime.block_on(async {
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(m) = action {
                out.push(m);
            }
        }
    });
    assert_eq!(out.len(), 2);
    assert!(is_scoped(&out[0], f.window), "{:?}", out[0]);
    assert!(
        matches!(out[1], Message::Library(_)),
        "a library follow-up must stay on the main window"
    );
}

#[test]
fn closing_an_undocked_window_closes_the_surfaces_opened_in_it() {
    let mut f = fixture();
    let open = f
        .app
        .scope_to_window(f.window, Message::Overlay(crate::app::OverlayMsg::OpenFind));
    let _ = f.app.update(open);
    assert!(f.app.ui_state.find_replace.open);

    let _ = f
        .app
        .update(Message::Window(WindowMsg::SecondaryWindowClosed(f.window)));

    assert!(
        !f.app.ui_state.find_replace.open,
        "a Find nobody can see stayed open"
    );
}

#[test]
fn a_project_tree_menu_belongs_to_the_window_it_was_raised_in() {
    let mut f = fixture();
    let open = f.app.scope_to_window(
        f.window,
        Message::ContextMenu(ContextMenuMsg::ShowProjectTree(Some(vec!["p".to_string()]))),
    );
    let _ = f.app.update(open);
    assert_eq!(f.app.interaction_state.menu_window, Some(f.window));
    assert!(f.app.interaction_state.project_tree_context_menu.is_some());
}

#[test]
fn a_components_row_pressed_in_an_undocked_window_remembers_that_window() {
    let mut f = fixture();
    let key = crate::library::state::ComponentsPanelRowKey {
        library_path: PathBuf::from("l.snxlib"),
        table: "t".to_string(),
        row_id: signex_library::RowId(uuid::Uuid::nil()),
    };
    let press = f.app.scope_to_window(
        f.window,
        Message::Library(crate::library::LibraryMessage::ComponentsPanelRowPressed(
            key,
        )),
    );
    let _ = f.app.update(press);

    let drag = f.app.library.components_panel.drag.as_ref().unwrap();
    assert_eq!(drag.window, Some(f.window));
}

#[test]
fn an_action_for_a_closed_window_does_not_fall_through_to_the_main_window() {
    let mut f = fixture();
    let message = f.app.scope_to_window(
        f.window,
        Message::ContextMenu(ContextMenuMsg::Show(300.0, 300.0)),
    );
    f.app.ui_state.windows.remove(&f.window);
    f.app.interaction_state.canvases.remove(&f.window);

    let _ = f.app.update(message);

    assert!(f.app.interaction_state.context_menu.is_none());
}

fn label_id() -> uuid::Uuid {
    uuid::Uuid::from_u128(9)
}

/// `fixture` where the window's sheet is A3 and holds a selected label
/// "WINDOW_NET"; the main window's sheet is A4 with nothing selected.
fn fixture_with_a_labelled_a3_window() -> Fixture {
    let mut f = fixture();
    let engine = f
        .app
        .document_state
        .engines
        .get_mut(&f.window_path)
        .unwrap();
    engine
        .execute(signex_engine::Command::SetPaperSize {
            paper_size: "A3".into(),
        })
        .unwrap();
    engine
        .execute(signex_engine::Command::PlaceLabel {
            label: signex_types::schematic::Label {
                unknown_columns: Default::default(),
                uuid: label_id(),
                text: "WINDOW_NET".into(),
                position: Point::new(30.48, 30.48),
                rotation: 0.0,
                label_type: signex_types::schematic::LabelType::Net,
                shape: String::new(),
                font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
                font_family: None,
                justify: Default::default(),
                justify_v: Default::default(),
                locked: false,
            },
        })
        .unwrap();
    let cache = crate::schematic_runtime::SchematicRenderCache::from_sheet(
        f.app.document_state.engines[&f.window_path].document(),
    );
    let slot = f.app.interaction_state.canvases.get_mut(&f.window).unwrap();
    slot.set_render_cache(Some(cache));
    slot.selected = vec![SelectedItem::new(label_id(), SelectedKind::Label)];
    f.app.refresh_panel_ctx();
    f
}

fn dock(message: crate::panels::PanelMsg) -> Message {
    Message::Dock(crate::dock::DockMessage::Panel(message))
}

#[test]
fn properties_in_an_undocked_window_shows_that_windows_sheet() {
    let f = fixture_with_a_labelled_a3_window();

    let window = f.app.panel_ctx_for(f.window);
    assert_eq!(window.paper_size, "A3");
    assert_eq!(window.selection_count, 1);
    assert!(
        window
            .selection_info
            .iter()
            .any(|(_, value)| value == "WINDOW_NET"),
        "the selected label's text is not in the window's Properties: {:?}",
        window.selection_info
    );
    assert!(has(&f.app, f.window, "Net Label"));
    assert!(has(&f.app, f.window, "1 object selected"));

    let main = f.app.panel_ctx_for(f.main);
    assert_eq!(main.paper_size, "A4");
    assert_eq!(main.selection_count, 0);
    assert!(has(&f.app, f.main, "Width: 297mm  Height: 210mm"));
    assert!(has(&f.app, f.main, "Nothing selected"));
}

#[test]
fn a_properties_edit_in_an_undocked_window_changes_that_windows_sheet_only() {
    let mut f = fixture_with_a_labelled_a3_window();
    let edit = |message| f.app.scope_to_window(f.window, dock(message));
    let rename = edit(crate::panels::PanelMsg::EditLabelText(
        label_id(),
        "RENAMED".into(),
    ));
    let resize = edit(crate::panels::PanelMsg::SetPaperSize("A2".into()));
    assert!(is_scoped(&rename, f.window) && is_scoped(&resize, f.window));
    feed(&mut f.app, vec![rename, resize]);

    let window_sheet = f.app.document_state.engines[&f.window_path].document();
    assert_eq!(window_sheet.labels[0].text, "RENAMED");
    assert_eq!(window_sheet.paper_size, "A2");
    let main_sheet = f.app.document_state.engines[&f.main_path].document();
    assert!(main_sheet.labels.is_empty());
    assert_ne!(main_sheet.paper_size, "A2");
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));

    assert_eq!(f.app.panel_ctx_for(f.window).paper_size, "A2");
    assert_eq!(f.app.panel_ctx_for(f.main).paper_size, "A4");
    assert_eq!(f.app.document_state.panel_ctx.selection_count, 0);
}

#[test]
fn opening_a_sheet_from_an_undocked_windows_projects_panel_runs_on_the_main_window() {
    use signex_widgets::tree_view::TreeMsg;
    let dir = tempfile::tempdir().unwrap();
    let names = ["a.snxsch", "b.snxsch", "c.snxsch"];
    let app = crate::app::handlers::menu::export::tests::app_workspace(
        &dir.path().to_string_lossy(),
        &names,
    );
    let mut f = fixture_in(app, dir.path());
    let c_path = dir.path().join("c.snxsch");
    let c = signex_engine::Engine::new(Default::default()).unwrap();
    f.app.document_state.engines.insert(c_path.clone(), c);
    f.app.document_state.tabs.push(tab(&c_path));
    f.app.refresh_panel_ctx();
    let key = vec![
        f.app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
        "c.snxsch".to_string(),
    ];

    let mut sent = Vec::new();
    for _ in 0..2 {
        let message = dock(crate::panels::PanelMsg::Tree(TreeMsg::Select(key.clone())));
        let scoped = f.app.scope_to_window(f.window, message);
        assert!(
            !is_scoped(&scoped, f.window),
            "an open-sheet click was scoped"
        );
        sent.push(scoped);
    }
    feed(&mut f.app, sent);

    assert_eq!(
        f.app.document_state.active_path,
        Some(c_path),
        "the main window did not open the sheet"
    );
    let slot = f.app.interaction_state.canvas_for_window(f.window);
    assert_eq!(slot.selected, Vec::new());
    let wires_shown = slot.active_render_cache().map(|c| c.snapshot().wires.len());
    assert_eq!(
        wires_shown,
        Some(1),
        "the window's canvas was loaded with another sheet"
    );
    assert!(f.app.interaction_state.canvas.selected.is_empty());
}

fn third_sheet(f: &mut Fixture) -> PathBuf {
    let path = PathBuf::from("c.snxsch");
    let engine = signex_engine::Engine::new(Default::default()).unwrap();
    f.app.document_state.engines.insert(path.clone(), engine);
    f.app.document_state.tabs.push(tab(&path));
    path
}

fn wires_shown_in(app: &Signex, window: iced::window::Id) -> Option<usize> {
    app.interaction_state
        .canvas_for_window(window)
        .active_render_cache()
        .map(|cache| cache.snapshot().wires.len())
}

#[test]
fn stepping_sheets_from_an_undocked_window_never_loads_another_sheet_into_it() {
    for id in ["next_sheet", "previous_sheet"] {
        let mut f = fixture();
        third_sheet(&mut f);
        let command = crate::keymap::AppCommandId::new(id).unwrap();
        let (outcome, _task) = f.app.dispatch_command_reporting(
            &command,
            crate::app::command::CommandArgs::none(),
            Some(f.window),
        );
        assert!(
            matches!(outcome, crate::app::command::CommandOutcome::Ran),
            "{id} did not run: {outcome:?}"
        );
        assert_eq!(
            wires_shown_in(&f.app, f.window),
            Some(1),
            "{id}: the window's canvas shows another sheet"
        );
        let shown = &f.app.ui_state.windows[&f.window];
        assert!(matches!(shown, WindowKind::UndockedTab { path, .. } if *path == f.window_path));
    }
}

#[test]
fn history_restore_and_erc_focus_from_an_undocked_window_run_on_the_main_window() {
    let f = fixture();
    for message in [
        crate::panels::PanelMsg::HistoryRestoreClicked { sha: "abc".into() },
        crate::panels::PanelMsg::HistoryRestoreCancelled,
        crate::panels::PanelMsg::FocusErcViolation(0),
        crate::panels::PanelMsg::FocusNextErcDiagnostic,
        crate::panels::PanelMsg::FocusPrevErcDiagnostic,
        crate::panels::PanelMsg::FpLibraryOpenSibling(PathBuf::from("x.snxfpt")),
        crate::panels::PanelMsg::SchLibrarySelectSymbol(0),
    ] {
        let sent = f.app.scope_to_window(f.window, dock(message.clone()));
        assert!(
            !is_scoped(&sent, f.window),
            "{message:?} would run inside the window's swap"
        );
    }
}

#[test]
fn a_window_refresh_leaves_the_main_windows_navigator_on_the_main_project() {
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

    f.app.refresh_panel_ctx();

    let wanted = f
        .app
        .document_state
        .navigator_job
        .wanted_fingerprint()
        .unwrap_or_default();
    let root =
        |dir: &tempfile::TempDir, name: &str| format!("root {}", dir.path().join(name).display());
    assert!(
        wanted.contains(&root(&dir_a, "a.snxsch")),
        "the Navigator was retargeted: {wanted}"
    );
    assert!(!wanted.contains(&root(&dir_b, "b.snxsch")));
}
