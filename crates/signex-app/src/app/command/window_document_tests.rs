//! #1787 / #1794: document commands typed in another window act on the
//! document that window shows, never on the main window's.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::command::CommandArgs;
use crate::app::path_ops::settle_task;
use crate::app::{TabInfo, TabKind};
use iced::Task;

const OLD: &[u8] = b"OLD";

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).unwrap()
}

fn tab(path: &Path) -> TabInfo {
    TabInfo {
        title: path.file_stem().unwrap().to_string_lossy().into_owned(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    }
}

/// A dirty engine for `path`, whose file already holds [`OLD`].
fn add_dirty_sheet(app: &mut Signex, path: &Path) {
    std::fs::write(path, OLD).unwrap();
    let mut engine =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    let document = engine.document().clone();
    engine
        .execute(signex_engine::Command::ReplaceDocument { document })
        .unwrap();
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(tab(path));
    app.document_state.mark_document_dirty(path);
}

struct Fixture {
    app: Signex,
    /// Shown by the main window.
    main_path: PathBuf,
    /// Shown by the undocked window.
    undocked_path: PathBuf,
    window: iced::window::Id,
    _dir: tempfile::TempDir,
}

/// Main window on `a.snxsch`; an undocked window on `b.snxsch`; both dirty.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _boot) = Signex::new();
    let main_path = dir.path().join("a.snxsch");
    let undocked_path = dir.path().join("b.snxsch");
    add_dirty_sheet(&mut app, &main_path);
    add_dirty_sheet(&mut app, &undocked_path);
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(main_path.clone());
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: undocked_path.clone(),
            title: "b".to_string(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    Fixture {
        app,
        main_path,
        undocked_path,
        window,
        _dir: dir,
    }
}

fn press(f: &mut Fixture, id: &str, window: Option<iced::window::Id>) {
    let task = f
        .app
        .dispatch_command(&command(id), CommandArgs::none(), window);
    settle_task(&mut f.app, task);
}

/// Whether running `task` asks the runtime to close `window`. Output
/// messages are fed back through `update` as well, so a chained close counts.
fn closes_window(app: &mut Signex, task: Task<Message>, window: iced::window::Id) -> bool {
    use futures::StreamExt as _;
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut pending = vec![task];
    while let Some(task) = pending.pop() {
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            continue;
        };
        let mut next = Vec::new();
        let mut closed = false;
        runtime.block_on(async {
            while let Some(action) = stream.next().await {
                match action {
                    iced_runtime::Action::Window(iced_runtime::window::Action::Close(id))
                        if id == window =>
                    {
                        closed = true;
                    }
                    iced_runtime::Action::Output(message) => next.push(message),
                    _ => {}
                }
            }
        });
        if closed {
            return true;
        }
        pending.extend(next.into_iter().map(|m| app.update(m)));
    }
    false
}

fn active_title(app: &Signex) -> String {
    app.document_state.tabs[app.document_state.active_tab]
        .title
        .clone()
}

