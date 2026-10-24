//! #833 — an async PDF-export completion exports the document it was started
//! from, not whichever tab is active when the OS dialog returns.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::tests::{add_project, app_workspace, open_with, sheet_with_net};
use crate::app::Signex;

/// Count `/Type /Page` objects (not `/Pages`) in the PDF.
fn page_objects(bytes: &[u8]) -> usize {
    let text = String::from_utf8_lossy(bytes);
    text.matches("/Type /Page")
        .count()
        .saturating_sub(text.matches("/Type /Pages").count())
}

/// Project A (one page, active when the dialog opens) and project B (three
/// pages). Returns the app, A's page path and B's first page path.
fn two_projects() -> (Signex, PathBuf, PathBuf) {
    let mut app = app_workspace("/w/a", &["a.snxsch"]);
    add_project(&mut app, "/w/b", &["b1.snxsch", "b2.snxsch", "b3.snxsch"]);
    let a = Path::new("/w/a").join("a.snxsch");
    open_with(
        &mut app.document_state,
        &a,
        sheet_with_net("R_A", "NET_A", &[]),
    );
    let mut first_b = PathBuf::new();
    for (i, name) in ["b1.snxsch", "b2.snxsch", "b3.snxsch"].iter().enumerate() {
        let p = Path::new("/w/b").join(name);
        if i == 0 {
            first_b = p.clone();
        }
        open_with(
            &mut app.document_state,
            &p,
            sheet_with_net(&format!("R_B{i}"), &format!("NET_B{i}"), &[]),
        );
    }
    app.document_state.active_path = Some(a.clone());
    (app, a, first_b)
}

#[test]
fn switching_tabs_while_the_pdf_dialog_is_open_exports_the_original_document() {
    let (mut app, a, b) = two_projects();
    let out = std::env::temp_dir().join(format!("signex-833-{}.pdf", Uuid::new_v4()));

    // The dialog opened on A; the user then clicks over to B's tab.
    app.document_state.active_path = Some(b);
    let _ = app.handle_export_pdf_finished(a, Ok(out.clone()));

    let bytes = std::fs::read(&out).expect("the PDF is written");
    assert_eq!(
        page_objects(&bytes),
        1,
        "the PDF must hold project A's single page, not project B's three"
    );
    std::fs::remove_file(&out).ok();
}

#[test]
fn a_closed_source_document_refuses_the_pdf_export_and_says_so() {
    let (mut app, a, _b) = two_projects();
    let _ = crate::diagnostics::init_logging();
    let out = std::env::temp_dir().join(format!("signex-833-gone-{}.pdf", Uuid::new_v4()));

    // The source tab was closed while the dialog was up.
    app.document_state.engines.remove(&a);
    let _ = app.handle_export_pdf_finished(a, Ok(out.clone()));

    assert!(!out.exists(), "nothing is written for a vanished source");
    assert!(
        app.document_state.error_notice.is_some(),
        "the refusal must be visible to the user"
    );
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("a.snxsch") && e.message.contains("PDF export refused")),
        "the refusal must reach the Messages panel"
    );
}

#[test]
fn the_no_netlist_warning_names_the_exported_document_not_the_current_tab() {
    let _ = crate::diagnostics::init_logging();
    let source = Path::new("/w/exported-852/source.snxsch");
    let ctx = signex_output::ExportContext {
        sheets: Vec::new(),
        metadata: Default::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
    };

    super::log_stitch_issues(source, &ctx, &super::ExportIssues::default());

    assert!(
        crate::diagnostics::recent_entries().iter().any(|e| {
            e.message.contains("no netlist derived") && e.message.contains("exported-852")
        }),
        "the warning must name the exported document"
    );
}

#[test]
fn netlist_finished_exports_the_document_it_started_from() {
    let (mut app, a, b) = two_projects();
    let out = std::env::temp_dir().join(format!("signex-852-{}.net", Uuid::new_v4()));

    app.document_state.active_path = Some(b);
    let _ = app.handle_export_netlist_finished(a, Ok(out.clone()));

    let text = std::fs::read_to_string(&out).expect("the netlist is written");
    assert!(text.contains("R_A"), "project A's part is exported: {text}");
    assert!(!text.contains("R_B"), "project B's parts are not: {text}");
    std::fs::remove_file(&out).ok();
}

#[test]
fn a_closed_netlist_source_writes_nothing_and_says_so() {
    let (mut app, a, _b) = two_projects();
    let _ = crate::diagnostics::init_logging();
    let out = std::env::temp_dir().join(format!("signex-852-gone-{}.net", Uuid::new_v4()));

    app.document_state.engines.remove(&a);
    let _ = app.handle_export_netlist_finished(a, Ok(out.clone()));

    assert!(!out.exists());
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Netlist export refused") && e.message.contains("a.snxsch")),
        "the refusal must reach the Messages panel"
    );
}

#[test]
fn bom_finished_exports_the_document_it_started_from() {
    let (mut app, a, b) = two_projects();
    let out = std::env::temp_dir().join(format!("signex-852-{}.csv", Uuid::new_v4()));

    app.document_state.active_path = Some(b);
    let _ = app.handle_export_bom_finished(a, Ok(out.clone()));

    let text = std::fs::read_to_string(&out).expect("the BOM is written");
    assert!(text.contains("R_A"), "project A's part is listed: {text}");
    assert!(!text.contains("R_B"), "project B's parts are not: {text}");
    std::fs::remove_file(&out).ok();
}

#[test]
fn a_pending_pdf_stash_for_another_document_is_left_alone() {
    let (mut app, a, b) = two_projects();
    let out = std::env::temp_dir().join(format!("signex-852-pend-{}.pdf", Uuid::new_v4()));
    app.document_state.pending_pdf = Some(crate::app::state::PendingPdfExport {
        source: b.clone(),
        options: Default::default(),
        files: std::collections::HashSet::new(),
    });

    // A completion for A must not consume (or be filtered by) B's stash: the
    // empty `files` set would otherwise reject the export.
    let _ = app.handle_export_pdf_finished(a, Ok(out.clone()));

    assert!(out.exists(), "A exports with defaults");
    assert!(
        app.document_state
            .pending_pdf
            .as_ref()
            .is_some_and(|p| p.source == b),
        "B's stash survives for B's own completion"
    );
    std::fs::remove_file(&out).ok();
}
