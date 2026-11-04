//! #1966: the dock panels and the commands of an undocked window act on the
//! sheet that window shows; what steps to or opens another sheet stays on the
//! main window.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::state::WindowKind;
use crate::app::{TabInfo, TabKind};
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
    let _ = f.app.update(edit);
    let main_sheet = f.app.document_state.engines[&f.main_path].document();
    assert_ne!(main_sheet.paper_size, "A2");
}
