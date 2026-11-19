//! #2058: the Export PDF modal's text comes from the catalog.

use signex_i18n::{Arg, I18n, LanguagePref, ids};

use crate::app::{Message, Signex, UiMsg};
use crate::layout_probe::lay_out_texts;

fn caption(i18n: &I18n) -> String {
    let arg = |v: &str| Arg::from(v);
    i18n.message_plain(
        ids::PDF_PAGE_CAPTION,
        &[
            ("page", arg("1")),
            ("pages", arg("3")),
            ("width", arg("4961")),
            ("height", arg("3508")),
            ("zoom", arg("50")),
        ],
    )
}

/// The page caption keeps the English the view built before, pixel sizes
/// ungrouped.
#[test]
fn the_page_caption_keeps_its_english() {
    assert_eq!(caption(&I18n::en_us()), "Page 1 of 3 — 4961×3508 px · 50%");
}

/// #2060: every page-setup name keeps the English its domain `Display`
/// gives, comes from the catalog, and follows the language.
#[test]
fn the_page_setup_names_keep_their_english() {
    use super::page_setup::{
        margin_edge_label, orientation_label, page_size_label, scale_mode_label,
    };
    use signex_output::{MarginEdge, PageSizeKind, PdfScaleMode};
    use signex_types::schematic::SheetOrientation;

    let en = I18n::en_us();
    let xa = I18n::new(&LanguagePref::Locale("en-XA".to_owned()));
    let check = |id, english: String| {
        assert_eq!(en.message_plain(id, &[]), english);
        // The pseudo-locale only changes letters, so `1:1` reads the same.
        if english.chars().any(char::is_alphabetic) {
            assert_ne!(xa.message_plain(id, &[]), english);
        }
    };
    for kind in PageSizeKind::ALL {
        check(page_size_label(kind), kind.to_string());
    }
    for orientation in [SheetOrientation::Landscape, SheetOrientation::Portrait] {
        check(orientation_label(orientation), orientation.to_string());
    }
    for mode in PdfScaleMode::ALL {
        check(scale_mode_label(mode), mode.to_string());
    }
    for edge in MarginEdge::ALL {
        check(margin_edge_label(edge), edge.to_string());
    }
}

/// A preview with default options and no pages, for laying out controls.
fn empty_preview() -> crate::app::state::PreviewState {
    let pdf_options = signex_output::PdfOptions::default();
    crate::app::state::PreviewState {
        source: std::path::PathBuf::new(),
        pages: Vec::new(),
        raster_source: None,
        selected: 0,
        page_setup: crate::app::state::PageSetupInputs::from_options(&pdf_options),
        pdf_options,
        specific_page_input: String::new(),
        range_note: None,
        zoom: 1.0,
        active_tab: crate::app::state::PdfPreviewTab::Preview,
        pan: (0.0, 0.0),
        panning: None,
        sheet_files: Vec::new(),
        selected_files: Default::default(),
        variants: Vec::new(),
        quality: crate::app::state::PdfQuality::Medium300,
        raster_generation: 0,
        source_path: None,
        export_status: None,
    }
}

/// #2060: the margin row lays out the edge names from the catalog, in the
/// language in force.
#[test]
fn the_margin_row_shows_the_catalog_edge_names() {
    let (mut app, _task) = Signex::new();
    let preview = empty_preview();
    let shown = |app: &Signex| {
        lay_out_texts(
            app.view_pdf_page_setup_section(&preview),
            iced::Size::new(1600.0, 400.0),
        )
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect::<Vec<_>>()
    };
    let en = shown(&app);
    for edge in ["Top", "Right", "Bottom", "Left"] {
        assert!(en.iter().any(|t| t == edge), "{edge} in {en:?}");
    }
    let pref = LanguagePref::Locale("en-XA".to_owned());
    let _ = app.update(Message::Ui(UiMsg::LanguageChanged(pref)));
    let xa = shown(&app);
    assert!(!xa.iter().any(|t| t == "Top"), "{xa:?}");
    let top = app
        .ui_state
        .i18n
        .message_plain(ids::BOARD_PANEL_RAIL_TOP, &[]);
    assert!(xa.contains(&top), "{xa:?}");
}

/// A section title follows the language in force.
#[test]
fn the_pdf_text_follows_the_language() {
    let (mut app, _task) = Signex::new();
    let switch = |app: &mut Signex, tag: &str| {
        let pref = LanguagePref::Locale(tag.to_owned());
        let _ = app.update(Message::Ui(UiMsg::LanguageChanged(pref)));
    };
    // The text the section title widget lays out.
    let shown = |app: &Signex| {
        let window = iced::Size::new(400.0, 100.0);
        lay_out_texts(app.pdf_section_title(ids::PDF_SECTION_FILES), window)
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect::<Vec<_>>()
            .join("|")
    };
    switch(&mut app, "en-US");
    assert_eq!(shown(&app), "Choose Project Files");
    switch(&mut app, "en-XA");
    assert_ne!(shown(&app), "Choose Project Files");
    assert_eq!(
        shown(&app),
        app.ui_state.i18n.message_plain(ids::PDF_SECTION_FILES, &[])
    );
    assert_ne!(caption(&app.ui_state.i18n), caption(&I18n::en_us()));
    switch(&mut app, "en-US");
    assert_eq!(shown(&app), "Choose Project Files");
}
