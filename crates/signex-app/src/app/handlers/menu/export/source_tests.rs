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
