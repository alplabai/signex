//! `export_notebook_pdf` from its operands, as the CLI and an agent run it:
//! the PDF is written where `path` says (the default file otherwise), the
//! answer names what it holds, and a stale reference under `stale=fail`,
//! a bad operand or a document of no project is refused.

use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable as _;
use signex_notebook::PACKAGE_IMPORT;
use signex_notebook::export::sha256_hex;
use signex_types::format::SnxSchematic;
use signex_types::schematic::{SchematicSheet, Symbol};

use super::operands::{export_options, notebook_file};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::keymap::AppCommandId;

const R7_AND_U1: &str = "= Power\nSee #component(\"R7\") and #component(\"U1\").\n";

fn notebook_with(dir: &tempfile::TempDir, body: &str) {
    let folder = dir.path().join("notebook");
    std::fs::create_dir_all(&folder).expect("notebook folder");
    let text = format!("{PACKAGE_IMPORT}\n#show: report.with(doc: \"DN-7\", rev: \"A\")\n{body}");
    std::fs::write(folder.join("design.snxnbk"), text).expect("notebook");
}

fn app_with_notebook(dir: &tempfile::TempDir) -> Signex {
    notebook_with(dir, R7_AND_U1);
    app_workspace(&dir.path().to_string_lossy(), &[])
}

/// A project whose one sheet, on disk and not open, holds U1 alone (with no
/// pins), and whose notebook says `body`.
fn project_with_u1(dir: &tempfile::TempDir, body: &str) -> Signex {
    let mut sheet = SchematicSheet::default();
    sheet.symbols.push(Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "Device:U".into(),
        reference: "U1".into(),
        value: "MCU".into(),
        unit: 1,
        ..Symbol::default()
    });
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(dir.path().join("top.snxsch"), text).expect("write sheet");
    notebook_with(dir, body);
    app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"])
}

fn project_path(app: &Signex) -> PathBuf {
    app.document_state.projects[0].path.clone()
}

fn args(extra: &[&str], project: &Path) -> CommandArgs {
    let id = AppCommandId::new("export_notebook_pdf").expect("a catalog id");
    let document = format!("document={}", project.display());
    let mut operands = vec![document.as_str()];
    operands.extend_from_slice(extra);
    CommandArgs::parse_operands(&id, &operands).expect("operands parse")
}

fn run(app: &mut Signex, extra: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new("export_notebook_pdf").expect("a catalog id");
    let project = project_path(app);
    app.run_operands_settled(&id, &args(extra, &project)).0
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

#[test]
fn the_pdf_is_written_to_path_and_the_answer_says_what_it_holds() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_notebook(&dir);
    let out = dir.path().join("out.pdf");
    let path = format!("path={}", out.display());
    let value = answer(run(&mut app, &[path.as_str(), "bookmarks=false"]));
    let bytes = std::fs::read(&out).expect("written");
    assert_eq!(value["path"], out.display().to_string());
    assert_eq!(value["notebook"], "notebook/design.snxnbk");
    assert_eq!(value["sha256"], sha256_hex(&bytes));
    assert_eq!(value["bytes"], bytes.len());
    assert_eq!(value["pages"], 1);
    assert_eq!(value["paper"]["name"], "a4");
    assert_eq!(value["variant"], "Base");
    assert_eq!(value["bookmarks"]["sections"], 0, "bookmarks=false");
    assert_eq!(value["title"]["doc"], "DN-7");
    assert_eq!(value["references"], 2);
}

#[test]
fn with_no_path_the_default_file_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_notebook(&dir);
    let value = answer(run(&mut app, &[]));
    let expected = dir
        .path()
        .join("Project Outputs")
        .join("notebook")
        .join("ProjectA-DN-7-revA.pdf");
    assert_eq!(value["path"], expected.display().to_string());
    assert!(expected.exists());
    // The default file is there now: it is replaced only when asked.
    let refused = refusal(run(&mut app, &[]));
    assert!(refused.contains("overwrite=true"), "{refused}");
    let again = answer(run(&mut app, &["overwrite=true"]));
    assert_eq!(again["sha256"], value["sha256"], "two exports, one file");
}

