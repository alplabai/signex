//! #3013 — `set_library_document_options`, `copy_stock_to_library` and
//! `new_library`: Document Options' Apply, Copy to Library's "Copy to
//! <library>" and New Library's Create Library (mockup rows 132-134), on the
//! GUI path (`Signex::update` with the dialog's messages) and the headless
//! path (operands), leaving the same state.

use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable;
use signex_types::coord::Unit;

use super::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::library::messages::LibraryMessage;
use crate::library::stock_copy::{StockCopyMsg, StockPrimitiveKind};
use crate::panels::SheetColor;
use crate::task_driver::TaskDriver;

fn settle(app: &mut Signex, task: iced::Task<Message>) {
    TaskDriver::new().settle(app, task).expect("settles");
}

fn send(app: &mut Signex, msg: LibraryMessage) {
    let task = app.update(Message::Library(msg));
    settle(app, task);
}

/// Run `id` with `operands` as a headless run does, its tasks driven.
fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    let (outcome, task) = app.run_headless_command_in_test(&command, args);
    settle(app, task);
    outcome
}

fn answer(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn arg(name: &str, path: &Path) -> String {
    format!("{name}={}", path.display())
}

/// A new library at `<dir>/<name>/<name>.snxlib`, mounted writable.
fn mounted_library(app: &mut Signex, dir: &Path, name: &str) -> PathBuf {
    let root = dir.join(name).join(format!("{name}.snxlib"));
    crate::library::commands::create_library_file(&root, &Default::default()).expect("library");
    app.library.open_library(root.clone()).expect("mounts");
    root
}

/// A library's display settings as the tests compare them.
fn display(app: &Signex, root: &Path) -> (SheetColor, bool, f32, Unit) {
    let d = app.library.library_at(root).expect("open").display;
    (d.sheet_color, d.grid_visible, d.grid_size_mm, d.unit)
}

#[test]
fn document_options_from_operands_is_the_dialogs_apply() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut by_dialog, _boot) = Signex::new();
    let dialog_root = mounted_library(&mut by_dialog, dir.path(), "a");
    let (mut by_command, _boot) = Signex::new();
    let command_root = mounted_library(&mut by_command, dir.path(), "b");
    let was_visible = display(&by_dialog, &dialog_root).1;

    send(
        &mut by_dialog,
        LibraryMessage::OpenDocumentOptions {
            library_path: dialog_root.clone(),
        },
    );
    send(
        &mut by_dialog,
        LibraryMessage::DocumentOptionsSetSheetColor(SheetColor::White),
    );
    send(&mut by_dialog, LibraryMessage::DocumentOptionsToggleGrid);
    send(
        &mut by_dialog,
        LibraryMessage::DocumentOptionsGridSpacingPicked(5.08),
    );
    send(
        &mut by_dialog,
        LibraryMessage::DocumentOptionsUnitPicked(Unit::Mil),
    );
    send(&mut by_dialog, LibraryMessage::DocumentOptionsApply);

    let grid = format!("grid={}", !was_visible);
    let answered = answer(headless(
        &mut by_command,
        "set_library_document_options",
        &[
            &arg("library", &command_root),
            "sheet_color=white",
            &grid,
            "grid_spacing=5.08",
            "unit=mil",
        ],
    ));
    assert_eq!(
        display(&by_command, &command_root),
        display(&by_dialog, &dialog_root)
    );
    assert_eq!(
        display(&by_command, &command_root),
        (SheetColor::White, !was_visible, 5.08, Unit::Mil)
    );
    assert!(by_command.library.document_options.is_none());
    assert_eq!(answered["sheet_color"], "white");
    assert_eq!(answered["grid"], !was_visible);
    assert_eq!(answered["grid_spacing"], 5.08);
    assert_eq!(answered["unit"], "mil");
}

/// A setting left out keeps its value, and the library can be named by id.
#[test]
fn document_options_by_library_id_keep_what_is_left_out() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _boot) = Signex::new();
    let root = mounted_library(&mut app, dir.path(), "lib");
    let before = display(&app, &root);
    let id = app.library.library_at(&root).expect("open").library_id;
    let library = format!("library={id}");
    let answered = answer(headless(
        &mut app,
        "set_library_document_options",
        &[&library, "unit=um"],
    ));
    assert_eq!(
        display(&app, &root),
        (before.0, before.1, before.2, Unit::Micrometer)
    );
    assert_eq!(answered["library"], root.display().to_string());
}

