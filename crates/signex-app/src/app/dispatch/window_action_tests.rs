//! #1966: the dock panels and the commands of an undocked window act on the
//! sheet that window shows; what steps to or opens another sheet stays on the
//! main window.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::state::WindowKind;
use crate::app::{TabInfo, TabKind, WindowMsg};
use crate::layout_probe::lay_out_texts;
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
        Message::Dock(crate::dock::DockMessage::ScopedToWindow(w, _)) if *w == window
    )
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
    let edit = |message| f.app.scope_dock_message(f.window, dock(message));
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
        let scoped = f.app.scope_dock_message(f.window, message);
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

fn said(fragment: &str) -> bool {
    crate::diagnostics::recent_entries()
        .iter()
        .any(|entry| entry.message.contains(fragment))
}

#[test]
fn only_commands_that_edit_the_shown_sheet_run_in_a_windows_swap() {
    use crate::find_replace::FindReplaceMsg;
    use crate::menu_bar::MenuMessage;
    // Find next / previous with Project scope open the hit's sheet.
    for message in [
        Message::FindReplaceMsg(FindReplaceMsg::NextRequested),
        Message::FindReplaceMsg(FindReplaceMsg::PreviousRequested),
        Message::Menu(MenuMessage::NextSheet),
        Message::Menu(MenuMessage::AscendToParentSheet),
        Message::Menu(MenuMessage::OpenProject),
        Message::Ui(crate::app::UiMsg::GridToggle),
        Message::Erc(crate::app::ErcMsg::Run),
    ] {
        assert!(
            !command_edits_shown_sheet(&message),
            "{message:?} would run inside the window's swap"
        );
    }
    for message in [
        Message::Edit(crate::app::EditMsg::Copy),
        Message::Menu(MenuMessage::ZoomIn),
        Message::Selection(crate::app::selection_request::SelectionRequest::SelectAll),
    ] {
        assert!(command_edits_shown_sheet(&message), "{message:?}");
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
        let sent = f.app.scope_dock_message(f.window, dock(message.clone()));
        assert!(
            !is_scoped(&sent, f.window),
            "{message:?} would run inside the window's swap"
        );
    }
}

