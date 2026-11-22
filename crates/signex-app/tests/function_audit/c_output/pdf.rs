//! Rows m167 (the `.snxout` name) and m168 (the PDF watermark, layers and
//! bookmarks), on a copy of the sample project.

use std::path::PathBuf;

use signex_app::app::{FileMsg, Message, PdfOptionMsg, PreviewState, PrintPreviewMsg, Signex};
use signex_app::io_ports::{HeadlessAnswers, IoPorts};
use signex_app::panels::PanelKind;
use signex_output::{PartBookmarkOrder, PdfLayer, WatermarkPages, WatermarkText};

use super::support::fresh_app;
use crate::fixture::{Fixture, fixture};
use crate::settle_support::settle;

struct Rig {
    app: Signex,
    sheet: PathBuf,
    fx: Fixture,
}

/// The sample's root sheet with the Export PDF dialog open.
fn rig() -> Rig {
    let fx = fixture().expect("the sample project");
    let mut app = fresh_app();
    settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(fx.sample.clone()))),
    );
    let sheet = fx.sample.with_extension("snxsch");
    settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(sheet.clone()))),
    );
    settle(&mut app, Message::PrintPreview(PrintPreviewMsg::Requested));
    Rig { app, sheet, fx }
}

fn option(rig: &mut Rig, msg: PdfOptionMsg) {
    settle(&mut rig.app, Message::from(msg));
}

fn preview(rig: &Rig) -> &PreviewState {
    rig.app
        .document_state
        .preview
        .as_ref()
        .expect("the dialog is open")
}

/// Decision 2026-11-19 keeps "Export Job" as the UI name of a `.snxout`
/// document, over the name mockup row 167 draws.
#[test]
fn the_snxout_panel_reads_export_jobs() {
    assert_eq!(PanelKind::OutputJobs.label(), "Export Jobs");
}

#[test]
fn the_watermark_and_the_layers_are_off_by_default() {
    let rig = rig();
    let options = preview(&rig).output_options();
    assert_eq!(options.watermark, None);
    assert_eq!(options.layers, None);
}

#[test]
fn print_a_watermark_writes_it_and_switching_off_keeps_its_settings() {
    let mut rig = rig();
    option(&mut rig, PdfOptionMsg::WatermarkToggled(true));
    option(
        &mut rig,
        PdfOptionMsg::WatermarkOpacityChanged("40".to_string()),
    );
    let on = preview(&rig).output_options().watermark.expect("printed");
    assert_eq!(on.text, WatermarkText::Draft);
    assert_eq!(on.opacity_percent, 40);

    option(&mut rig, PdfOptionMsg::WatermarkToggled(false));
    assert_eq!(preview(&rig).output_options().watermark, None);
    option(&mut rig, PdfOptionMsg::WatermarkToggled(true));
    assert_eq!(preview(&rig).output_options().watermark, Some(on));
}

#[test]
fn the_watermark_fields_reach_what_the_pdf_prints() {
    let mut rig = rig();
    option(&mut rig, PdfOptionMsg::WatermarkToggled(true));
    option(
        &mut rig,
        PdfOptionMsg::WatermarkTextChanged("FOR BENCH USE".to_string()),
    );
    option(
        &mut rig,
        PdfOptionMsg::WatermarkAngleChanged("30".to_string()),
    );
    option(
        &mut rig,
        PdfOptionMsg::WatermarkPagesPicked(WatermarkPages::FirstOnly),
    );
    let printed = preview(&rig).output_options().watermark.expect("printed");
    assert_eq!(
        printed.text,
        WatermarkText::Custom("FOR BENCH USE".to_string())
    );
    assert_eq!(printed.angle_deg, 30.0);
    assert_eq!(printed.pages, WatermarkPages::FirstOnly);
}

#[test]
fn add_date_and_revision_prints_a_line_under_the_text() {
    let mut rig = rig();
    option(&mut rig, PdfOptionMsg::WatermarkToggled(true));
    if let Some(preview) = rig.app.document_state.preview.as_mut() {
        preview.dialog.watermark.revision = "B".to_string();
        preview.dialog.watermark.date = "2026-11-19".to_string();
    }
    let off = preview(&rig).output_options().watermark.expect("printed");
    assert_eq!(off.revision_line, None);
    option(&mut rig, PdfOptionMsg::WatermarkRevisionToggled(true));
    let on = preview(&rig).output_options().watermark.expect("printed");
    assert_eq!(on.revision_line.as_deref(), Some("Rev B · 2026-11-19"));
}

#[test]
fn a_bad_opacity_is_refused_and_keeps_the_text() {
    let mut rig = rig();
    option(&mut rig, PdfOptionMsg::WatermarkToggled(true));
    option(
        &mut rig,
        PdfOptionMsg::WatermarkOpacityChanged("35".to_string()),
    );
    for bad in ["150", "-1", "abc"] {
        option(
            &mut rig,
            PdfOptionMsg::WatermarkOpacityChanged(bad.to_string()),
        );
        let draft = &preview(&rig).dialog.watermark;
        assert_eq!(draft.settings.opacity_percent, 35, "{bad:?}");
        assert_eq!(draft.opacity_input, bad, "the typed text is kept");
    }
}

#[test]
fn a_bad_angle_is_refused_and_keeps_the_text() {
    let mut rig = rig();
    option(&mut rig, PdfOptionMsg::WatermarkToggled(true));
    option(
        &mut rig,
        PdfOptionMsg::WatermarkAngleChanged("90".to_string()),
    );
    for bad in ["720", "abc", "NaN"] {
        option(
            &mut rig,
            PdfOptionMsg::WatermarkAngleChanged(bad.to_string()),
        );
        let draft = &preview(&rig).dialog.watermark;
        assert_eq!(draft.settings.angle_deg, 90.0, "{bad:?}");
        assert_eq!(draft.angle_input, bad, "the typed text is kept");
    }
}

