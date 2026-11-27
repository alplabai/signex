//! #2430: Base picked in Export PDF prints Base, never the project's active
//! variant. Base prints no variant name: an empty `${VARIANT}` and no sheet
//! bookmark tag (Caner, 2026-11-20). Driven through `Signex::update`, from
//! opening the dialog to the written file.

use signex_types::schematic::{HAlign, Point, TextNote, VAlign};

use super::net_text_hierarchy::placed_app;
use super::open_with;
use crate::app::handlers::menu::export::settle;
use crate::app::{ExportMsg, Message, PrintPreviewMsg, Signex};

const DIR: &str = "/w/base_variant";

/// The note shows the variant both ways a title block can name it: the
/// `${VARIANT}` special string and the `active_variant` field.
const VARIANT_NOTE: &str = "[${VARIANT}${active_variant}]";

/// A project defining the variant Lite, `active` its active variant, with
/// `note` on the top sheet.
fn project(active: Option<&str>, note: &str) -> Signex {
    let mut app = placed_app(DIR, &["top.snxsch"], &[], Vec::new());
    let data = &mut app.document_state.projects[0].data;
    data.variant_definitions = vec!["Lite".to_string()];
    data.active_variant = active.map(str::to_string);
    let top = std::path::PathBuf::from(DIR).join("top.snxsch");
    let mut sheet = app.document_state.engines[&top].document().clone();
    sheet.text_notes.push(TextNote {
        uuid: uuid::Uuid::from_u128(0x2430),
        text: note.to_string(),
        position: Point::new(20.0, 20.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked: false,
        format: Default::default(),
    });
    open_with(&mut app.document_state, &top, sheet);
    app
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    settle(app, task);
}

/// Open Export PDF, pick `variant` (`None` is the picker's Base row), turn
/// on Use Physical Structure so `${VARIANT}` prints, and export.
fn export(app: &mut Signex, variant: Option<&str>) -> Vec<u8> {
    send(app, Message::PrintPreview(PrintPreviewMsg::Requested));
    send(
        app,
        Message::PrintPreview(PrintPreviewMsg::SetVariant(variant.map(str::to_string))),
    );
    send(
        app,
        Message::PrintPreview(PrintPreviewMsg::SetUsePhysicalStructure(true)),
    );
    let preview = app.document_state.preview.as_ref().expect("dialog open");
    let request = Box::new(crate::app::state::PdfExportRequest {
        source: preview.source.clone(),
        options: preview.pdf_options.clone(),
        files: preview.selected_files.clone(),
        descended_placement: None,
    });
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("variant.pdf");
    send(
        app,
        Message::Export(ExportMsg::PdfFinished {
            request,
            result: Ok(out.clone()),
        }),
    );
    std::fs::read(&out).expect("the PDF was written")
}

#[test]
fn base_picked_while_the_project_is_on_lite_prints_base() {
    let mut on_lite = project(Some("Lite"), VARIANT_NOTE);
    let base = export(&mut on_lite, None);
    assert_eq!(
        on_lite
            .document_state
            .preview
            .as_ref()
            .and_then(|p| p.pdf_options.variant.as_deref()),
        Some("Base"),
        "the Base row is stored by name"
    );

    // The same page with "[]" typed in: Base prints no variant name, and no
    // trace of Lite.
    let mut typed = project(None, "[]");
    assert_eq!(base, export(&mut typed, None));

    // Lite picked prints Lite, so the comparison above can tell them apart.
    let mut lite = project(Some("Lite"), VARIANT_NOTE);
    assert_ne!(base, export(&mut lite, Some("Lite")));
}

fn contains(pdf: &[u8], needle: &[u8]) -> bool {
    pdf.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn base_with_physical_structure_prints_no_variant_name_or_bookmark_tag() {
    let mut base = project(None, "[${VARIANT}]");
    let pdf = export(&mut base, None);
    let mut typed = project(None, "[]");
    assert_eq!(
        pdf,
        export(&mut typed, None),
        "${{VARIANT}} is empty for Base"
    );
    assert!(!contains(&pdf, b" [Base]"), "no Base sheet-bookmark tag");

    // A named variant still prints its name and tags the sheet bookmarks.
    let mut lite = project(Some("Lite"), "[${VARIANT}]");
    let lite_pdf = export(&mut lite, Some("Lite"));
    assert!(
        contains(&lite_pdf, b" [Lite]"),
        "the Lite sheet bookmark is tagged"
    );
}

#[test]
fn the_dialog_opens_on_the_projects_variant_named() {
    let mut on_lite = project(Some("Lite"), VARIANT_NOTE);
    send(
        &mut on_lite,
        Message::PrintPreview(PrintPreviewMsg::Requested),
    );
    let chosen = |app: &Signex| {
        app.document_state
            .preview
            .as_ref()
            .and_then(|p| p.pdf_options.variant.clone())
    };
    assert_eq!(chosen(&on_lite).as_deref(), Some("Lite"));

    let mut on_base = project(None, VARIANT_NOTE);
    send(
        &mut on_base,
        Message::PrintPreview(PrintPreviewMsg::Requested),
    );
    assert_eq!(chosen(&on_base).as_deref(), Some("Base"));
}

/// Open Export PDF, turn on Use Physical Structure so `${VARIANT}` prints,
/// pick `variant` (`None` is the picker's Base row), and return the pixels
/// of the shown preview page.
fn preview(app: &mut Signex, variant: Option<&str>) -> Vec<u8> {
    send(app, Message::PrintPreview(PrintPreviewMsg::Requested));
    send(
        app,
        Message::PrintPreview(PrintPreviewMsg::SetUsePhysicalStructure(true)),
    );
    send(
        app,
        Message::PrintPreview(PrintPreviewMsg::SetVariant(variant.map(str::to_string))),
    );
    let preview = app.document_state.preview.as_ref().expect("dialog open");
    match preview.pages[preview.selected].handle() {
        Some(iced::widget::image::Handle::Rgba { pixels, .. }) => pixels.to_vec(),
        _ => panic!("the shown preview page is drawn"),
    }
}

#[test]
fn base_picked_while_the_project_is_on_lite_previews_base() {
    let mut on_lite = project(Some("Lite"), VARIANT_NOTE);
    let base = preview(&mut on_lite, None);

    // The preview shows what the PDF prints: "[]", no trace of Lite.
    let mut typed = project(None, "[]");
    assert_eq!(base, preview(&mut typed, None));

    // Lite picked previews Lite, so the comparison above can tell them apart.
    let mut lite = project(Some("Lite"), VARIANT_NOTE);
    assert_ne!(base, preview(&mut lite, Some("Lite")));
}