#[test]
fn a_history_restore_click_in_an_undocked_window_leaves_that_windows_sheet_alone() {
    let mut f = fixture();
    let before = f.app.document_state.engines[&f.window_path].revision();
    let sent = f.app.scope_dock_message(
        f.window,
        dock(crate::panels::PanelMsg::HistoryRestoreClicked { sha: "abc".into() }),
    );
    let _ = f.app.update(sent);
    assert_eq!(
        f.app.document_state.engines[&f.window_path].revision(),
        before
    );
    assert_eq!(wires_shown_in(&f.app, f.window), Some(1));
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
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

#[test]
fn dock_layout_and_other_windows_messages_are_never_wrapped() {
    let f = fixture();
    for message in [
        Message::Dock(crate::dock::DockMessage::TabDragStart(
            crate::dock::PanelPosition::Left,
            0,
        )),
        Message::Dock(crate::dock::DockMessage::ToggleCollapse(
            crate::dock::PanelPosition::Right,
        )),
    ] {
        assert!(!is_scoped(
            &f.app.scope_dock_message(f.window, message),
            f.window
        ));
    }
    let main_edit = dock(crate::panels::PanelMsg::SetPaperSize("A2".into()));
    assert!(!is_scoped(
        &f.app.scope_dock_message(f.main, main_edit),
        f.window
    ));
}

#[test]
fn a_components_row_pressed_in_an_undocked_window_remembers_that_window() {
    let mut f = fixture();
    let key = crate::library::state::ComponentsPanelRowKey {
        library_path: PathBuf::from("l.snxlib"),
        table: "t".to_string(),
        row_id: signex_library::RowId(uuid::Uuid::nil()),
    };
    let press = f.app.scope_dock_message(
        f.window,
        Message::Dock(crate::dock::DockMessage::Library(
            crate::library::LibraryMessage::ComponentsPanelRowPressed(key),
        )),
    );
    assert!(is_scoped(&press, f.window));
    let _ = f.app.update(press);
    let drag = f.app.library.components_panel.drag.as_ref().unwrap();
    assert_eq!(drag.window, Some(f.window));
}

#[test]
fn a_panel_action_for_a_closed_window_is_dropped_and_said() {
    let mut f = fixture();
    let edit = f.app.scope_dock_message(
        f.window,
        dock(crate::panels::PanelMsg::SetPaperSize("A2".into())),
    );
    f.app.ui_state.windows.remove(&f.window);
    f.app.interaction_state.canvases.remove(&f.window);
    let _ = crate::diagnostics::init_logging();
    let _ = f.app.update(edit);
    assert!(said("for a window that has closed was dropped"));
    let main_sheet = f.app.document_state.engines[&f.main_path].document();
    assert_ne!(main_sheet.paper_size, "A2");
}

#[test]
fn an_edit_in_a_window_updates_the_main_selection_when_main_shows_the_same_sheet() {
    let mut f = fixture_with_a_labelled_a3_window();
    // The main window shows the window's sheet too, with the label selected.
    f.app.document_state.active_tab = 1;
    f.app.document_state.active_path = Some(f.window_path.clone());
    let cache = crate::schematic_runtime::SchematicRenderCache::from_sheet(
        f.app.document_state.engines[&f.window_path].document(),
    );
    f.app.interaction_state.canvas.set_render_cache(Some(cache));
    f.app.interaction_state.canvas.selected =
        vec![SelectedItem::new(label_id(), SelectedKind::Label)];
    f.app.update_selection_info();

    let rename = f.app.scope_dock_message(
        f.window,
        dock(crate::panels::PanelMsg::EditLabelText(
            label_id(),
            "RENAMED".into(),
        )),
    );
    let _ = f.app.update(rename);

    let main = &f.app.document_state.panel_ctx;
    assert!(
        main.selection_info.iter().any(|(_, v)| v == "RENAMED"),
        "main's selection still shows the old text: {:?}",
        main.selection_info
    );
}

#[test]
fn a_designator_edit_in_a_window_of_another_project_changes_its_placement_only() {
    use crate::app::handlers::menu::export::tests::current_placement::{
        PLACEMENT_B, descend_by_canvas, placed_twice_on_disk,
    };
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_B);
    // Another project becomes the main window's.
    let other_dir = tempfile::tempdir().unwrap();
    let mut other = app.document_state.projects[0].clone();
    other.id = app.document_state.mint_project_id();
    other.path = other_dir.path().join("Other.snxprj");
    other.data.name = "Other".into();
    other.data.dir = other_dir.path().to_string_lossy().into_owned();
    other.data.schematic_root = Some("other.snxsch".into());
    other.data.sheets[0].filename = "other.snxsch".into();
    let other_path = other_dir.path().join("other.snxsch");
    crate::app::runtime::sheet_tree_tests::write_sheet(other_dir.path(), "other.snxsch", &[]);
    app.document_state.projects.push(other);
    app.open_or_focus_sheet_path(other_path);
    app.refresh_panel_ctx();
    app.settle_navigator();
    // The child sheet, placed twice in the first project, in its own window.
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: child.clone(),
            title: "child".into(),
        },
    );
    let mut slot = crate::canvas::CanvasSlot::new();
    slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet(
            app.document_state.engines[&child].document(),
        ),
    ));
    app.interaction_state.canvases.insert(window, slot);
    let symbol = app.document_state.engines[&child].document().symbols[0].uuid;

    let edit = app.scope_dock_message(
        window,
        dock(crate::panels::PanelMsg::EditSymbolDesignator(
            symbol,
            "R205".into(),
        )),
    );
    assert!(is_scoped(&edit, window));
    let _ = app.update(edit);

    let sheet = app.document_state.engines[&child].document();
    let instances = &sheet.symbols[0].instances;
    assert_eq!(
        sheet.symbols[0].reference, "R1",
        "the shared reference was rewritten"
    );
    assert!(
        instances.iter().any(|i| i.reference == "R1"),
        "placement A lost its entry: {instances:?}"
    );
    assert!(instances.iter().any(|i| i.reference == "R205"));
}