#[test]
fn write_pdf_layers_carries_which_groups_are_visible_on_open() {
    let mut rig = rig();
    option(&mut rig, PdfOptionMsg::PdfLayersToggled(true));
    option(
        &mut rig,
        PdfOptionMsg::PdfLayerVisibilityToggled(PdfLayer::Parts, false),
    );
    let layers = preview(&rig).output_options().layers.expect("written");
    assert!(layers.hidden_on_open.contains(&PdfLayer::Parts));
    assert!(!layers.hidden_on_open.contains(&PdfLayer::WiresAndBuses));

    option(&mut rig, PdfOptionMsg::PdfLayersToggled(false));
    assert_eq!(
        preview(&rig).output_options().layers,
        None,
        "off is a flat PDF"
    );
}

#[test]
fn the_watermark_layer_row_is_live_only_while_a_watermark_prints() {
    let mut rig = rig();
    assert!(!preview(&rig).layer_row_enabled(PdfLayer::Watermark));
    assert!(preview(&rig).layer_row_enabled(PdfLayer::Parts));
    option(&mut rig, PdfOptionMsg::WatermarkToggled(true));
    assert!(preview(&rig).layer_row_enabled(PdfLayer::Watermark));
}

#[test]
fn the_bookmark_switches_set_what_the_export_writes() {
    let mut rig = rig();
    option(&mut rig, PdfOptionMsg::BookmarkSheetsToggled(false));
    option(&mut rig, PdfOptionMsg::BookmarkPartsToggled(false));
    let options = preview(&rig).output_options();
    assert!(!options.bookmark_sheets);
    assert!(!options.bookmark_parts);
    assert!(
        !preview(&rig).part_order_enabled(),
        "the Group switch follows One bookmark per part"
    );

    option(&mut rig, PdfOptionMsg::BookmarkPartsToggled(true));
    option(
        &mut rig,
        PdfOptionMsg::PartBookmarkOrderPicked(PartBookmarkOrder::Alphabetical),
    );
    let options = preview(&rig).output_options();
    assert!(options.bookmark_parts);
    assert_eq!(options.part_bookmark_order, PartBookmarkOrder::Alphabetical);
}

/// Decision 2026-11-20: the picker's Base row is stored as Base by name.
#[test]
fn the_variant_row_holds_one_variant_and_base_is_named() {
    let mut rig = rig();
    settle(
        &mut rig.app,
        Message::PrintPreview(PrintPreviewMsg::SetVariant(None)),
    );
    assert_eq!(
        preview(&rig).pdf_options.variant.as_deref(),
        Some(signex_types::project::BASE_VARIANT)
    );
}

#[test]
fn the_options_edit_no_document() {
    let mut rig = rig();
    for msg in [
        PdfOptionMsg::WatermarkToggled(true),
        PdfOptionMsg::WatermarkTextChanged("X".to_string()),
        PdfOptionMsg::PdfLayersToggled(true),
        PdfOptionMsg::BookmarkPartsToggled(false),
    ] {
        option(&mut rig, msg);
    }
    assert!(!rig.app.document_state.is_document_dirty(&rig.sheet));
}

#[test]
fn close_ends_the_dialog_and_leaves_the_sheet_clean() {
    let mut rig = rig();
    let before = serde_json::to_string(
        rig.app
            .document_state
            .active_engine()
            .expect("the sheet")
            .document(),
    )
    .expect("the sheet as JSON");
    option(&mut rig, PdfOptionMsg::WatermarkToggled(true));
    settle(&mut rig.app, Message::PrintPreview(PrintPreviewMsg::Close));
    assert!(rig.app.document_state.preview.is_none());
    let after = serde_json::to_string(
        rig.app
            .document_state
            .active_engine()
            .expect("the sheet")
            .document(),
    )
    .expect("the sheet as JSON");
    assert_eq!(after, before);
    assert!(!rig.app.document_state.is_document_dirty(&rig.sheet));
}

#[test]
fn export_with_a_watermark_writes_a_pdf_and_keeps_the_dialog_open() {
    let mut rig = rig();
    let target = rig
        .fx
        .sample
        .parent()
        .expect("the sample's folder")
        .join("stamped.pdf");
    let answers = HeadlessAnswers::default();
    answers.replace_all([("path".to_string(), target.display().to_string())]);
    rig.app.ui_state.io = IoPorts::headless(answers);
    option(&mut rig, PdfOptionMsg::WatermarkToggled(true));
    settle(&mut rig.app, Message::PrintPreview(PrintPreviewMsg::Export));
    let bytes = std::fs::read(&target).expect("the PDF was written");
    assert!(bytes.starts_with(b"%PDF"));
    assert!(rig.app.document_state.preview.is_some(), "the dialog stays");
    assert!(!rig.app.document_state.is_document_dirty(&rig.sheet));
}

#[test]
fn a_cancelled_export_writes_nothing() {
    let mut rig = rig();
    let folder = rig
        .fx
        .sample
        .parent()
        .expect("the sample's folder")
        .to_path_buf();
    let before = std::fs::read_dir(&folder).expect("the folder").count();
    rig.app.ui_state.io = IoPorts::headless(HeadlessAnswers::default());
    settle(&mut rig.app, Message::PrintPreview(PrintPreviewMsg::Export));
    assert_eq!(
        std::fs::read_dir(&folder).expect("the folder").count(),
        before
    );
}
