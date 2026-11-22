//! Mockup row 111, Export PDF: the sheet list, Pages, Page Setup, Output,
//! Structure, Bookmarks and Nets, Schematics include, the page toolbar,
//! Cancel and Export (decision 2026-11-19). Nothing here edits the sheet.

use signex_app::app::{Message, PrintPreviewMsg};
use signex_output::{
    ColourMode, MarginEdge, PageRange, PageSize, PdfOptions, PdfScale, PdfScaleMode,
};
use signex_types::schematic::SheetOrientation;

use super::support::{Bench, open_sheet};

fn send(b: &mut Bench, msg: PrintPreviewMsg) {
    b.send(Message::PrintPreview(msg));
}

fn opened() -> Bench {
    let mut b = open_sheet();
    send(&mut b, PrintPreviewMsg::Requested);
    b
}

fn specific(b: &Bench) -> Option<Vec<usize>> {
    match &b.preview().pdf_options.page_range {
        PageRange::Specific(pages) => Some(pages.clone()),
        _ => None,
    }
}

/// The control sends `msg`; the option `read` follows it both ways.
fn flips(b: &mut Bench, msg: fn(bool) -> PrintPreviewMsg, read: fn(&PdfOptions) -> bool) {
    let was = read(&b.preview().pdf_options);
    send(b, msg(!was));
    assert_eq!(read(&b.preview().pdf_options), !was);
    send(b, msg(was));
    assert_eq!(read(&b.preview().pdf_options), was);
}

#[test]
fn the_dialog_opens_with_every_sheet_ticked_and_its_page_drawn() {
    let b = opened();
    let preview = b.preview();
    assert_eq!(preview.sheet_files.len(), 1);
    assert_eq!(preview.selected_files.len(), 1, "every sheet starts ticked");
    assert_eq!(preview.pages.len(), 1, "the page is drawn");
    assert!(!b.dirty(), "opening the dialog is no edit");
}

#[test]
fn clear_empties_the_page_area_and_select_all_brings_it_back() {
    let mut b = opened();
    send(&mut b, PrintPreviewMsg::ClearAllFiles);
    assert!(b.preview().selected_files.is_empty());
    assert!(b.preview().pages.is_empty(), "no sheet, no page");
    send(&mut b, PrintPreviewMsg::SelectAllFiles);
    assert_eq!(b.preview().selected_files.len(), 1);
    assert_eq!(b.preview().pages.len(), 1);
}

#[test]
fn unticking_a_sheet_keeps_it_listed_and_takes_its_page_out() {
    let mut b = opened();
    let key = b.preview().sheet_files[0].0.clone();
    send(&mut b, PrintPreviewMsg::ToggleFile(key.clone()));
    assert_eq!(b.preview().sheet_files.len(), 1, "it stays in the list");
    assert!(!b.preview().selected_files.contains(&key));
    assert!(b.preview().pages.is_empty());
    send(&mut b, PrintPreviewMsg::ToggleFile(key.clone()));
    assert!(b.preview().selected_files.contains(&key));
    assert_eq!(b.preview().pages.len(), 1);
}

#[test]
fn export_with_no_sheet_ticked_is_refused_and_writes_nothing() {
    let mut b = opened();
    send(&mut b, PrintPreviewMsg::ClearAllFiles);
    let out = b.out("none.pdf");
    b.answer(&out);
    send(&mut b, PrintPreviewMsg::Export);
    assert!(!out.exists(), "nothing is written");
    assert!(b.has_notice(), "the refusal says why");
}

#[test]
fn the_pages_switch_picks_all_current_or_specific() {
    let mut b = opened();
    send(&mut b, PrintPreviewMsg::SetPageRangeCurrent);
    assert!(matches!(
        b.preview().pdf_options.page_range,
        PageRange::Current
    ));
    send(&mut b, PrintPreviewMsg::SetPageRangeSpecific);
    assert_eq!(specific(&b), Some(vec![1]));
    send(&mut b, PrintPreviewMsg::SetPageRangeAll);
    assert!(matches!(b.preview().pdf_options.page_range, PageRange::All));
}

#[test]
fn a_typed_page_number_becomes_the_page_range() {
    let mut b = opened();
    send(&mut b, PrintPreviewMsg::SetPageRangeSpecific);
    send(&mut b, PrintPreviewMsg::SetSpecificPageInput("3".into()));
    assert_eq!(specific(&b), Some(vec![3]));
    assert_eq!(b.preview().specific_page_input, "3");
}

/// The field's example in row 111 is "e.g. 1-2, 4".
#[test]
fn the_specific_pages_box_takes_a_list_with_a_range() {
    let mut b = opened();
    send(&mut b, PrintPreviewMsg::SetPageRangeSpecific);
    send(
        &mut b,
        PrintPreviewMsg::SetSpecificPageInput("1-2, 4".into()),
    );
    assert_eq!(specific(&b), Some(vec![1, 2, 4]));
}