#[test]
fn a_row_menu_opened_in_an_undocked_window_paints_there_only_and_its_picks_are_scoped() {
    let mut f = fixture();
    let key = crate::library::state::ComponentsPanelRowKey {
        library_path: PathBuf::from("l.snxlib"),
        table: "t".to_string(),
        row_id: signex_library::RowId(uuid::Uuid::nil()),
    };
    let right_click = f.app.scope_dock_message(
        f.window,
        Message::Dock(crate::dock::DockMessage::Library(
            crate::library::LibraryMessage::ComponentsPanelRowRightClicked(key),
        )),
    );
    let _ = f.app.update(right_click);
    assert!(has(&f.app, f.window, "Edit Component"));
    assert!(!has(&f.app, f.main, "Edit Component"));
    // Press the painted "Edit Component" entry in the window.
    let clicked =
        crate::layout_probe::messages_on_click(f.app.view(f.window), SIZE, "Edit Component");
    let picked = clicked.iter().find(|m| {
        matches!(
            m,
            Message::Dock(crate::dock::DockMessage::ScopedToWindow(w, _)) if *w == f.window
        )
    });
    assert!(
        picked.is_some(),
        "the pick was not scoped to the window: {clicked:?}"
    );
}

fn pressed_row(f: &mut Fixture) {
    let key = crate::library::state::ComponentsPanelRowKey {
        library_path: PathBuf::from("l.snxlib"),
        table: "t".to_string(),
        row_id: signex_library::RowId(uuid::Uuid::nil()),
    };
    let press = f.app.scope_dock_message(
        f.window,
        Message::Dock(crate::dock::DockMessage::Library(
            crate::library::LibraryMessage::ComponentsPanelRowPressed(key),
        )),
    );
    let _ = f.app.update(press);
    f.app
        .library
        .components_panel
        .drag
        .as_mut()
        .unwrap()
        .started = true;
}

#[test]
fn a_components_drag_released_in_another_window_is_cancelled() {
    let _ = crate::diagnostics::init_logging();
    let mut f = fixture();
    pressed_row(&mut f);
    let _ = f
        .app
        .update(Message::Ui(crate::app::UiMsg::DragEnd(f.main)));
    assert!(f.app.library.components_panel.drag.is_none());
    assert!(said("released in another window than it started in"));
}

#[test]
fn a_components_drag_released_in_its_own_window_ends_there() {
    let mut f = fixture();
    pressed_row(&mut f);
    let _ = f
        .app
        .update(Message::Ui(crate::app::UiMsg::DragEnd(f.window)));
    assert!(f.app.library.components_panel.drag.is_none());
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

#[test]
fn a_title_block_draft_typed_in_a_window_survives_its_reattach() {
    let mut f = fixture();
    f.app.refresh_panel_ctx();
    f.app.document_state.drafts.set_title_block(
        &f.window_path,
        crate::panels::TitleBlockField::Title,
        "typed in the window",
    );
    let _ = f
        .app
        .update(Message::Window(WindowMsg::ReattachTab(f.window)));
    assert!(
        !f.app
            .document_state
            .window_panel_ctx
            .contains_key(&f.window)
    );
    assert_eq!(
        f.app
            .document_state
            .drafts
            .title_block_text(&f.window_path, crate::panels::TitleBlockField::Title)
            .as_deref(),
        Some("typed in the window")
    );
}

/// Main shows `other`; `top`, which places `child` twice, is in an undocked
/// window with its first sheet symbol selected. `child` is open as a tab
/// when `open_child`.
struct Descent {
    app: Signex,
    window: iced::window::Id,
    top: PathBuf,
    child: PathBuf,
    _dir: tempfile::TempDir,
}

fn descent_fixture(open_child: bool) -> Descent {
    use crate::app::handlers::menu::export::tests::current_placement::{
        PLACEMENT_A, placed_twice_on_disk,
    };
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    if open_child {
        assert!(app.open_or_focus_sheet_path(child.clone()));
    }
    crate::app::runtime::sheet_tree_tests::write_sheet(dir.path(), "other.snxsch", &[]);
    assert!(app.open_or_focus_sheet_path(dir.path().join("other.snxsch")));
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: top.clone(),
            title: "top".into(),
        },
    );
    let mut slot = crate::canvas::CanvasSlot::new();
    slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet(
            app.document_state.engines[&top].document(),
        ),
    ));
    slot.selected = vec![SelectedItem::new(
        uuid::Uuid::from_u128(PLACEMENT_A),
        SelectedKind::ChildSheet,
    )];
    app.interaction_state.canvases.insert(window, slot);
    Descent {
        app,
        window,
        top,
        child,
        _dir: dir,
    }
}

