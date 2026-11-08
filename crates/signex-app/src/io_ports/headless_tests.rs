//! #1931: every converted dialog, driven headlessly, answers from its
//! arguments or refuses naming the missing one; no window opens and nothing
//! waits. Launches are refused and reported.

use std::path::{Path, PathBuf};

use iced::Task;

use super::{HeadlessAnswers, IoPorts, Opener};
use crate::app::contracts::OverlayMsg;
use crate::app::{FileMsg, Message, PreferencesMsg, Signex};
use crate::diagnostics::logged_count;
use crate::library::recovery::LibraryMissingChoice;
use crate::library::state::ComponentsMountSource;
use crate::library::stock_copy::StockCopyMsg;
use crate::library::{LibraryMessage, PrimitivePickerMsg};
use crate::menu_bar::MenuMessage;
use crate::preferences::PrefMsg;
use crate::task_driver::TaskDriver;

/// The refusals are counted in the shared log by dialog title, and several
/// tests here open the same dialog, so they take turns.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn headless_app(answers: &HeadlessAnswers) -> (Signex, std::sync::MutexGuard<'static, ()>) {
    let serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (mut app, _boot) = Signex::new();
    app.ui_state.io = IoPorts::headless(answers.clone());
    (app, serial)
}

/// Make the active shortcut profile a custom one, which is the only kind
/// Export Keyboard Shortcuts writes: the file-writing save dialog these tests
/// drive.
fn with_custom_profile(app: &mut Signex) {
    let create = app.update(pref(PrefMsg::KeymapCreateCustomProfile));
    TaskDriver::new().settle(app, create).expect("created");
}

/// Settle `task`; the dialog titled `title` must log one refusal naming
/// `arg`.
fn assert_refused(app: &mut Signex, task: Task<Message>, title: &str, arg: &str) {
    let needle = format!("{title}: refused in a headless run; give the `{arg}` argument");
    let before = logged_count(&needle);
    TaskDriver::new()
        .settle(app, task)
        .expect("a headless dialog settles without a window");
    assert_eq!(logged_count(&needle), before + 1, "{title}");
}

fn refused_on_message(message: Message, title: &str) {
    let (mut app, _serial) = headless_app(&HeadlessAnswers::default());
    let task = app.update(message);
    assert_refused(&mut app, task, title, "path");
}

fn pref(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

#[test]
fn menu_dialogs_refuse_without_a_path() {
    for (menu, title) in [
        (MenuMessage::OpenProject, "Open Project or Schematic"),
        (MenuMessage::ImportAltium, "Import Altium Project"),
        (
            MenuMessage::NewProject,
            "New Signex Project (a folder named after the project is created)",
        ),
    ] {
        refused_on_message(Message::Menu(menu), title);
    }
}

#[test]
fn library_dialogs_refuse_without_a_path() {
    for (message, title) in [
        (
            LibraryMessage::OpenLibraryDialog,
            "Open Library (*.snxlib/)",
        ),
        (
            LibraryMessage::StockCopy(StockCopyMsg::NewLibraryRequested),
            "New Library",
        ),
        (
            LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Browse),
            "Pick Symbol (*.snxsym)",
        ),
        (
            LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::Locate),
            "Locate Library (*.snxlib)",
        ),
        (
            LibraryMessage::ComponentsPanelAddLibrary(ComponentsMountSource::Installed),
            "Add Library (*.snxlib)",
        ),
        (
            LibraryMessage::ComponentsPanelAddLibrary(ComponentsMountSource::Global),
            "Add Library (*.snxlib)",
        ),
    ] {
        refused_on_message(Message::Library(message), title);
    }
}

#[test]
fn preference_dialogs_refuse_without_a_path() {
    refused_on_message(
        pref(PrefMsg::KeymapImportProfile),
        "Import Signex Keyboard Shortcuts",
    );
}

#[test]
fn the_shortcut_export_dialog_refuses_without_a_path() {
    let (mut app, _serial) = headless_app(&HeadlessAnswers::default());
    // Only a custom profile can be exported.
    let create = app.update(pref(PrefMsg::KeymapCreateCustomProfile));
    TaskDriver::new().settle(&mut app, create).expect("created");
    let task = app.update(pref(PrefMsg::KeymapExportProfile));
    assert_refused(&mut app, task, "Export Signex Keyboard Shortcuts", "path");
}

#[test]
fn the_primitive_save_as_dialog_refuses_without_a_path() {
    let (mut app, _serial) = headless_app(&HeadlessAnswers::default());
    let path = PathBuf::from("/nowhere/part.snxsym");
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "part".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::SymbolEditor(path),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let task = app.update(Message::Menu(MenuMessage::SaveAs));
    assert_refused(&mut app, task, "Save Symbol As", "path");
}

