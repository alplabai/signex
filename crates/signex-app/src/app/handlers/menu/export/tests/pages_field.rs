//! #2787: the Pages field of Export PDF (mockup row 111). After Specific 2,
//! All, Specific the box says 2, the preview shows Sheet 2 and the file
//! prints Sheet 2; a list such as `1, 3` previews and prints both. Driven
//! through `Signex::update`, from opening the dialog to the written file.

use signex_output::PageRange;

use super::pdf_command::three_sheets;
use crate::app::handlers::menu::export::settle;
use crate::app::{ExportMsg, Message, PrintPreviewMsg, Signex};

fn send(app: &mut Signex, message: PrintPreviewMsg) {
    let task = app.update(Message::PrintPreview(message));
    settle(app, task);
}

/// The sheet numbers of the preview's pages, in order.
fn shown_pages(app: &Signex) -> Vec<usize> {
    let preview = app.document_view.preview.as_ref().expect("dialog open");
    preview.pages.iter().map(|page| page.page_number).collect()
}

/// Export what the dialog holds, as Export PDF… does once a file is picked,
/// and return the file's text.
fn export(app: &mut Signex, dir: &std::path::Path) -> String {
    let preview = app.document_view.preview.as_ref().expect("dialog open");
    let request = Box::new(crate::app::state::PdfExportRequest {
        source: preview.source.clone(),
        options: preview.output_options(),
        files: preview.selected_files.clone(),
        descended_placement: None,
    });
    let out = dir.join("pages.pdf");
    let task = app.update(Message::Export(ExportMsg::PdfFinished {
        request,
        result: Ok(out.clone()),
    }));
    settle(app, task);
    String::from_utf8_lossy(&std::fs::read(&out).expect("the PDF was written")).into_owned()
}

fn printed_pages(text: &str) -> usize {
    text.matches("/Type /Page").count() - text.matches("/Type /Pages").count()
}

#[test]
fn specific_all_specific_previews_and_prints_the_page_in_the_box() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = three_sheets(dir.path());
    send(&mut app, PrintPreviewMsg::Requested);
    send(&mut app, PrintPreviewMsg::SetPageRangeSpecific);
    send(
        &mut app,
        PrintPreviewMsg::SetSpecificPageInput("2".to_string()),
    );
    send(&mut app, PrintPreviewMsg::SetPageRangeAll);
    assert_eq!(shown_pages(&app), [1, 2, 3]);
    send(&mut app, PrintPreviewMsg::SetPageRangeSpecific);

    let preview = app.document_view.preview.as_ref().expect("dialog open");
    assert_eq!(preview.specific_page_input, "2", "the box");
    assert!(
        matches!(&preview.pdf_options.page_range, PageRange::Specific(pages) if pages == &[2]),
        "{:?}",
        preview.pdf_options.page_range
    );
    assert_eq!(shown_pages(&app), [2], "the preview");
    let text = export(&mut app, dir.path());
    assert_eq!(printed_pages(&text), 1, "the file");
    assert!(text.contains("(Sheet 2: b.snxsch)"), "Sheet 2 is printed");
    assert!(!text.contains("(Sheet 1: a.snxsch)"), "Sheet 1 is not");
}

#[test]
fn a_list_in_the_box_previews_and_prints_each_page_it_names() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = three_sheets(dir.path());
    send(&mut app, PrintPreviewMsg::Requested);
    send(&mut app, PrintPreviewMsg::SetPageRangeSpecific);
    send(
        &mut app,
        PrintPreviewMsg::SetSpecificPageInput("1, 3".to_string()),
    );
    assert_eq!(shown_pages(&app), [1, 3], "the preview");
    let text = export(&mut app, dir.path());
    assert_eq!(printed_pages(&text), 2, "the file");
    assert!(text.contains("(Sheet 1: a.snxsch)"));
    assert!(text.contains("(Sheet 3: c.snxsch)"));
    assert!(!text.contains("(Sheet 2: b.snxsch)"));
}