fn window_still_shows_top(d: &Descent) {
    let shown = d
        .app
        .interaction_state
        .canvas_for_window(d.window)
        .active_render_cache()
        .map(|cache| cache.snapshot().child_sheets.len());
    assert_eq!(
        shown,
        Some(2),
        "the window's canvas no longer shows its sheet"
    );
    assert!(matches!(
        &d.app.ui_state.windows[&d.window],
        WindowKind::UndockedTab { path, .. } if *path == d.top
    ));
}

fn enter_in_window(d: &mut Descent) {
    let command = crate::keymap::AppCommandId::new("placement_accept").unwrap();
    let (_outcome, task) = d.app.dispatch_command_reporting(
        &command,
        crate::app::command::CommandArgs::none(),
        Some(d.window),
    );
    crate::app::path_ops::settle_task(&mut d.app, task);
}

#[test]
fn enter_on_a_sheet_symbol_in_a_window_descends_on_the_main_window_child_open() {
    let mut d = descent_fixture(true);
    enter_in_window(&mut d);
    window_still_shows_top(&d);
    assert_eq!(d.app.document_state.active_path, Some(d.child.clone()));
}

#[test]
fn enter_on_a_sheet_symbol_in_a_window_descends_on_the_main_window_child_not_open() {
    let mut d = descent_fixture(false);
    let command = crate::keymap::AppCommandId::new("placement_accept").unwrap();
    let (_outcome, task) = d.app.dispatch_command_reporting(
        &command,
        crate::app::command::CommandArgs::none(),
        Some(d.window),
    );
    // Nothing was read for the window: no descent waits with the window's
    // sheet as its parent.
    assert!(d.app.ui_state.pending_sheet_opens.is_empty());
    crate::app::path_ops::settle_task(&mut d.app, task);
    window_still_shows_top(&d);
    assert_eq!(d.app.document_state.active_path, Some(d.child.clone()));
    assert!(d.app.ui_state.pending_sheet_opens.is_empty());
}

#[test]
fn a_double_click_on_a_sheet_symbol_in_a_window_descends_on_the_main_window() {
    let mut d = descent_fixture(true);
    // The window's sheet was itself entered through a placement; main's
    // sheet has none, so only the window's can explain the child's.
    let parent = signex_types::schematic::InstancePath::root().child(uuid::Uuid::from_u128(500));
    d.app
        .document_state
        .tab_placements
        .record(&d.top, Some(parent.clone()));
    let task = d.app.update(Message::CanvasEventInWindow {
        window_id: d.window,
        event: crate::canvas::CanvasEvent::DoubleClicked {
            world_x: 5.0,
            world_y: 5.0,
            screen_x: 0.0,
            screen_y: 0.0,
        },
    });
    crate::app::path_ops::settle_task(&mut d.app, task);
    window_still_shows_top(&d);
    assert_eq!(d.app.document_state.active_path, Some(d.child.clone()));
    // The child is shown through the placement of the symbol that was
    // double-clicked on the window's sheet, not one derived from main's.
    let expected = parent.child(uuid::Uuid::from_u128(1));
    assert_eq!(
        d.app.document_state.tab_placements.get(&d.child),
        Some(&expected)
    );
}

#[test]
fn focus_open_sheet_refuses_inside_a_window_swap_and_says_so() {
    let _ = crate::diagnostics::init_logging();
    let mut d = descent_fixture(true);
    d.app.interaction_state.acting_window = Some(d.window);
    let before = d.app.document_state.active_path.clone();
    assert!(!d.app.focus_open_sheet(&d.child));
    assert_eq!(d.app.document_state.active_path, before);
    assert!(said("was not done from inside an undocked window"));
    d.app.interaction_state.acting_window = None;
    assert!(d.app.focus_open_sheet(&d.child));
}