#[test]
fn document_options_that_cannot_apply_are_refused_with_nothing_changed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _boot) = Signex::new();
    let root = mounted_library(&mut app, dir.path(), "lib");
    let before = display(&app, &root);
    let library = arg("library", &root);
    let elsewhere = arg("library", &dir.path().join("none.snxlib"));
    for (operands, needle) in [
        (vec![elsewhere.as_str()], "No open library is"),
        (
            vec![library.as_str(), "sheet_color=purple"],
            "`sheet_color` is one of",
        ),
        (vec![library.as_str(), "unit=furlong"], "`unit` is one of"),
    ] {
        let reason = refusal(headless(
            &mut app,
            "set_library_document_options",
            &operands,
        ));
        assert!(reason.contains(needle), "{operands:?}: {reason}");
    }
    // The person's open dialog is not taken over.
    send(
        &mut app,
        LibraryMessage::OpenDocumentOptions {
            library_path: root.clone(),
        },
    );
    let reason = refusal(headless(
        &mut app,
        "set_library_document_options",
        &[&library, "unit=mil"],
    ));
    assert!(reason.contains("dialog is open"), "{reason}");
    assert!(app.library.document_options.is_some(), "still open");
    assert_eq!(display(&app, &root), before);
}

/// The stock library mounted as at start, and the file it lists as SOIC-8.
fn with_stock(app: &mut Signex) -> PathBuf {
    let stock =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib");
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock.clone())
        .expect("mount stock");
    app.library
        .library_at(&stock)
        .expect("stock open")
        .cached_footprints
        .iter()
        .find(|footprint| footprint.name == "SOIC-8")
        .and_then(|footprint| footprint.file.clone())
        .expect("the stock SOIC-8 file")
}

/// What a copy leaves: the dialog closed, the library remembered for the
/// next copy, and the copy open in its editor, made from the stock file.
fn copy_left(app: &Signex, target: &Path) -> (bool, bool, usize, Vec<Option<uuid::Uuid>>) {
    let copies: Vec<Option<uuid::Uuid>> = app
        .document_state
        .footprint_editors
        .iter()
        .filter(|(path, _)| path.starts_with(target.parent().expect("a folder")))
        .map(|(_, editor)| editor.file.copied_from)
        .collect();
    (
        app.library.stock_copy.is_none(),
        app.library.stock_copy_target.as_deref() == Some(target),
        app.document_state.footprint_editors.len(),
        copies,
    )
}

#[test]
fn a_stock_copy_from_operands_is_the_dialogs_copy_to() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut by_dialog, _boot) = Signex::new();
    let source = with_stock(&mut by_dialog);
    let dialog_target = mounted_library(&mut by_dialog, dir.path(), "a");
    let opened = by_dialog.request_stock_copy(source, StockPrimitiveKind::Footprint);
    settle(&mut by_dialog, opened);
    send(
        &mut by_dialog,
        LibraryMessage::StockCopy(StockCopyMsg::TargetSelected(dialog_target.clone())),
    );
    send(
        &mut by_dialog,
        LibraryMessage::StockCopy(StockCopyMsg::TargetPicked(dialog_target.clone())),
    );

    let (mut by_command, _boot) = Signex::new();
    let _ = with_stock(&mut by_command);
    let command_target = mounted_library(&mut by_command, dir.path(), "b");
    let answered = answer(headless(
        &mut by_command,
        "copy_stock_to_library",
        &[
            "name=SOIC-8",
            "kind=footprint",
            &arg("path", &command_target),
        ],
    ));
    assert_eq!(answered["kind"], "footprint");
    assert_eq!(answered["library"], command_target.display().to_string());

    let left = copy_left(&by_command, &command_target);
    assert_eq!(left, copy_left(&by_dialog, &dialog_target));
    assert_eq!((left.0, left.1, left.2), (true, true, 1));
    assert!(left.3.len() == 1 && left.3[0].is_some(), "{left:?}");
}

#[test]
fn a_stock_copy_that_cannot_be_made_is_refused_with_nothing_copied() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _boot) = Signex::new();
    let _ = with_stock(&mut app);
    let target = mounted_library(&mut app, dir.path(), "lib");
    let stock = app.library.stock_library_root.clone().expect("stock");
    let to = arg("path", &target);
    let to_stock = arg("path", &stock);
    for (operands, needle) in [
        (
            vec!["name=NOPE", "kind=footprint", to.as_str()],
            "no footprint named NOPE",
        ),
        (
            vec!["name=SOIC-8", "kind=pad", to.as_str()],
            "`kind` is symbol or footprint",
        ),
        (
            vec!["name=SOIC-8", "kind=footprint", to_stock.as_str()],
            "is not open and writable",
        ),
    ] {
        let reason = refusal(headless(&mut app, "copy_stock_to_library", &operands));
        assert!(reason.contains(needle), "{operands:?}: {reason}");
    }
    assert!(app.library.stock_copy.is_none(), "no dialog is left open");
    assert!(app.document_state.footprint_editors.is_empty());
}