/// Inference from row 111 ("the preview redraws as they change"): Export
/// reads the box, so the preview must show the page the box names.
#[test]
fn going_back_to_specific_previews_the_page_the_box_still_names() {
    let mut b = opened();
    send(&mut b, PrintPreviewMsg::SetPageRangeSpecific);
    send(&mut b, PrintPreviewMsg::SetSpecificPageInput("2".into()));
    send(&mut b, PrintPreviewMsg::SetPageRangeAll);
    send(&mut b, PrintPreviewMsg::SetPageRangeSpecific);
    assert_eq!(b.preview().specific_page_input, "2");
    assert_eq!(specific(&b), Some(vec![2]));
}

#[test]
fn a_bad_page_number_keeps_its_text_and_the_range_and_export_refuses_it() {
    let mut b = opened();
    send(&mut b, PrintPreviewMsg::SetPageRangeSpecific);
    send(&mut b, PrintPreviewMsg::SetSpecificPageInput("1".into()));
    send(&mut b, PrintPreviewMsg::SetSpecificPageInput("abc".into()));
    assert_eq!(b.preview().specific_page_input, "abc", "the text is kept");
    assert_eq!(specific(&b), Some(vec![1]), "the range is kept");
    let out = b.out("bad-page.pdf");
    b.answer(&out);
    send(&mut b, PrintPreviewMsg::Export);
    assert!(!out.exists(), "nothing is written");
    assert!(b.has_notice(), "the refusal says why");
}

#[test]
fn page_size_and_orientation_reshape_the_drawn_page() {
    let mut b = opened();
    send(&mut b, PrintPreviewMsg::SetPageSize(PageSize::IsoA3));
    assert_eq!(b.preview().pdf_options.page_size, PageSize::IsoA3);
    send(
        &mut b,
        PrintPreviewMsg::SetOrientation(SheetOrientation::Portrait),
    );
    assert_eq!(
        b.preview().pdf_options.orientation,
        SheetOrientation::Portrait
    );
    let page = &b.preview().pages[0];
    assert!(page.width_px < page.height_px, "the page stands upright");
}

#[test]
fn a_margin_applies_and_a_bad_margin_keeps_its_text_and_the_old_value() {
    let mut b = opened();
    send(
        &mut b,
        PrintPreviewMsg::SetMargin(MarginEdge::Left, "25".into()),
    );
    assert_eq!(b.preview().pdf_options.margins.left_mm, 25.0);
    send(
        &mut b,
        PrintPreviewMsg::SetMargin(MarginEdge::Left, "abc".into()),
    );
    assert_eq!(b.preview().pdf_options.margins.left_mm, 25.0, "value kept");
    assert_eq!(b.preview().page_setup.margin_text(MarginEdge::Left), "abc");
    assert!(
        b.preview().page_setup.notice().is_some(),
        "shown as invalid"
    );
    assert!(!b.dirty());
}

#[test]
fn the_scale_applies_and_a_bad_percent_keeps_its_text_and_the_old_value() {
    let mut b = opened();
    assert_eq!(b.preview().pdf_options.scale, PdfScale::FitToPage);
    send(&mut b, PrintPreviewMsg::SetScaleMode(PdfScaleMode::Percent));
    send(&mut b, PrintPreviewMsg::SetScalePercent("50".into()));
    assert_eq!(b.preview().pdf_options.scale, PdfScale::Percent(50.0));
    send(&mut b, PrintPreviewMsg::SetScalePercent("x".into()));
    assert_eq!(b.preview().pdf_options.scale, PdfScale::Percent(50.0));
    assert_eq!(b.preview().page_setup.percent, "x", "the text is kept");
    assert!(
        b.preview().page_setup.notice().is_some(),
        "shown as invalid"
    );
}

#[test]
fn the_output_section_sets_both_colour_modes_and_the_title_block() {
    let mut b = opened();
    send(
        &mut b,
        PrintPreviewMsg::SetColourMode(ColourMode::Grayscale),
    );
    assert_eq!(b.preview().pdf_options.colour_mode, ColourMode::Grayscale);
    send(
        &mut b,
        PrintPreviewMsg::SetPcbColourMode(ColourMode::BlackAndWhite),
    );
    assert_eq!(
        b.preview().pdf_options.pcb_colour_mode,
        ColourMode::BlackAndWhite
    );
    assert_eq!(
        b.preview().pdf_options.colour_mode,
        ColourMode::Grayscale,
        "the two modes are separate"
    );
    flips(&mut b, PrintPreviewMsg::SetIncludeTitleBlock, |o| {
        o.include_title_block
    });
}

