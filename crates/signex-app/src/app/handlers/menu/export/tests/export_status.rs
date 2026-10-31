//! #1766 — a finished PDF export says where the file went, in the Export
//! window and in the Messages panel; a failed one says why, in both.

use std::path::{Path, PathBuf};

use super::protel_hierarchy::placed_app;
use crate::app::contracts::ExportWriteOutcome;
use crate::app::handlers::menu::export::settle;
use crate::app::handlers::menu::export::write::export_status_line;
use crate::app::handlers::menu::save_target::Panel;
use crate::app::{ExportMsg, Message, PrintPreviewMsg, Signex};

const DIR: &str = "/w/export_status";

fn open_preview(dir: &str) -> Signex {
    let mut app = placed_app(dir, &["top.snxsch"], &[], Vec::new());
    let task = app.handle_print_preview_requested();
    settle(&mut app, task);
    assert!(
        app.document_state.preview.is_some(),
        "fixture: preview open"
    );
    app
}

fn source(app: &Signex) -> PathBuf {
    app.document_state
        .preview
        .as_ref()
        .expect("preview")
        .source
        .clone()
}

fn outcome(path: &str, source: Option<PathBuf>, result: Result<(), String>) -> ExportWriteOutcome {
    ExportWriteOutcome {
        path: path.into(),
        result,
        success_log: format!("Wrote {path} (1 page(s), 10 bytes)"),
        advisory: None,
        warnings: Vec::new(),
        preview_source: source,
    }
}

fn status(app: &Signex) -> Option<String> {
    app.document_state
        .preview
        .as_ref()
        .and_then(|p| p.export_status.clone())
}

/// The newest Messages entry id, installing the logger first. Entries are
/// compared by id so parallel tests cannot evict the one being looked for.
fn mark() -> Option<u64> {
    let _ = crate::diagnostics::init_logging();
    crate::diagnostics::recent_entries().last().map(|e| e.id)
}

fn logged_since(since: Option<u64>, needle: &str) -> bool {
    level_since(since, needle).is_some()
}

fn level_since(since: Option<u64>, needle: &str) -> Option<crate::diagnostics::DiagnosticLevel> {
    crate::diagnostics::recent_entries()
        .into_iter()
        .find(|e| Some(e.id) > since && e.message.contains(needle))
        .map(|e| e.level)
}

#[test]
fn a_written_pdf_is_confirmed_in_the_window_and_its_full_path_logged() {
    let since = mark();
    let mut app = open_preview(DIR);
    let src = source(&app);
    let _ = app.handle_export_written(outcome("/work/amp/logged-once-1766.pdf", Some(src), Ok(())));

    let line = status(&app).expect("the Export window shows a status line");
    assert!(
        line.contains("/work/amp/logged-once-1766.pdf"),
        "got {line:?}"
    );
    assert!(logged_since(since, "Wrote /work/amp/logged-once-1766.pdf"));
}

#[test]
fn a_failed_pdf_write_is_reported_in_the_window_and_logged() {
    let since = mark();
    let mut app = open_preview(DIR);
    let src = source(&app);
    let _ = app.handle_export_written(outcome(
        "/ro/amp.pdf",
        Some(src),
        Err("Could not write PDF to /ro/amp.pdf:\nread-only volume 1766".to_string()),
    ));

    let line = status(&app).expect("the Export window shows a status line");
    assert!(line.contains("read-only"), "got {line:?}");
    assert!(
        logged_since(since, "read-only volume 1766"),
        "the failure reaches Messages"
    );
}

#[test]
fn a_failed_non_pdf_write_is_logged_too() {
    let since = mark();
    let mut app = open_preview(DIR);
    let _ = app.handle_export_written(outcome(
        "/ro/amp.net",
        None,
        Err("Could not write netlist: disk full 1766".to_string()),
    ));
    assert!(logged_since(since, "disk full 1766"));
    assert_eq!(status(&app), None);
}

#[test]
fn a_non_pdf_export_leaves_the_pdf_window_status_alone() {
    let mut app = open_preview(DIR);
    let _ = app.handle_export_written(outcome("/work/amp/amp.net", None, Ok(())));
    assert_eq!(status(&app), None);
}

#[test]
fn an_export_of_another_document_does_not_report_in_this_window() {
    let mut app = open_preview(DIR);
    let other = PathBuf::from("/elsewhere/B.snxsch");
    let _ = app.handle_export_written(outcome("/work/B.pdf", Some(other), Ok(())));
    assert_eq!(status(&app), None);
}

#[test]
fn changing_a_setting_after_an_export_brings_the_page_count_back() {
    let mut app = open_preview(DIR);
    let src = source(&app);
    let _ = app.handle_export_written(outcome("/work/amp/amp.pdf", Some(src), Ok(())));
    assert!(status(&app).is_some());

    let task = app.update(Message::PrintPreview(
        PrintPreviewMsg::SetIncludeTitleBlock(false),
    ));
    settle(&mut app, task);
    assert_eq!(status(&app), None);
}