/// A project made from the answered New Project dialog: the project file.
fn new_project(app: &mut Signex, answers: &HeadlessAnswers, dir: &Path) -> PathBuf {
    let chosen = dir.join("Demo.snxprj");
    answers.set("path", chosen.display().to_string());
    let task = app.update(Message::Menu(MenuMessage::NewProject));
    TaskDriver::new().settle(app, task).expect("new project");
    dir.join("Demo").join("Demo.snxprj")
}

#[test]
fn an_answered_new_project_dialog_creates_the_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    let answers = HeadlessAnswers::default();
    let (mut app, _serial) = headless_app(&answers);
    let project = new_project(&mut app, &answers, dir.path());
    assert!(project.is_file(), "the answered path was used");
    assert_eq!(app.document_state.projects.len(), 1);
}

#[test]
fn project_dialogs_refuse_without_their_argument() {
    let dir = tempfile::tempdir().expect("tempdir");
    let answers = HeadlessAnswers::default();
    let (mut app, _serial) = headless_app(&answers);
    new_project(&mut app, &answers, dir.path());
    let task = app.add_existing_to_project(vec![0]);
    assert_refused(&mut app, task, "Add Existing to Project", "paths");
    let task = app.add_new_schematic(vec![0]);
    assert_refused(&mut app, task, "Add New Schematic to Project", "path");
    let task = app.add_project_symbol_library(vec![0]);
    assert_refused(&mut app, task, "New Symbol Library", "path");
    let task = app.add_project_footprint_library(vec![0]);
    assert_refused(&mut app, task, "New PCB Library", "path");
}

#[test]
fn schematic_save_and_export_dialogs_refuse_without_a_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let answers = HeadlessAnswers::default();
    let (mut app, _serial) = headless_app(&answers);
    let project = new_project(&mut app, &answers, dir.path());
    let sheet = project.with_extension("snxsch");
    let task = app.update(Message::File(FileMsg::Opened(Some(sheet))));
    TaskDriver::new()
        .settle(&mut app, task)
        .expect("sheet open");
    let task = app.update(Message::Menu(MenuMessage::SaveAs));
    assert_refused(&mut app, task, "Save Schematic As", "path");
    let task = app.update(Message::Menu(MenuMessage::ExportNetlist));
    assert_refused(&mut app, task, "Export Netlist", "path");
}

#[test]
fn a_save_answer_naming_an_existing_file_needs_overwrite() {
    let dir = tempfile::tempdir().expect("tempdir");
    let existing = dir.path().join("shortcuts.toml");
    std::fs::write(&existing, b"keep me").expect("seed");
    let answers = HeadlessAnswers::default();
    let (mut app, _serial) = headless_app(&answers);
    with_custom_profile(&mut app);
    answers.set("path", existing.display().to_string());
    // The ring shortens long lines, so the needle stops at the path.
    let needle = format!(
        "Export Signex Keyboard Shortcuts: refused in a headless run; {} exists",
        existing.display()
    );
    let before = logged_count(&needle);
    let task = app.update(pref(PrefMsg::KeymapExportProfile));
    TaskDriver::new().settle(&mut app, task).expect("settled");
    assert_eq!(logged_count(&needle), before + 1);
    assert_eq!(std::fs::read(&existing).expect("read"), b"keep me");

    answers.set("path", existing.display().to_string());
    answers.set("overwrite", "true");
    let task = app.update(pref(PrefMsg::KeymapExportProfile));
    TaskDriver::new().settle(&mut app, task).expect("settled");
    assert_ne!(std::fs::read(&existing).expect("read"), b"keep me");
}

#[test]
fn an_answer_is_used_once_and_must_be_absolute() {
    let answers = HeadlessAnswers::default();
    let (mut app, _serial) = headless_app(&answers);
    answers.set("path", "relative/demo.snxprj");
    let needle = "Open Project or Schematic: refused in a headless run; relative/demo.snxprj is not an absolute path";
    let before = logged_count(needle);
    let task = app.update(Message::Menu(MenuMessage::OpenProject));
    TaskDriver::new().settle(&mut app, task).expect("settled");
    assert_eq!(logged_count(needle), before + 1);
    // Taken by that dialog: the next one has no answer left.
    let task = app.update(Message::Menu(MenuMessage::ImportAltium));
    assert_refused(&mut app, task, "Import Altium Project", "path");
}

#[test]
fn a_headless_link_is_refused_and_reported() {
    let (mut app, _serial) = headless_app(&HeadlessAnswers::default());
    let task = app.update(Message::Overlay(OverlayMsg::OpenLink(
        crate::about::ExternalLink::Documentation,
    )));
    TaskDriver::new().settle(&mut app, task).expect("settled");
    let notice = app.document_state.error_notice.as_ref().expect("a notice");
    assert!(notice.detail.contains("headless"), "{}", notice.detail);
}

