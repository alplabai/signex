//! #1766 — a finished PDF export says where the file went, in the Export
//! window and in the Messages panel; a failed one says why.

use super::protel_hierarchy::placed_app;
use crate::app::Signex;
use crate::app::contracts::ExportWriteOutcome;
use crate::app::handlers::menu::export::settle;

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

fn outcome(path: &str, result: Result<(), String>) -> ExportWriteOutcome {
    ExportWriteOutcome {
        path: path.into(),
        result,
        success_log: format!("Wrote {path} (1 page(s), 10 bytes)"),
        advisory: None,
        warnings: Vec::new(),
        report_in_preview: true,
    }
}

fn status(app: &Signex) -> Option<String> {
    app.document_state
        .preview
        .as_ref()
        .and_then(|p| p.export_status.clone())
}

#[test]
fn a_written_pdf_is_confirmed_with_its_full_path_and_logged() {
    let _ = crate::diagnostics::init_logging();
    let mut app = open_preview("export_status_ok");
    let _ = app.handle_export_written(outcome("/work/amp/amp.pdf", Ok(())));

    let line = status(&app).expect("the Export window shows a status line");
    assert!(line.contains("/work/amp/amp.pdf"), "got {line:?}");
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Wrote /work/amp/amp.pdf"))
    );
}

#[test]
fn a_failed_pdf_write_is_reported_in_the_window_too() {
    let mut app = open_preview("export_status_err");
    let _ = app.handle_export_written(outcome(
        "/ro/amp.pdf",
        Err("Could not write PDF to /ro/amp.pdf:\nread-only".to_string()),
    ));

    let line = status(&app).expect("the Export window shows a status line");
    assert!(line.contains("read-only"), "got {line:?}");
}

#[test]
fn a_non_pdf_export_leaves_the_pdf_window_status_alone() {
    let mut app = open_preview("export_status_other");
    let mut other = outcome("/work/amp/amp.net", Ok(()));
    other.report_in_preview = false;
    let _ = app.handle_export_written(other);
    assert_eq!(status(&app), None);
}

#[test]
fn the_save_target_of_an_open_project_is_its_folder_and_name() {
    let app = placed_app("save_target_proj", &["top.snxsch"], &[], Vec::new());
    let project = app
        .document_state
        .active_document_project()
        .expect("project");
    let target = app.save_target(None);
    assert_eq!(target.dir(), project.path.parent());
    let stem = project.path.file_stem().unwrap().to_string_lossy();
    assert_eq!(
        target.file_name("schematic", "", "pdf"),
        format!("{stem}.pdf")
    );
}