#[test]
fn save_in_an_undocked_window_writes_that_document_only() {
    let mut f = fixture();
    let window = f.window;
    press(&mut f, "save_document", Some(window));
    assert_ne!(std::fs::read(&f.undocked_path).unwrap(), OLD, "b was saved");
    assert_eq!(std::fs::read(&f.main_path).unwrap(), OLD, "a untouched");
    assert!(!f.app.document_state.is_document_dirty(&f.undocked_path));
    assert!(f.app.document_state.is_document_dirty(&f.main_path));
    assert_eq!(active_title(&f.app), "a", "the main window kept its tab");
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

#[test]
fn save_in_the_main_window_still_writes_the_main_document() {
    let mut f = fixture();
    press(&mut f, "save_document", None);
    assert_ne!(std::fs::read(&f.main_path).unwrap(), OLD);
    assert_eq!(std::fs::read(&f.undocked_path).unwrap(), OLD);
}

#[test]
fn save_in_a_window_with_no_tab_writes_nothing() {
    let mut f = fixture();
    let orphan = iced::window::Id::unique();
    f.app.ui_state.windows.insert(
        orphan,
        WindowKind::UndockedTab {
            path: f.main_path.with_file_name("gone.snxsch"),
            title: "gone".to_string(),
        },
    );
    press(&mut f, "save_document", Some(orphan));
    assert_eq!(std::fs::read(&f.main_path).unwrap(), OLD);
    assert_eq!(std::fs::read(&f.undocked_path).unwrap(), OLD);
}

#[test]
fn save_in_a_component_preview_writes_nothing_and_in_a_panel_saves_the_main_document() {
    let mut f = fixture();
    let preview = component_preview(&mut f.app);
    press(&mut f, "save_document", Some(preview));
    assert_eq!(std::fs::read(&f.main_path).unwrap(), OLD);
    assert_eq!(std::fs::read(&f.undocked_path).unwrap(), OLD);

    let panel = iced::window::Id::unique();
    f.app.ui_state.windows.insert(
        panel,
        WindowKind::DetachedPanel(crate::panels::PanelKind::Projects),
    );
    press(&mut f, "save_document", Some(panel));
    assert_ne!(std::fs::read(&f.main_path).unwrap(), OLD, "main saved");
    assert_eq!(std::fs::read(&f.undocked_path).unwrap(), OLD);
}

/// A detached panel edits the main document, so the tab stepper and Print
/// keep acting on the main window.
#[test]
fn tab_stepping_and_print_in_a_detached_panel_act_on_the_main_window() {
    let mut f = fixture();
    let panel = iced::window::Id::unique();
    f.app.ui_state.windows.insert(
        panel,
        WindowKind::DetachedPanel(crate::panels::PanelKind::Projects),
    );
    press(&mut f, "print", Some(panel));
    assert_eq!(
        f.app.document_state.preview.as_ref().unwrap().source,
        f.main_path
    );
    press(&mut f, "next_document_tab", Some(panel));
    assert_eq!(active_title(&f.app), "b");
}

/// The dialog is asynchronous, so its completion arrives while the main
/// window shows another document.
#[test]
fn save_as_from_an_undocked_window_writes_that_document_to_the_target() {
    let mut f = fixture();
    let target = f.main_path.with_file_name("copy.snxsch");
    let message = Message::File(crate::app::FileMsg::SaveAs {
        from_path: f.undocked_path.clone(),
        to_path: target.clone(),
    });
    crate::app::path_ops::settle(&mut f.app, message);
    assert!(target.exists(), "b was written to the chosen file");
    assert_eq!(std::fs::read(&f.main_path).unwrap(), OLD, "a untouched");
    assert_eq!(
        std::fs::read(&f.undocked_path).unwrap(),
        OLD,
        "b's old file kept"
    );
    assert_eq!(
        f.app.document_state.tabs[1].path, target,
        "b's tab re-keyed"
    );
    assert!(
        matches!(
            f.app.ui_state.windows.get(&f.window),
            Some(WindowKind::UndockedTab { path, .. }) if path == &target
        ),
        "the window follows its document"
    );
    assert_eq!(active_title(&f.app), "a");
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

#[test]
fn save_as_from_an_undocked_window_never_overwrites_the_main_document() {
    let mut f = fixture();
    let message = Message::File(crate::app::FileMsg::SaveAs {
        from_path: f.undocked_path.clone(),
        to_path: f.main_path.clone(),
    });
    crate::app::path_ops::settle(&mut f.app, message);
    assert_eq!(
        std::fs::read(&f.main_path).unwrap(),
        OLD,
        "a is open: refused"
    );
}

#[test]
fn print_in_an_undocked_window_previews_that_document() {
    let mut f = fixture();
    let window = f.window;
    press(&mut f, "print", Some(window));
    let preview = f
        .app
        .document_state
        .preview
        .as_ref()
        .expect("preview opened");
    assert_eq!(preview.source, f.undocked_path);
    assert_eq!(active_title(&f.app), "a");
}

#[test]
fn tab_stepping_in_an_undocked_window_does_nothing() {
    let mut f = fixture();
    let window = f.window;
    for id in ["next_document_tab", "previous_document_tab"] {
        press(&mut f, id, Some(window));
        assert_eq!(active_title(&f.app), "a", "{id} left the main tab strip");
    }
    press(&mut f, "next_document_tab", None);
    assert_eq!(active_title(&f.app), "b", "the main window still steps");
}

#[test]
fn the_undocked_windows_menu_rows_run_against_its_own_document() {
    let mut f = fixture();
    let window = f.window;
    let task = f.app.update(Message::Ui(crate::app::UiMsg::MenuInWindow {
        window,
        msg: crate::menu_bar::MenuMessage::Save,
    }));
    settle_task(&mut f.app, task);
    assert_ne!(std::fs::read(&f.undocked_path).unwrap(), OLD);
    assert_eq!(std::fs::read(&f.main_path).unwrap(), OLD);
}

/// Export ▸ PDF and File ▸ Print open the same modal, so in an undocked
/// window both preview that window's own document (#1813).
#[test]
fn export_pdf_in_an_undocked_window_previews_that_document() {
    let mut f = fixture();
    let window = f.window;
    assert_eq!(
        crate::menu_bar::MenuMessage::ExportPdf.window_command(),
        Some("print")
    );
    let task = f.app.update(Message::Ui(crate::app::UiMsg::MenuInWindow {
        window,
        msg: crate::menu_bar::MenuMessage::ExportPdf,
    }));
    settle_task(&mut f.app, task);
    let preview = f
        .app
        .document_state
        .preview
        .as_ref()
        .expect("preview opened");
    assert_eq!(preview.source, f.undocked_path);
}

fn row_enabled(f: &Fixture, window: Option<iced::window::Id>, id: &str) -> bool {
    crate::app::command::is_enabled(&command(id), &f.app.command_context(window))
}

/// The main window shows a PCB and no sheet; the undocked window a sheet.
fn main_shows_a_pcb(f: &mut Fixture) {
    f.app.document_state.engines.remove(&f.main_path);
    f.app.document_state.active_path = None;
    f.app.document_state.tabs[0].kind = TabKind::Pcb;
}

/// The undocked window shows a PCB; the main window keeps its sheet.
fn undocked_shows_a_pcb(f: &mut Fixture) {
    f.app.document_state.engines.remove(&f.undocked_path);
    f.app.document_state.tabs[1].kind = TabKind::Pcb;
}

/// #1813: Save, Save As and Print in an undocked sheet's menu enable by
/// that sheet, whatever the main window shows.
#[test]
fn an_undocked_sheets_file_rows_enable_with_a_pcb_in_the_main_window() {
    let mut f = fixture();
    main_shows_a_pcb(&mut f);
    for id in ["save_document", "save_document_as", "print"] {
        assert!(row_enabled(&f, Some(f.window), id), "{id} in the window");
        assert!(!row_enabled(&f, None, id), "{id} in the main window");
    }
}

/// #1813: an undocked PCB greys the rows that need a sheet, so a click
/// cannot be enabled and then refused by the retargeted window.
#[test]
fn an_undocked_pcbs_sheet_rows_grey_with_a_sheet_in_the_main_window() {
    let mut f = fixture();
    undocked_shows_a_pcb(&mut f);
    for id in ["save_document", "save_document_as", "print"] {
        assert!(!row_enabled(&f, Some(f.window), id), "{id} in the window");
        assert!(row_enabled(&f, None, id), "{id} in the main window");
    }
}

/// The main window keeps reading its own active tab.
#[test]
fn the_main_windows_file_rows_still_follow_its_active_tab() {
    let mut f = fixture();
    for id in ["save_document", "save_document_as", "print"] {
        assert!(row_enabled(&f, None, id), "{id}");
    }
    main_shows_a_pcb(&mut f);
    assert!(!row_enabled(&f, None, "print"));
    assert!(!row_enabled(&f, None, "save_document_as"));
}

/// #1787: Ctrl+F4 in an undocked window closes that window's document and
/// leaves the main window's active tab open.
#[test]
fn close_document_in_an_undocked_window_closes_its_own_tab() {
    let mut f = fixture();
    let window = f.window;
    let task = f.app.dispatch_command(
        &command("close_active_document"),
        CommandArgs::none(),
        Some(window),
    );
    let titles: Vec<_> = f
        .app
        .document_state
        .tabs
        .iter()
        .map(|t| t.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["a"],
        "the undocked window's tab closed, main's stayed"
    );
    assert_eq!(f.app.document_state.active_tab, 0);
    assert!(closes_window(&mut f.app, task, window), "the window closed");
}

#[test]
fn close_document_in_the_main_window_closes_the_active_tab() {
    let mut f = fixture();
    press(&mut f, "close_active_document", None);
    let titles: Vec<_> = f
        .app
        .document_state
        .tabs
        .iter()
        .map(|t| t.title.as_str())
        .collect();
    assert_eq!(titles, ["b"]);
}

fn component_preview(app: &mut Signex) -> iced::window::Id {
    let library_path = PathBuf::from("parts.snxlib");
    let row_id = signex_library::RowId::new();
    app.document_state.tabs.push(TabInfo {
        title: "R1".to_string(),
        path: PathBuf::from("parts.snxlib/Resistors.tsv#1"),
        cached_document: None,
        project_id: None,
        kind: TabKind::ComponentEditor(crate::app::ComponentEditorTab {
            library_path: library_path.clone(),
            table: "Resistors".to_string(),
            row_id,
        }),
    });
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::ComponentEditor {
            library_path,
            table: "Resistors".to_string(),
            row_id,
        },
    );
    window
}

/// Ctrl+F4 in a Component Preview closes its `ComponentEditor` tab and the
/// window, not just re-docks the preview.
#[test]
fn close_document_in_a_component_preview_closes_its_tab_and_window() {
    let mut f = fixture();
    let window = component_preview(&mut f.app);
    let task = f.app.dispatch_command(
        &command("close_active_document"),
        CommandArgs::none(),
        Some(window),
    );
    assert!(
        f.app
            .document_state
            .tabs
            .iter()
            .all(|t| !t.kind.is_component_editor()),
        "the Component Preview tab closed"
    );
    assert_eq!(f.app.document_state.tabs.len(), 2, "the sheets stayed open");
    assert!(closes_window(&mut f.app, task, window), "the window closed");
}

/// A Component Preview whose tab is gone is closed as the OS close button
/// does; the main window's tabs are untouched.
#[test]
fn close_document_in_a_component_preview_without_a_tab_closes_the_window() {
    let mut f = fixture();
    let window = iced::window::Id::unique();
    f.app.ui_state.windows.insert(
        window,
        WindowKind::ComponentEditor {
            library_path: PathBuf::from("parts.snxlib"),
            table: "Resistors".to_string(),
            row_id: signex_library::RowId::new(),
        },
    );
    let task = f.app.dispatch_command(
        &command("close_active_document"),
        CommandArgs::none(),
        Some(window),
    );
    assert_eq!(f.app.document_state.tabs.len(), 2);
    assert!(closes_window(&mut f.app, task, window), "the window closed");
}

/// A detached panel hosts no document: nothing closes.
#[test]
fn close_document_in_a_detached_panel_leaves_the_tabs() {
    let mut f = fixture();
    let panel = iced::window::Id::unique();
    f.app.ui_state.windows.insert(
        panel,
        WindowKind::DetachedPanel(crate::panels::PanelKind::Projects),
    );
    press(&mut f, "close_active_document", Some(panel));
    assert_eq!(f.app.document_state.tabs.len(), 2);
}

/// An undocked window whose tab is gone closes nothing.
#[test]
fn close_document_in_a_window_with_no_tab_leaves_the_tabs() {
    let mut f = fixture();
    let orphan = iced::window::Id::unique();
    f.app.ui_state.windows.insert(
        orphan,
        WindowKind::UndockedTab {
            path: PathBuf::from("gone.snxsch"),
            title: "gone".to_string(),
        },
    );
    press(&mut f, "close_active_document", Some(orphan));
    assert_eq!(f.app.document_state.tabs.len(), 2);
}

fn save_as_completion(f: &Fixture, to: &Path) -> Message {
    Message::File(crate::app::FileMsg::DocumentSaved(
        crate::persist::DocumentSaveDone {
            path: to.to_path_buf(),
            revision: 0,
            net_color_revision: 0,
            save_as_from: Some(f.undocked_path.clone()),
            bytes: std::sync::Arc::new(b"saved".to_vec()),
            sequence: 0,
            result: Ok(crate::persist::SaveOutcome::Written),
            recovery_hold: None,
        },
    ))
}

/// Close the undocked window's tab, as the user does while a dialog is open.
fn close_undocked_tab(f: &mut Fixture) {
    let _ = f.app.close_tab_parking_dirty(1);
    assert_eq!(f.app.document_state.tabs.len(), 1);
}

/// A Save As dialog answered after the undocked tab closed writes nothing
/// and leaves the main window's document alone.
#[test]
fn a_save_as_answered_after_the_undocked_tab_closed_writes_nothing() {
    let mut f = fixture();
    close_undocked_tab(&mut f);
    let target = f.main_path.with_file_name("copy.snxsch");
    let message = Message::File(crate::app::FileMsg::SaveAs {
        from_path: f.undocked_path.clone(),
        to_path: target.clone(),
    });
    crate::app::path_ops::settle(&mut f.app, message);
    assert!(!target.exists());
    assert_eq!(std::fs::read(&f.main_path).unwrap(), OLD);
    assert_eq!(f.app.document_state.tabs[0].path, f.main_path);
}

/// A Save As write that lands after the tab closed or the window re-docked
/// must not re-key the main window's tab.
#[test]
fn a_save_as_completion_after_the_tab_closed_leaves_the_main_tab_keyed() {
    let mut f = fixture();
    close_undocked_tab(&mut f);
    f.app.ui_state.windows.remove(&f.window);
    let target = f.main_path.with_file_name("copy.snxsch");
    let message = save_as_completion(&f, &target);
    crate::app::path_ops::settle(&mut f.app, message);
    assert_eq!(f.app.document_state.tabs.len(), 1);
    assert_eq!(f.app.document_state.tabs[0].path, f.main_path);
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

#[test]
fn a_save_as_completion_after_the_window_re_docked_leaves_every_tab_keyed() {
    let mut f = fixture();
    f.app.ui_state.windows.remove(&f.window);
    let target = f.main_path.with_file_name("copy.snxsch");
    let message = save_as_completion(&f, &target);
    crate::app::path_ops::settle(&mut f.app, message);
    assert_eq!(f.app.document_state.tabs[0].path, f.main_path);
    assert_eq!(f.app.document_state.tabs[1].path, f.undocked_path);
    assert_eq!(f.app.document_state.active_path, Some(f.main_path.clone()));
}

/// For the main window the per-window facts are the active-tab facts.
#[test]
fn the_main_windows_document_facts_equal_its_active_tab_facts() {
    let mut f = fixture();
    for kind in [
        TabKind::Schematic,
        TabKind::SymbolEditor(f.main_path.clone()),
        TabKind::FootprintEditor(f.main_path.clone()),
        TabKind::Pcb,
    ] {
        f.app.document_state.tabs[0].kind = kind;
        for window in [None, f.app.ui_state.main_window_id] {
            let ctx = f.app.command_context(window);
            assert_eq!(ctx.document_is_schematic, ctx.has_schematic);
            assert_eq!(
                ctx.document_is_editor,
                ctx.has_symbol_editor || ctx.has_footprint_editor
            );
        }
    }
}

/// A Component Preview has no file of its own: its File rows grey.
#[test]
fn a_component_previews_file_rows_are_greyed() {
    let mut f = fixture();
    let window = component_preview(&mut f.app);
    for id in ["save_document", "save_document_as", "print"] {
        assert!(!row_enabled(&f, Some(window), id), "{id}");
    }
}

/// Close Document in a window whose tab is gone says so in the Messages panel.
#[test]
fn close_document_in_a_window_with_no_tab_is_logged() {
    let mut f = fixture();
    let window = f.window;
    f.app
        .document_state
        .tabs
        .retain(|t| t.path != f.undocked_path);
    {
        use crate::ignore::IgnoreResult;
        crate::diagnostics::init_logging()
            .ignore("another test in this binary already installed the same logger");
    }
    let reported = || {
        crate::diagnostics::recent_entries()
            .iter()
            .filter(|entry| entry.message.contains("no longer open"))
            .count()
    };
    let before = reported();
    let _ = f.app.dispatch_command(
        &command("close_active_document"),
        CommandArgs::none(),
        Some(window),
    );
    assert_eq!(reported(), before + 1, "nothing reached the Messages panel");
}
