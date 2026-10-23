//! #353 — the PDF export modal exposes page size, orientation, margins and
//! scale. Drives the real message path (`PrintPreviewMsg::Requested` ->
//! `Set*`) against a real engine; the preview is re-rasterised on every
//! accepted edit, so the page bitmap's aspect ratio is asserted too.

use signex_app::app::{Message, PrintPreviewMsg, Signex};
use signex_output::{MarginEdge, Orientation, PageSize, PageSizeKind, PdfScale, PdfScaleMode};
use signex_types::schematic::SchematicSheet;
use std::collections::HashMap;
use std::path::PathBuf;

fn app_with_sheet(paper: &str) -> Signex {
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: paper.to_string(),
        root_sheet_page: "1".to_string(),
        symbols: Vec::new(),
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: HashMap::new(),
        unknown: Default::default(),
    };
    let path = PathBuf::from("page-setup.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _t) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "page-setup".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    let _ = app.update(Message::PrintPreview(PrintPreviewMsg::Requested));
    app
}

fn send(app: &mut Signex, msg: PrintPreviewMsg) {
    let _ = app.update(Message::PrintPreview(msg));
}

fn preview(app: &Signex) -> &signex_app::app::PreviewState {
    app.document_state.preview.as_ref().expect("preview open")
}

fn first_page_aspect(app: &Signex) -> f64 {
    let page = &preview(app).pages[0];
    f64::from(page.width_px) / f64::from(page.height_px)
}

#[test]
fn an_a3_landscape_sheet_preselects_iso_a3_landscape() {
    let app = app_with_sheet("A3");
    let opts = &preview(&app).pdf_options;
    assert_eq!(opts.page_size, PageSize::IsoA3);
    assert_eq!(opts.orientation, Orientation::Landscape);
    assert_eq!(opts.page_size.kind(), PageSizeKind::IsoA3);
}

#[test]
fn every_page_size_is_selectable_and_reshapes_the_preview() {
    let mut app = app_with_sheet("A4");
    for kind in PageSizeKind::ALL {
        let current = preview(&app).pdf_options.page_size;
        let orientation = preview(&app).pdf_options.orientation;
        let size = kind.select(current, orientation);
        send(&mut app, PrintPreviewMsg::SetPageSize(size));
        let opts = &preview(&app).pdf_options;
        assert_eq!(opts.page_size.kind(), kind);
        let (w, h) = opts.page_size.dimensions_mm(opts.orientation);
        assert!(
            (first_page_aspect(&app) - w / h).abs() < 0.02,
            "{kind}: preview aspect {} vs {}",
            first_page_aspect(&app),
            w / h
        );
    }
}

#[test]
fn custom_size_takes_typed_millimetres_and_keeps_the_old_value_on_garbage() {
    let mut app = app_with_sheet("A4");
    send(
        &mut app,
        PrintPreviewMsg::SetPageSize(
            PageSizeKind::Custom.select(PageSize::IsoA4, Orientation::Landscape),
        ),
    );
    send(&mut app, PrintPreviewMsg::SetCustomWidth("400".into()));
    send(&mut app, PrintPreviewMsg::SetCustomHeight("100".into()));
    assert_eq!(
        preview(&app).pdf_options.page_size,
        PageSize::Custom {
            width_mm: 400.0,
            height_mm: 100.0
        }
    );
    assert!((first_page_aspect(&app) - 4.0).abs() < 0.05);

    for bad in ["", "abc", "-5", "0", "9.9"] {
        send(&mut app, PrintPreviewMsg::SetCustomWidth(bad.into()));
        assert_eq!(
            preview(&app).pdf_options.page_size,
            PageSize::Custom {
                width_mm: 400.0,
                height_mm: 100.0
            },
            "{bad:?}"
        );
        assert!(preview(&app).page_setup.notice().is_some(), "{bad:?}");
    }
}

#[test]
fn flipping_orientation_updates_the_options_and_the_preview() {
    let mut app = app_with_sheet("A3");
    assert!(first_page_aspect(&app) > 1.0);
    send(
        &mut app,
        PrintPreviewMsg::SetOrientation(Orientation::Portrait),
    );
    assert_eq!(preview(&app).pdf_options.orientation, Orientation::Portrait);
    assert!(first_page_aspect(&app) < 1.0);
}