/// An app with the project `ProjectA.snxprj` of `dir` open.
fn project_app(dir: &Path) -> (Signex, PathBuf) {
    let app = crate::app::handlers::menu::export::tests::app_workspace(&dir.to_string_lossy(), &[]);
    let project = app.document_state.projects[0].path.clone();
    (app, project)
}

/// The libraries the project waits to create, as the tests compare them.
fn pending(app: &Signex) -> Vec<(PathBuf, bool, bool, String)> {
    let mut specs: Vec<_> = app.document_state.projects[0]
        .pending_libraries
        .values()
        .map(|spec| {
            (
                spec.lib_path.clone(),
                spec.enable_git,
                spec.use_lfs,
                spec.display_name.clone(),
            )
        })
        .collect();
    specs.sort();
    specs
}

#[test]
fn new_library_from_operands_is_the_dialogs_create_library() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lib = dir.path().join("parts").join("parts.snxlib");
    let (mut by_dialog, project) = project_app(dir.path());
    send(
        &mut by_dialog,
        LibraryMessage::CreateLibraryAtPath {
            project_path: project.clone(),
            lib_path: lib.clone(),
        },
    );
    send(
        &mut by_dialog,
        LibraryMessage::LibraryCreateOptionsToggleGit,
    );
    send(
        &mut by_dialog,
        LibraryMessage::LibraryCreateOptionsToggleLfs,
    );
    send(&mut by_dialog, LibraryMessage::LibraryCreateOptionsConfirm);

    let (mut by_command, _) = project_app(dir.path());
    let answered = answer(headless(
        &mut by_command,
        "new_library",
        &[
            &arg("document", &project),
            &arg("path", &lib),
            "git=true",
            "lfs=true",
        ],
    ));
    assert_eq!(pending(&by_command), pending(&by_dialog));
    assert_eq!(pending(&by_command).len(), 1);
    assert_eq!(answered["library"], lib.display().to_string());
    assert_eq!(answered["git"], true);
    assert_eq!(answered["lfs"], true);
    assert_eq!(answered["pending"], true);
    assert!(by_command.document_state.is_document_dirty(&project));
    assert!(by_command.library.create_options.is_none());
    assert!(
        !lib.exists(),
        "nothing is written before the project is saved"
    );
}

#[test]
fn a_new_library_that_cannot_be_created_is_refused_with_nothing_registered() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, project) = project_app(dir.path());
    let taken = dir.path().join("taken.snxlib");
    std::fs::write(&taken, "x").expect("a file in the way");
    let document = arg("document", &project);
    let fresh = arg("path", &dir.path().join("fresh.snxlib"));
    let in_the_way = arg("path", &taken);
    for (operands, needle) in [
        (
            vec![document.as_str(), fresh.as_str(), "lfs=true"],
            "`lfs=true` needs `git=true`",
        ),
        (vec![document.as_str(), in_the_way.as_str()], "taken.snxlib"),
    ] {
        let reason = refusal(headless(&mut app, "new_library", &operands));
        assert!(reason.contains(needle), "{operands:?}: {reason}");
        assert!(app.library.create_options.is_none(), "no dialog left open");
    }
    assert!(pending(&app).is_empty());
    assert!(!app.document_state.is_document_dirty(&project));
}

/// Without operands a headless run is told the operand it lacks, and the
/// palette is told which dialog to use.
#[test]
fn without_operands_each_says_what_it_needs() {
    let (mut app, _boot) = Signex::new();
    for (id, operand, dialog) in [
        (
            "set_library_document_options",
            "library",
            "Document Options",
        ),
        ("copy_stock_to_library", "name", "Copy to Library"),
        ("new_library", "document", "Component Library"),
        ("recover_library", "action", "recovery dialog"),
    ] {
        let command = AppCommandId::new(id).expect("a catalog id");
        let (outcome, _) = app.run_headless_command_in_test(&command, CommandArgs::none());
        let reason = refusal(outcome);
        assert!(
            reason.contains(&format!("needs the operand `{operand}`")),
            "{id}: {reason}"
        );
        let why = super::operand_handlers::needs_operands_reason(&command).expect("a reason");
        assert!(why.contains(dialog), "{id}: {why}");
    }
}