/// The stale check reads the named project's design from disk, though
/// nothing of it is open or in view and no panel has read it.
#[test]
fn a_stale_reference_refuses_the_export_under_stale_fail() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_with_u1(&dir, R7_AND_U1);
    assert!(app.ui_state.annotations.scan.is_none(), "no scan read it");
    let out = dir.path().join("out.pdf");
    let path = format!("path={}", out.display());
    let reason = refusal(run(&mut app, &[path.as_str(), "stale=fail"]));
    assert!(reason.contains("R7"), "{reason}");
    assert!(!out.exists(), "nothing written");
    let value = answer(run(&mut app, &[path.as_str()]));
    assert_eq!(value["stale"][0]["name"], "R7", "Warn exports and lists it");
    assert!(out.exists());
}

#[test]
fn a_removed_pin_is_stale() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_with_u1(&dir, "= Power\nThe reset #pin(\"U1\", \"7\").\n");
    let out = dir.path().join("out.pdf");
    let path = format!("path={}", out.display());
    let reason = refusal(run(&mut app, &[path.as_str(), "stale=fail"]));
    assert!(reason.contains("U1.7"), "{reason}");
}

/// Under Fail a project whose design cannot be read (no sheet) is not
/// passed unchecked.
#[test]
fn stale_fail_refuses_a_design_it_cannot_read() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_notebook(&dir);
    let out = dir.path().join("out.pdf");
    let path = format!("path={}", out.display());
    let reason = refusal(run(&mut app, &[path.as_str(), "stale=fail"]));
    assert!(reason.contains("could not be read whole"), "{reason}");
    assert!(!out.exists());
}

#[test]
fn an_export_path_must_be_a_new_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_notebook(&dir);
    let at = |name: &str| format!("path={}", dir.path().join(name).display());
    let not_pdf = refusal(run(&mut app, &[at("out.txt").as_str()]));
    assert!(not_pdf.contains(".pdf"), "{not_pdf}");
    let snx = refusal(run(&mut app, &[at("top.snxsch").as_str()]));
    assert!(snx.contains("Signex document"), "{snx}");
    let notebook = refusal(run(&mut app, &[at("notebook/design.snxnbk").as_str()]));
    assert!(notebook.contains("Signex document"), "{notebook}");
    std::fs::write(dir.path().join("taken.pdf"), "mine").unwrap();
    let taken = refusal(run(&mut app, &[at("taken.pdf").as_str()]));
    assert!(taken.contains("overwrite=true"), "{taken}");
    let kept = std::fs::read(dir.path().join("taken.pdf")).unwrap();
    assert_eq!(kept, b"mine");
    let replaced = answer(run(&mut app, &[at("taken.pdf").as_str(), "overwrite=true"]));
    assert!(replaced["bytes"].as_u64().unwrap_or(0) > 4);
}

#[test]
fn a_bad_operand_is_refused_with_what_it_takes() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_notebook(&dir);
    for (operand, expect) in [
        ("pages=x", "`pages`"),
        ("paper=tabloid", "`paper`"),
        ("stale=ignore", "`stale`"),
        ("annotations=memo", "`annotations`"),
    ] {
        let reason = refusal(run(&mut app, &[operand]));
        assert!(reason.contains(expect), "{operand}: {reason}");
    }
}

#[test]
fn a_missing_notebook_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let reason = refusal(run(&mut app, &[]));
    assert!(reason.contains("notebook/design.snxnbk"), "{reason}");
}

#[test]
fn a_notebook_is_named_inside_its_folder_with_or_without_extension() {
    assert_eq!(
        notebook_file("generated/design-report"),
        "generated/design-report.snxnbk"
    );
    assert_eq!(notebook_file("notebook/design.snxnbk"), "design.snxnbk");
    assert_eq!(notebook_file(" chapters/power.typ "), "chapters/power.typ");
}

#[test]
fn the_operands_set_the_export_settings() {
    let project = PathBuf::from("/p/ProjectA.snxprj");
    let parsed = export_options(&args(
        &[
            "pages=2-3",
            "paper=letter",
            "variant=Lite",
            "annotations=issue,decision",
            "title_block=false",
            "python_source=true",
        ],
        &project,
    ))
    .expect("settings");
    assert_eq!(
        parsed.pages,
        signex_notebook::export::PageSelection::Range { first: 2, last: 3 }
    );
    assert_eq!(parsed.paper, signex_notebook::export::Paper::Letter);
    assert_eq!(parsed.variant, "Lite");
    assert_eq!(parsed.annotations.len(), 2);
    assert!(!parsed.title_block && parsed.python_source);
    let none = export_options(&args(&["annotations=none"], &project)).expect("settings");
    assert!(none.annotations.is_empty());
}