#[test]
fn margins_apply_clamp_and_reject_without_losing_the_last_good_value() {
    let mut app = app_with_sheet("A4");
    send(
        &mut app,
        PrintPreviewMsg::SetMargin(MarginEdge::Left, "25".into()),
    );
    assert_eq!(preview(&app).pdf_options.margins.left_mm, 25.0);
    assert_eq!(preview(&app).pdf_options.margins.right_mm, 10.0);

    for bad in ["", "abc", "-5", "0"] {
        send(
            &mut app,
            PrintPreviewMsg::SetMargin(MarginEdge::Left, bad.into()),
        );
        assert_eq!(preview(&app).pdf_options.margins.left_mm, 25.0, "{bad:?}");
        assert!(preview(&app).page_setup.notice().is_some(), "{bad:?}");
    }

    // A4 landscape is 297 mm wide: a 900 mm margin is limited to fit.
    send(
        &mut app,
        PrintPreviewMsg::SetMargin(MarginEdge::Left, "900".into()),
    );
    let m = preview(&app).pdf_options.margins;
    assert!(m.left_mm + m.right_mm < 297.0, "{m:?}");
    assert!(preview(&app).page_setup.notice().is_some());
}

#[test]
fn percent_scale_reaches_pdf_scale_and_rejects_garbage() {
    let mut app = app_with_sheet("A4");
    assert_eq!(preview(&app).pdf_options.scale, PdfScale::FitToPage);
    send(
        &mut app,
        PrintPreviewMsg::SetScaleMode(PdfScaleMode::Percent),
    );
    assert_eq!(preview(&app).pdf_options.scale, PdfScale::Percent(100.0));
    send(&mut app, PrintPreviewMsg::SetScalePercent("50".into()));
    assert_eq!(preview(&app).pdf_options.scale, PdfScale::Percent(50.0));
    for bad in ["", "abc", "-5", "0"] {
        send(&mut app, PrintPreviewMsg::SetScalePercent(bad.into()));
        assert_eq!(
            preview(&app).pdf_options.scale,
            PdfScale::Percent(50.0),
            "{bad:?}"
        );
        assert!(preview(&app).page_setup.notice().is_some(), "{bad:?}");
    }
    send(
        &mut app,
        PrintPreviewMsg::SetScaleMode(PdfScaleMode::OneToOne),
    );
    assert_eq!(preview(&app).pdf_options.scale, PdfScale::OneToOne);
}

#[test]
fn shrinking_the_page_refits_margins_and_says_so() {
    let mut app = app_with_sheet("A4");
    send(&mut app, PrintPreviewMsg::SetPageSize(PageSize::IsoA0));
    for edge in [MarginEdge::Left, MarginEdge::Right] {
        send(&mut app, PrintPreviewMsg::SetMargin(edge, "200".into()));
    }
    assert!(preview(&app).page_setup.notice().is_none());

    send(&mut app, PrintPreviewMsg::SetPageSize(PageSize::IsoA5));

    let m = preview(&app).pdf_options.margins;
    let (w, _) = PageSize::IsoA5.dimensions_mm(preview(&app).pdf_options.orientation);
    assert!(m.left_mm + m.right_mm < w, "{m:?}");
    let notice = preview(&app).page_setup.notice().expect("refit notice");
    assert!(notice.contains("reduced"), "{notice}");
    // The text boxes show the refitted values, not the old 200.
    assert_ne!(
        preview(&app).page_setup.margin_text(MarginEdge::Left),
        "200"
    );
}

#[test]
fn a_valid_edit_does_not_clear_another_fields_rejection() {
    let mut app = app_with_sheet("A4");
    send(
        &mut app,
        PrintPreviewMsg::SetMargin(MarginEdge::Left, "abc".into()),
    );
    send(
        &mut app,
        PrintPreviewMsg::SetMargin(MarginEdge::Right, "12".into()),
    );
    assert_eq!(preview(&app).pdf_options.margins.right_mm, 12.0);
    let notice = preview(&app).page_setup.notice().expect("still rejected");
    assert!(notice.contains("Left margin"), "{notice}");

    send(
        &mut app,
        PrintPreviewMsg::SetMargin(MarginEdge::Left, "15".into()),
    );
    assert!(preview(&app).page_setup.notice().is_none());
}

#[test]
fn custom_orientation_follows_the_page_shape() {
    let mut app = app_with_sheet("A4");
    send(
        &mut app,
        PrintPreviewMsg::SetPageSize(
            PageSizeKind::Custom.select(PageSize::IsoA4, Orientation::Landscape),
        ),
    );
    send(&mut app, PrintPreviewMsg::SetCustomWidth("100".into()));
    send(&mut app, PrintPreviewMsg::SetCustomHeight("200".into()));
    assert_eq!(preview(&app).pdf_options.orientation, Orientation::Portrait);

    // The picker is not offered for Custom; a stray message changes nothing.
    send(
        &mut app,
        PrintPreviewMsg::SetOrientation(Orientation::Landscape),
    );
    assert_eq!(preview(&app).pdf_options.orientation, Orientation::Portrait);
    assert_eq!(
        preview(&app).pdf_options.page_size,
        PageSize::Custom {
            width_mm: 100.0,
            height_mm: 200.0
        }
    );

    send(&mut app, PrintPreviewMsg::SetCustomWidth("300".into()));
    assert_eq!(
        preview(&app).pdf_options.orientation,
        Orientation::Landscape
    );
}