#[test]
fn find_next_with_project_scope_from_a_window_opens_the_hit_on_the_main_window() {
    let mut d = descent_fixture(false);
    let hit = signex_engine::FindHit {
        kind: signex_engine::FindKind::NetLabel,
        item: SelectedItem::new(uuid::Uuid::from_u128(77), SelectedKind::Label),
        target: signex_engine::TextTarget::Label(uuid::Uuid::from_u128(77)),
        text: "CHILD_NET".into(),
        placement: None,
        single: true,
        position: Point::new(1.0, 1.0),
    };
    let find = &mut d.app.ui_state.find_replace;
    find.scope = crate::find_replace::FindScope::Project;
    find.matches = vec![crate::find_replace::FindMatch {
        sheet: Some(d.child.clone()),
        hit,
        placement_label: None,
    }];
    let command = crate::keymap::AppCommandId::new("find_next").unwrap();
    let (_outcome, task) = d.app.dispatch_command_reporting(
        &command,
        crate::app::command::CommandArgs::none(),
        Some(d.window),
    );
    crate::app::path_ops::settle_task(&mut d.app, task);
    window_still_shows_top(&d);
    assert_eq!(d.app.document_state.active_path, Some(d.child.clone()));
}

fn press_in_window(f: &mut Fixture, id: &str) {
    let _ = f.app.update(Message::Ui(crate::app::UiMsg::MenuInWindow {
        window: f.window,
        msg: crate::menu_bar::MenuMessage::command(id),
    }));
}

fn select_wire_in(slot: &mut crate::canvas::CanvasSlot) {
    slot.selected = vec![SelectedItem::new(wire_id(), SelectedKind::Wire)];
}

#[test]
fn delete_typed_in_a_window_deletes_its_selection_when_main_has_none() {
    let mut f = fixture();
    select_wire_in(f.app.interaction_state.canvases.get_mut(&f.window).unwrap());
    assert!(f.app.interaction_state.canvas.selected.is_empty());
    press_in_window(&mut f, "delete_selection");
    assert!(
        f.app.document_state.engines[&f.window_path]
            .document()
            .wires
            .is_empty()
    );
}

#[test]
fn delete_typed_in_a_window_with_nothing_selected_there_leaves_main_alone() {
    let mut f = fixture();
    // Main shows a sheet with a wire that is selected; the window selects nothing.
    let main_engine = f.app.document_state.engines.get_mut(&f.main_path).unwrap();
    main_engine
        .execute(signex_engine::Command::PlaceWireSegment {
            wire: Wire {
                uuid: wire_id(),
                start: Point::new(10.16, 10.16),
                end: Point::new(20.32, 10.16),
                ..Default::default()
            },
        })
        .unwrap();
    select_wire_in(&mut f.app.interaction_state.canvas);
    press_in_window(&mut f, "delete_selection");
    assert_eq!(
        f.app.document_state.engines[&f.main_path]
            .document()
            .wires
            .len(),
        1,
        "main's selected wire was deleted by a command typed in the window"
    );
    assert_eq!(
        f.app.document_state.engines[&f.window_path]
            .document()
            .wires
            .len(),
        1
    );
}

#[test]
fn arranging_drawings_and_the_other_sheet_edits_from_a_window_run_in_its_swap() {
    use crate::app::{EditMsg, InputMsg, OverlayMsg};
    use crate::library::{LibraryMessage, PlacedPartAction, PlacedPartMsg};
    for arrange in [
        signex_engine::DrawingArrange::ToFront,
        signex_engine::DrawingArrange::ToBack,
        signex_engine::DrawingArrange::Forward,
        signex_engine::DrawingArrange::Backward,
    ] {
        assert!(command_edits_shown_sheet(&Message::Edit(
            EditMsg::ArrangeDrawings(arrange)
        )));
    }
    for message in [
        Message::Library(LibraryMessage::PlacedPart(PlacedPartMsg::ActionRequested(
            PlacedPartAction::EditSymbol,
        ))),
        Message::Overlay(OverlayMsg::ToggleAutoFocus),
        Message::Input(InputMsg::EscapePressed { window: None }),
    ] {
        assert!(command_edits_shown_sheet(&message), "{message:?}");
    }
}