#[test]
fn clearing_the_file_selection_after_an_export_drops_the_old_confirmation() {
    let mut app = open_preview(DIR);
    let src = source(&app);
    let _ = app.handle_export_written(outcome("/work/amp/amp.pdf", Some(src), Ok(())));

    let task = app.update(Message::PrintPreview(PrintPreviewMsg::ClearAllFiles));
    settle(&mut app, task);
    assert_eq!(status(&app), None);
}

#[test]
fn an_export_click_with_nothing_selected_does_not_keep_the_old_confirmation() {
    let mut app = open_preview(DIR);
    let src = source(&app);
    let _ = app.handle_export_written(outcome("/work/amp/amp.pdf", Some(src), Ok(())));
    app.document_state
        .preview
        .as_mut()
        .unwrap()
        .selected_files
        .clear();

    let _ = app.handle_print_preview_export();
    assert_eq!(status(&app), None);
}

#[test]
fn the_status_line_is_one_short_line() {
    let long = "/very/long/".repeat(20) + "amp.pdf";
    let ok = export_status_line(Path::new(&long), &Ok(()));
    assert!(ok.starts_with("Exported "), "got {ok:?}");
    assert!(ok.ends_with("amp.pdf"), "the file name survives: {ok:?}");
    assert!(ok.chars().count() <= 90, "got {} chars", ok.chars().count());

    let err = export_status_line(Path::new("/x.pdf"), &Err("first\nsecond".to_string()));
    assert!(!err.contains('\n'), "got {err:?}");
    assert!(err.starts_with("Export failed: "), "got {err:?}");
}

#[test]
fn a_finished_pdf_through_the_real_message_is_written_confirmed_and_logged() {
    let since = mark();
    let mut app = open_preview(DIR);
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("amp.pdf");
    let preview = app.document_state.preview.as_ref().unwrap();
    let request = Box::new(crate::app::state::PdfExportRequest {
        source: preview.source.clone(),
        options: preview.pdf_options.clone(),
        files: preview.selected_files.clone(),
        descended_placement: None,
    });

    let task = app.update(Message::Export(ExportMsg::PdfFinished {
        request,
        result: Ok(out.clone()),
    }));
    settle(&mut app, task);

    assert!(out.exists(), "the PDF was written");
    let line = status(&app).expect("status line");
    assert!(line.contains("amp.pdf"), "got {line:?}");
    assert!(logged_since(since, &format!("Wrote {}", out.display())));
}

#[test]
fn the_save_target_of_an_open_project_is_its_folder_and_name() {
    let app = placed_app("/w/save_target_proj", &["top.snxsch"], &[], Vec::new());
    let target = app.save_target(None);
    assert_eq!(
        target.start_dir(Panel::Export).as_deref(),
        Some(Path::new("/w/save_target_proj"))
    );
    assert_eq!(target.file_name("schematic", "", "pdf"), "ProjectA.pdf");
}

#[test]
fn the_save_target_follows_the_source_documents_project_among_several() {
    let mut app = placed_app("/w/proj_a", &["top.snxsch"], &[], Vec::new());
    let mut other = app.document_state.projects[0].clone();
    other.id = app.document_state.mint_project_id();
    other.path = PathBuf::from("/w/proj_b/ProjectB.snxprj");
    other.data.name = "ProjectB".to_string();
    other.data.dir = "/w/proj_b".to_string();
    other.data.schematic_root = Some("b.snxsch".to_string());
    other.data.sheets[0].filename = "b.snxsch".to_string();
    other.data.sheets[0].name = "b.snxsch".to_string();
    app.document_state.projects.push(other);

    let target = app.save_target(Some(Path::new("/w/proj_b/b.snxsch")));
    assert_eq!(
        target.start_dir(Panel::Export).as_deref(),
        Some(Path::new("/w/proj_b"))
    );
    assert_eq!(target.file_name("schematic", "", "pdf"), "ProjectB.pdf");
    let active = app.save_target(Some(Path::new("/w/proj_a/top.snxsch")));
    assert_eq!(
        active.start_dir(Panel::Export).as_deref(),
        Some(Path::new("/w/proj_a"))
    );
}

#[test]
fn a_refusal_logs_a_warning_and_a_real_failure_logs_an_error_without_a_prefix() {
    use crate::diagnostics::DiagnosticLevel;
    let since = mark();
    let mut app = open_preview(DIR);
    app.refuse_export("Select something 1766-refusal");
    app.report_export_failure("BOM export failed: 1766-failure");

    assert_eq!(
        level_since(since, "1766-refusal"),
        Some(DiagnosticLevel::Warning)
    );
    assert_eq!(
        level_since(since, "1766-failure"),
        Some(DiagnosticLevel::Error)
    );
    assert!(!logged_since(since, "Export failed: BOM export failed"));
}