#[test]
fn custom_page_edges_have_a_lower_bound_so_margins_cannot_collapse() {
    let mut app = app_with_sheet("A4");
    send(
        &mut app,
        PrintPreviewMsg::SetPageSize(
            PageSizeKind::Custom.select(PageSize::IsoA4, Orientation::Landscape),
        ),
    );
    let before = preview(&app).pdf_options.page_size;
    for tiny in ["9", "1", "0.5"] {
        send(&mut app, PrintPreviewMsg::SetCustomWidth(tiny.into()));
        send(&mut app, PrintPreviewMsg::SetCustomHeight(tiny.into()));
        assert_eq!(preview(&app).pdf_options.page_size, before, "{tiny}");
    }
    assert!(preview(&app).page_setup.notice().is_some());

    send(&mut app, PrintPreviewMsg::SetCustomWidth("10".into()));
    send(&mut app, PrintPreviewMsg::SetCustomHeight("10".into()));
    let opts = &preview(&app).pdf_options;
    assert_eq!(
        opts.page_size,
        PageSize::Custom {
            width_mm: 10.0,
            height_mm: 10.0
        }
    );
    let m = opts.margins;
    assert!(
        m.left_mm + m.right_mm < 10.0 && m.top_mm + m.bottom_mm < 10.0,
        "{m:?}"
    );
}

fn logged(marker: &str) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(marker))
}

#[test]
fn automatic_changes_reach_the_messages_panel() {
    let _ = signex_app::diagnostics::init_logging();
    let mut app = app_with_sheet("A4");

    // Margin clamp.
    send(
        &mut app,
        PrintPreviewMsg::SetMargin(MarginEdge::Left, "5000".into()),
    );
    assert!(logged("Left margin limited to"));

    // Page-change refit.
    send(&mut app, PrintPreviewMsg::SetPageSize(PageSize::IsoA0));
    send(
        &mut app,
        PrintPreviewMsg::SetMargin(MarginEdge::Top, "300".into()),
    );
    send(&mut app, PrintPreviewMsg::SetPageSize(PageSize::IsoA5));
    assert!(logged("Margins were reduced to fit the new page size"));

    // Rejected input.
    send(
        &mut app,
        PrintPreviewMsg::SetMargin(MarginEdge::Bottom, "zzq".into()),
    );
    assert!(logged("Bottom margin"));

    // Percent fallback.
    send(
        &mut app,
        PrintPreviewMsg::SetScaleMode(PdfScaleMode::Percent),
    );
    send(&mut app, PrintPreviewMsg::SetScalePercent("qqx".into()));
    send(
        &mut app,
        PrintPreviewMsg::SetScaleMode(PdfScaleMode::OneToOne),
    );
    send(
        &mut app,
        PrintPreviewMsg::SetScaleMode(PdfScaleMode::Percent),
    );
    assert!(logged("Scale text \"qqx\" is not a valid percentage"));
    assert_eq!(preview(&app).pdf_options.scale, PdfScale::Percent(100.0));
    assert!(preview(&app).page_setup.notice().is_some());
}

#[test]
fn a_huge_custom_page_previews_within_the_pixel_cap() {
    let _ = signex_app::diagnostics::init_logging();
    let mut app = app_with_sheet("A4");
    send(
        &mut app,
        PrintPreviewMsg::SetPageSize(PageSize::Custom {
            width_mm: 100.0,
            height_mm: 100.0,
        }),
    );
    send(&mut app, PrintPreviewMsg::SetCustomWidth("5080".into()));
    send(&mut app, PrintPreviewMsg::SetCustomHeight("5080".into()));
    let page = &preview(&app).pages[0];
    assert!(
        page.width_px <= signex_output::MAX_PREVIEW_EDGE_PX,
        "{}x{}",
        page.width_px,
        page.height_px
    );
    assert!(page.height_px <= signex_output::MAX_PREVIEW_EDGE_PX);
    assert!(logged("reduced resolution"));
}

#[test]
fn a_portrait_suffixed_sheet_seeds_a_consistent_custom_orientation() {
    let app = app_with_sheet("Tabloid portrait");
    let o = preview(&app).pdf_options.clone();
    if let PageSize::Custom {
        width_mm,
        height_mm,
    } = o.page_size
    {
        let expect = if width_mm >= height_mm {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        };
        assert_eq!(o.orientation, expect);
    }
}