#[test]
fn bring_to_front_typed_in_a_window_reorders_the_windows_drawings_not_mains() {
    let mut f = fixture();
    let rect = |n: u128| signex_types::schematic::SchDrawing::Line {
        uuid: uuid::Uuid::from_u128(n),
        start: Point::new(1.0, 1.0),
        end: Point::new(5.0, 5.0),
        width: 0.1,
        stroke_color: None,
        locked: false,
    };
    let first = rect(101);
    let second = rect(102);
    let order_of = |app: &Signex, path: &PathBuf| -> Vec<uuid::Uuid> {
        app.document_state.engines[path]
            .document()
            .drawings
            .iter()
            .map(|d| d.uuid())
            .collect()
    };
    let engine = f
        .app
        .document_state
        .engines
        .get_mut(&f.window_path)
        .unwrap();
    for drawing in [first, second] {
        engine
            .execute(signex_engine::Command::PlaceSchDrawing { drawing })
            .unwrap();
    }
    f.app
        .interaction_state
        .canvases
        .get_mut(&f.window)
        .unwrap()
        .selected = vec![SelectedItem::new(
        uuid::Uuid::from_u128(101),
        SelectedKind::Drawing,
    )];
    press_in_window(&mut f, "bring_drawings_to_front");
    assert_eq!(
        order_of(&f.app, &f.window_path),
        vec![uuid::Uuid::from_u128(102), uuid::Uuid::from_u128(101)]
    );
}