#[test]
fn the_structure_boxes_set_their_options() {
    let mut b = opened();
    flips(&mut b, PrintPreviewMsg::SetUsePhysicalStructure, |o| {
        o.use_physical_structure
    });
    flips(&mut b, PrintPreviewMsg::SetPhysicalDesignators, |o| {
        o.physical_designators
    });
    flips(&mut b, PrintPreviewMsg::SetPhysicalNetLabels, |o| {
        o.physical_net_labels
    });
    flips(&mut b, PrintPreviewMsg::SetPhysicalPorts, |o| {
        o.physical_ports
    });
    flips(&mut b, PrintPreviewMsg::SetPhysicalSheetNumber, |o| {
        o.physical_sheet_number
    });
    flips(&mut b, PrintPreviewMsg::SetPhysicalDocumentNumber, |o| {
        o.physical_document_number
    });
}

#[test]
fn the_bookmark_boxes_and_the_zoom_slider_set_their_options() {
    let mut b = opened();
    flips(&mut b, PrintPreviewMsg::SetGenerateNetsInfo, |o| {
        o.generate_nets_info
    });
    flips(&mut b, PrintPreviewMsg::SetBookmarkPins, |o| {
        o.bookmark_pins
    });
    flips(&mut b, PrintPreviewMsg::SetBookmarkNetLabels, |o| {
        o.bookmark_net_labels
    });
    flips(&mut b, PrintPreviewMsg::SetBookmarkPorts, |o| {
        o.bookmark_ports
    });
    flips(&mut b, PrintPreviewMsg::SetGlobalBookmarks, |o| {
        o.global_bookmarks
    });
    send(&mut b, PrintPreviewMsg::SetBookmarkZoom(0.25));
    assert_eq!(b.preview().pdf_options.bookmark_zoom, 0.25);
    send(&mut b, PrintPreviewMsg::SetBookmarkZoom(2.0));
    assert_eq!(b.preview().pdf_options.bookmark_zoom, 1.0, "held to Close");
}

#[test]
fn the_schematics_include_boxes_set_their_options() {
    let mut b = opened();
    flips(&mut b, PrintPreviewMsg::SetIncludeNoErcMarkers, |o| {
        o.include_no_erc_markers
    });
    flips(&mut b, PrintPreviewMsg::SetIncludeNoConnectMarkers, |o| {
        o.include_no_connect_markers
    });
    flips(&mut b, PrintPreviewMsg::SetIncludeParameterSets, |o| {
        o.include_parameter_sets
    });
    flips(&mut b, PrintPreviewMsg::SetIncludeProbes, |o| {
        o.include_probes
    });
    flips(&mut b, PrintPreviewMsg::SetIncludeBlankets, |o| {
        o.include_blankets
    });
    flips(&mut b, PrintPreviewMsg::SetIncludeNotes, |o| {
        o.include_notes
    });
    flips(&mut b, PrintPreviewMsg::SetIncludeCollapsedNotes, |o| {
        o.include_collapsed_notes
    });
}

#[test]
fn zoom_in_and_fit_page_move_only_the_view() {
    let mut b = opened();
    b.leaves_document(|b| {
        send(b, PrintPreviewMsg::Zoom(1.0));
        assert!(b.preview().zoom > 1.0, "zoomed in");
        send(b, PrintPreviewMsg::ZoomFitPressed);
        assert_eq!(b.preview().zoom, 1.0, "Fit shows the whole page");
    });
}

#[test]
fn cancel_closes_the_dialog_and_changes_nothing() {
    let mut b = opened();
    b.leaves_document(|b| {
        send(b, PrintPreviewMsg::SetPageSize(PageSize::IsoA3));
        send(b, PrintPreviewMsg::Close);
    });
    assert!(b.app.document_state.preview.is_none(), "the dialog closed");
    assert!(!b.dirty());
}

#[test]
fn export_writes_the_pdf_and_the_dialog_stays_with_the_outcome() {
    let mut b = opened();
    let out = b.out("sheet.pdf");
    b.answer(&out);
    b.leaves_document(|b| send(b, PrintPreviewMsg::Export));
    let bytes = std::fs::read(&out).expect("the PDF is written");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(
        b.preview().export_status.is_some(),
        "the footer shows the outcome"
    );
}

#[test]
fn a_cancelled_save_dialog_writes_nothing_and_keeps_the_dialog() {
    let mut b = opened();
    b.cancel_dialog();
    b.leaves_document(|b| send(b, PrintPreviewMsg::Export));
    assert!(b.app.document_state.preview.is_some(), "the dialog stays");
    assert_eq!(std::fs::read_dir(b.dir.path()).expect("the dir").count(), 0);
}