#[test]
fn the_digikey_flow_fails_at_once_when_the_browser_is_refused() {
    let outcome = crate::library::settings::digikey_oauth::run_blocking(
        "client-id".into(),
        "client-secret".into(),
        "http://127.0.0.1:9/authorize".into(),
        "http://127.0.0.1:9/token".into(),
        crate::library::settings::digikey_oauth::CancelHandle::new(),
        Some(Opener::Refuse),
    );
    match outcome {
        crate::library::settings::digikey_oauth::Outcome::Failed { reason } => {
            assert!(reason.contains("headless"), "{reason}")
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn reveal_is_refused_headless() {
    assert!(matches!(
        Opener::Refuse.reveal(Path::new("/tmp")),
        Err(super::OpenError::Refused)
    ));
}

/// A refused save takes its `overwrite` answer with it: the next save, given
/// only an existing path, must still ask (here: refuse).
#[test]
fn a_refused_save_does_not_leave_overwrite_behind() {
    let dir = tempfile::tempdir().expect("tempdir");
    let existing = dir.path().join("shortcuts.toml");
    std::fs::write(&existing, b"keep me").expect("seed");
    let answers = HeadlessAnswers::default();
    let (mut app, _serial) = headless_app(&answers);
    with_custom_profile(&mut app);
    answers.set("path", "relative/shortcuts.toml");
    answers.set("overwrite", "true");
    let task = app.update(pref(PrefMsg::KeymapExportProfile));
    TaskDriver::new().settle(&mut app, task).expect("settled");

    answers.set("path", existing.display().to_string());
    let task = app.update(pref(PrefMsg::KeymapExportProfile));
    TaskDriver::new().settle(&mut app, task).expect("settled");
    assert_eq!(std::fs::read(&existing).expect("read"), b"keep me");
}

/// An open dialog takes every answer too, `overwrite` included.
#[test]
fn an_open_dialog_takes_overwrite_with_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let existing = dir.path().join("shortcuts.toml");
    std::fs::write(&existing, b"keep me").expect("seed");
    let answers = HeadlessAnswers::default();
    let (mut app, _serial) = headless_app(&answers);
    with_custom_profile(&mut app);
    answers.set("overwrite", "true");
    let task = app.update(Message::Menu(MenuMessage::OpenProject));
    assert_refused(&mut app, task, "Open Project or Schematic", "path");

    answers.set("path", existing.display().to_string());
    let task = app.update(pref(PrefMsg::KeymapExportProfile));
    TaskDriver::new().settle(&mut app, task).expect("settled");
    assert_eq!(std::fs::read(&existing).expect("read"), b"keep me");
}

#[test]
fn an_empty_answer_set_drops_earlier_answers() {
    let answers = HeadlessAnswers::default();
    let (mut app, _serial) = headless_app(&answers);
    answers.set("path", "/nowhere/project.snxprj");
    answers.replace_all([]);
    let task = app.update(Message::Menu(MenuMessage::ImportAltium));
    assert_refused(&mut app, task, "Import Altium Project", "path");
}

#[test]
fn an_empty_paths_answer_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let answers = HeadlessAnswers::default();
    let (mut app, _serial) = headless_app(&answers);
    new_project(&mut app, &answers, dir.path());
    answers.set("paths", "");
    let task = app.add_existing_to_project(vec![0]);
    assert_refused(&mut app, task, "Add Existing to Project", "paths");
}

/// An import whose picked file cannot be read says so on its pane's status
/// line (a folder stands in for an unreadable file).
#[test]
fn an_unreadable_import_sets_the_pane_status() {
    let dir = tempfile::tempdir().expect("tempdir");
    let answers = HeadlessAnswers::default();
    let (mut app, _serial) = headless_app(&answers);
    answers.set("path", dir.path().display().to_string());
    let task = app.update(pref(PrefMsg::KeymapImportProfile));
    TaskDriver::new().settle(&mut app, task).expect("settled");
    assert!(
        app.ui_state
            .preferences_keymap_status
            .starts_with("Could not import keyboard shortcuts:"),
        "{}",
        app.ui_state.preferences_keymap_status
    );
}

/// A command that opens no dialog cannot leave `overwrite` for the next one:
/// the next command's answers replace it.
#[test]
fn a_commands_answers_replace_what_an_earlier_command_left() {
    let dir = tempfile::tempdir().expect("tempdir");
    let existing = dir.path().join("shortcuts.toml");
    std::fs::write(&existing, b"keep me").expect("seed");
    let answers = HeadlessAnswers::default();
    let (mut app, _serial) = headless_app(&answers);
    with_custom_profile(&mut app);
    answers.replace_all([("overwrite".to_string(), "true".to_string())]);
    // No active sheet: Export Netlist returns before any dialog.
    let task = app.update(Message::Menu(MenuMessage::ExportNetlist));
    TaskDriver::new().settle(&mut app, task).expect("settled");

    answers.replace_all([("path".to_string(), existing.display().to_string())]);
    let task = app.update(pref(PrefMsg::KeymapExportProfile));
    TaskDriver::new().settle(&mut app, task).expect("settled");
    assert_eq!(std::fs::read(&existing).expect("read"), b"keep me");
}