fn child_sheet(id: uuid::Uuid) -> signex_types::schematic::ChildSheet {
    signex_types::schematic::ChildSheet {
        uuid: id,
        name: "Old".into(),
        filename: "child.snxsch".into(),
        position: Point::new(40.64, 40.64),
        size: (20.0, 10.0),
        stroke_width: 0.0,
        fill: Default::default(),
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn typed_in_window(f: &Fixture, message: crate::panels::PanelMsg) -> Message {
    f.app.scope_dock_message(f.window, dock(message))
}

#[test]
fn a_child_sheet_name_typed_in_a_window_lands_in_that_windows_properties() {
    let mut f = fixture();
    f.app.refresh_panel_ctx();
    assert!(
        f.app
            .document_state
            .window_panel_ctx
            .contains_key(&f.window)
    );
    let id = uuid::Uuid::from_u128(11);
    let typed = typed_in_window(
        &f,
        crate::panels::PanelMsg::ChildSheetNameTyping(id, "Power".into()),
    );
    assert!(is_scoped(&typed, f.window));
    let _ = f.app.update(typed);
    assert_eq!(
        f.app.document_state.window_panel_ctx[&f.window].child_sheet_name_buf,
        Some((id, "Power".to_string()))
    );
    assert!(
        f.app
            .document_state
            .panel_ctx
            .child_sheet_name_buf
            .is_none()
    );
}

#[test]
fn a_child_sheet_name_committed_in_a_window_lands_in_that_windows_engine() {
    let mut f = fixture();
    let id = uuid::Uuid::from_u128(11);
    f.app
        .document_state
        .engines
        .get_mut(&f.window_path)
        .unwrap()
        .execute(signex_engine::Command::PlaceChildSheet {
            child_sheet: child_sheet(id),
        })
        .unwrap();
    f.app.refresh_panel_ctx();
    for message in [
        crate::panels::PanelMsg::ChildSheetNameTyping(id, "Power".into()),
        crate::panels::PanelMsg::CommitChildSheetName(id),
    ] {
        let scoped = typed_in_window(&f, message);
        let _ = f.app.update(scoped);
    }
    let window_sheet = f.app.document_state.engines[&f.window_path].document();
    assert_eq!(window_sheet.child_sheets[0].name, "Power");
    let main_sheet = f.app.document_state.engines[&f.main_path].document();
    assert!(main_sheet.child_sheets.is_empty());
}

#[test]
fn every_properties_message_that_writes_the_shown_sheet_or_its_context_is_scoped() {
    use crate::library::PlacedPartAction;
    use crate::panels::PanelMsg as P;
    use signex_widgets::tree_view::TreeMsg;
    let f = fixture();
    let id = uuid::Uuid::from_u128(11);
    let colour = iced::Color::from_rgb(0.1, 0.2, 0.3);
    let scoped = [
        P::ChildSheetNameTyping(id, String::new()),
        P::ChildSheetFileTyping(id, String::new()),
        P::CommitChildSheetName(id),
        P::CommitChildSheetFile(id),
        P::ToggleChildSheetBorderPicker(id),
        P::ToggleChildSheetFillPicker(id),
        P::OpenChildSheetAdvancedPicker(id, true),
        P::CancelChildSheetColorPicker,
        P::EditChildSheetBorderColor(id, colour),
        P::EditChildSheetFillColor(id, colour),
        P::ChildSheetStrokeWidthTyping(id, String::new()),
        P::CommitChildSheetStrokeWidth(id),
        P::ResetChildSheetStyle(id),
        P::EditSymbolLibId(id, String::new()),
        P::UpdateSymbolFromLibrary(id),
        P::PlacedPartAction(PlacedPartAction::EditSymbol),
        P::SetPrePlacementText(String::new()),
        P::SetPrePlacementDesignator(String::new()),
        P::SetPrePlacementRotation(0.0),
        P::SetPrePlacementFont(String::new()),
        P::SetPrePlacementFontSize(10),
        P::TogglePrePlacementBold,
        P::TogglePrePlacementItalic,
        P::TogglePrePlacementUnderline,
        P::ConfirmPrePlacement,
        P::Navigator(TreeMsg::Toggle(vec!["x".into()])),
    ];
    for message in scoped {
        let wrapped = typed_in_window(&f, message.clone());
        assert!(is_scoped(&wrapped, f.window), "{message:?} runs on main");
    }
    // A Navigator row click opens a sheet: it stays on the main window.
    let click = typed_in_window(&f, P::Navigator(TreeMsg::Select(vec!["x".into()])));
    assert!(!is_scoped(&click, f.window));
}

#[test]
fn the_panel_inputs_stamp_moves_with_every_net_class_field() {
    let dir = tempfile::tempdir().unwrap();
    let names = ["a.snxsch", "b.snxsch"];
    let app = crate::app::handlers::menu::export::tests::app_workspace(
        &dir.path().to_string_lossy(),
        &names,
    );
    let mut f = fixture_in(app, dir.path());
    f.app.document_state.projects[0]
        .data
        .net_classes
        .push(signex_types::net_class::NetClass::named("Power"));
    let mut seen = vec![f.app.window_panel_inputs_stamp()];
    let edits: [fn(&mut signex_types::net_class::NetClass); 5] = [
        |c| c.name = "Power2".into(),
        |c| c.nets.push("VCC".into()),
        |c| c.patterns.push("V*".into()),
        |c| c.clearance = 0.2,
        |c| c.style.wire_width_mm = Some(0.3),
    ];
    for edit in edits {
        edit(&mut f.app.document_state.projects[0].data.net_classes[0]);
        let stamp = f.app.window_panel_inputs_stamp();
        assert!(
            !seen.contains(&stamp),
            "a net-class edit left the stamp as it was"
        );
        seen.push(stamp);
    }
}

#[test]
fn an_in_place_edit_opened_from_a_window_commits_on_the_windows_sheet() {
    let mut f = fixture_with_a_labelled_a3_window();
    let command = crate::keymap::AppCommandId::new("edit_text_in_place").unwrap();
    let (_outcome, task) = f.app.dispatch_command_reporting(
        &command,
        crate::app::command::CommandArgs::none(),
        Some(f.window),
    );
    drop(task);
    let open = f.app.interaction_state.editing_text.as_ref();
    assert_eq!(
        open.and_then(|s| s.sheet.clone()),
        Some(f.window_path.clone())
    );
    let _ = f
        .app
        .update(Message::TextEdit(crate::app::TextEditMsg::Changed(
            "RETYPED".into(),
        )));
    let _ = f
        .app
        .update(Message::TextEdit(crate::app::TextEditMsg::Submit));
    let sheet = f.app.document_state.engines[&f.window_path].document();
    assert_eq!(sheet.labels[0].text, "RETYPED");
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

#[test]
fn a_descent_from_a_window_reaches_an_open_child_that_is_not_on_disk() {
    let mut d = descent_fixture(true);
    std::fs::remove_file(&d.child).unwrap();
    enter_in_window(&mut d);
    assert_eq!(d.app.document_state.active_path, Some(d.child.clone()));
}

#[path = "window_action_context_tests.rs"]
mod context;
