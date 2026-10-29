//! #353 — the PDF export modal exposes page size, orientation, margins and
//! scale. Drives the real message path (`PrintPreviewMsg::Requested` ->
//! `Set*`) against a real engine; the preview is re-rasterised on every
//! accepted edit, so the page bitmap's aspect ratio is asserted too.

use futures::StreamExt;
use iced::Task;
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
        lib_symbols: Default::default(),
        net_colors: HashMap::new(),
        drawing_placements: Default::default(),
        frame: Default::default(),
        unknown: Default::default(),
        font_family: None,
    };
    let path = PathBuf::from("page-setup.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _t) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "page-setup".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    send(&mut app, PrintPreviewMsg::Requested);
    app
}

/// Run a task to completion the way the iced runtime would, returning the
/// messages it produced.
fn run(task: Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

/// Feed every message `task` produces back through `update()`, following
/// the tasks those updates return, until nothing is left to run.
fn drive(app: &mut Signex, task: Task<Message>) {
    for message in run(task) {
        let follow_up = app.update(message);
        drive(app, follow_up);
    }
}

/// Send `msg` and drive the Task `update()` returns, as the runtime would.
fn send(app: &mut Signex, msg: PrintPreviewMsg) {
    let task = app.update(Message::PrintPreview(msg));
    drive(app, task);
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

#[test]
fn opening_the_modal_and_driving_its_task_fills_the_preview() {
    let mut app = app_with_sheet("A4");
    send(&mut app, PrintPreviewMsg::Close);
    assert!(app.document_state.preview.is_none());

    let task = app.update(Message::PrintPreview(PrintPreviewMsg::Requested));
    assert!(
        preview(&app).pages.is_empty(),
        "update() must not rasterise inline"
    );
    drive(&mut app, task);
    assert!(!preview(&app).pages.is_empty());
}

#[test]
fn update_does_not_rasterise_inline_and_discards_a_superseded_result() {
    let mut app = app_with_sheet("A4");
    let before_px = preview(&app).pages[0].width_px;

    // update() hands the render to a Task: the page bitmap is unchanged
    // until the Rasterized message comes back.
    let first = app.update(Message::PrintPreview(PrintPreviewMsg::SetPageSize(
        PageSize::IsoA0,
    )));
    assert_eq!(preview(&app).pages[0].width_px, before_px);
    let stale_generation = preview(&app).raster_generation;

    // A second edit supersedes the first request.
    let second = app.update(Message::PrintPreview(PrintPreviewMsg::SetPageSize(
        PageSize::IsoA5,
    )));
    assert_ne!(preview(&app).raster_generation, stale_generation);

    // The first request's real result arrives late and is dropped.
    drive(&mut app, first);
    assert_eq!(preview(&app).pages[0].width_px, before_px);

    // The current request's real result is applied.
    drive(&mut app, second);
    let (w, h) = PageSize::IsoA5.dimensions_mm(preview(&app).pdf_options.orientation);
    assert!((first_page_aspect(&app) - w / h).abs() < 0.02);
}

#[test]
fn page_setup_is_restored_when_the_modal_reopens_for_the_same_document() {
    let mut app = app_with_sheet("A4");
    send(&mut app, PrintPreviewMsg::SetPageSize(PageSize::IsoA3));
    send(
        &mut app,
        PrintPreviewMsg::SetMargin(MarginEdge::Left, "25".into()),
    );
    send(
        &mut app,
        PrintPreviewMsg::SetScaleMode(PdfScaleMode::OneToOne),
    );
    send(&mut app, PrintPreviewMsg::Close);
    assert!(app.document_state.preview.is_none());

    send(&mut app, PrintPreviewMsg::Requested);
    let opts = &preview(&app).pdf_options;
    assert_eq!(opts.page_size, PageSize::IsoA3);
    assert_eq!(opts.margins.left_mm, 25.0);
    assert_eq!(opts.scale, PdfScale::OneToOne);
    assert_eq!(preview(&app).page_setup.margin_text(MarginEdge::Left), "25");

    // The memory is keyed by document: a different path has no entry, so it
    // would open with its own sheet size.
    send(&mut app, PrintPreviewMsg::Close);
    let memory = &app.document_state.page_setup_memory;
    assert_eq!(memory.len(), 1);
    assert!(!memory.contains_key(&PathBuf::from("other.snxsch")));
}

/// The generation counter must not restart with each preview: a render left
/// over from a closed preview would otherwise match the reopened one's.
#[test]
fn a_late_result_from_a_closed_preview_cannot_land_on_the_reopened_one() {
    let mut app = app_with_sheet("A4");
    let stale_outline: Vec<signex_output::PreviewPageInfo> = preview(&app)
        .pages
        .iter()
        .map(|slot| signex_output::PreviewPageInfo {
            page_number: slot.page_number,
            sheet_count: slot.sheet_count,
            width_px: slot.width_px,
            height_px: slot.height_px,
        })
        .collect();
    assert!(!stale_outline.is_empty());
    let stale_pages = vec![(
        0,
        signex_output::PreviewPage {
            page_number: 1,
            sheet_count: 1,
            width_px: 1,
            height_px: 1,
            rgba: vec![0, 0, 0, 255],
        },
    )];
    let first_generation = preview(&app).raster_generation;
    send(&mut app, PrintPreviewMsg::Close);

    // Reopen, but hold back the new preview's own render.
    let reopen = app.update(Message::PrintPreview(PrintPreviewMsg::Requested));
    assert_ne!(preview(&app).raster_generation, first_generation);
    assert!(preview(&app).pages.is_empty());

    // The old session's result arrives first.
    let late = Message::PrintPreview(PrintPreviewMsg::Rasterized {
        generation: first_generation,
        outline: std::sync::Arc::new(stale_outline),
        pages: std::sync::Arc::new(stale_pages),
    });
    let follow_up = app.update(late);
    drive(&mut app, follow_up);
    assert!(
        preview(&app).pages.is_empty(),
        "a previous session's render was shown as this preview"
    );

    drive(&mut app, reopen);
    assert!(!preview(&app).pages.is_empty());
}

/// Closing the preview's OS window is the usual close path; it must keep the
/// page setup exactly like the modal's own Close.
#[test]
fn closing_the_os_window_remembers_the_page_setup() {
    use signex_app::app::WindowMsg;
    let mut app = app_with_sheet("A4");
    // Opening the preview registered its detached OS window.
    let window = *app
        .ui_state
        .windows
        .keys()
        .next()
        .expect("the preview opens as a detached window");
    send(&mut app, PrintPreviewMsg::SetPageSize(PageSize::IsoA3));

    let closed = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(window)));
    drop(run(closed));
    assert!(app.document_state.preview.is_none());

    send(&mut app, PrintPreviewMsg::Requested);
    assert_eq!(preview(&app).pdf_options.page_size, PageSize::IsoA3);
}

/// The setup is filed under the document the preview was opened for, not
/// whichever tab is active when it closes.
#[test]
fn page_setup_is_filed_under_the_document_the_preview_was_opened_for() {
    let mut app = app_with_sheet("A4");
    let opened_for = PathBuf::from("page-setup.snxsch");
    let other = PathBuf::from("other.snxsch");
    send(&mut app, PrintPreviewMsg::SetPageSize(PageSize::IsoA3));

    app.document_state.active_path = Some(other.clone());
    send(&mut app, PrintPreviewMsg::Close);

    let memory = &app.document_state.page_setup_memory;
    assert!(memory.contains_key(&opened_for));
    assert!(!memory.contains_key(&other));
}

/// Remember an A3 setup for the open sheet by closing its preview, as a user
/// does, and confirm the entry exists before the test closes the document.
fn remember_a3_setup(app: &mut Signex) {
    send(app, PrintPreviewMsg::SetPageSize(PageSize::IsoA3));
    send(app, PrintPreviewMsg::Close);
    assert_eq!(app.document_state.page_setup_memory.len(), 1);
}

/// #982 — the memory is session-only and per open document, so closing the
/// document's tab drops its entry.
#[test]
fn closing_the_tab_forgets_its_page_setup() {
    use signex_app::app::{ContextMenuMsg, TabContextAction};
    let mut app = app_with_sheet("A4");
    remember_a3_setup(&mut app);

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(0),
    )));

    assert!(app.document_state.tabs.is_empty());
    assert!(app.document_state.page_setup_memory.is_empty());
}

#[test]
fn close_all_forgets_the_page_setup() {
    use signex_app::app::{ContextMenuMsg, TabContextAction};
    let mut app = app_with_sheet("A4");
    remember_a3_setup(&mut app);

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::CloseAll,
    )));

    assert!(app.document_state.page_setup_memory.is_empty());
}

/// Closing a different tab must leave this document's setup alone.
#[test]
fn closing_another_tab_keeps_the_page_setup() {
    use signex_app::app::{ContextMenuMsg, TabContextAction};
    let mut app = app_with_sheet("A4");
    remember_a3_setup(&mut app);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "other".to_string(),
        path: PathBuf::from("other.snxsch"),
        cached_document: None,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(1),
    )));

    assert!(
        app.document_state
            .page_setup_memory
            .contains_key(&PathBuf::from("page-setup.snxsch"))
    );
}

/// Closing a tab while its preview is still open must not let the preview's
/// later dismissal file the setup back.
#[test]
fn a_preview_open_for_a_closed_tab_does_not_resurrect_its_setup() {
    use signex_app::app::{ContextMenuMsg, TabContextAction};
    let mut app = app_with_sheet("A4");
    send(&mut app, PrintPreviewMsg::SetPageSize(PageSize::IsoA3));

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(0),
    )));
    send(&mut app, PrintPreviewMsg::Close);

    assert!(app.document_state.page_setup_memory.is_empty());
}

#[test]
fn closing_the_project_forgets_its_documents_page_setup() {
    use signex_app::app::LoadedProject;
    use signex_types::project::{ProjectData, SheetEntry};
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let mut app = app_with_sheet("A4");
    remember_a3_setup(&mut app);

    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: tmp.path().join("P.snxprj"),
        data: ProjectData {
            description: None,
            name: "P".to_string(),
            dir: tmp.path().to_string_lossy().into_owned(),
            schematic_root: Some("page-setup.snxsch".to_string()),
            pcb_file: None,
            sheets: vec![SheetEntry {
                name: "page-setup.snxsch".to_string(),
                filename: "page-setup.snxsch".to_string(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            }],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.tabs[0].project_id = Some(id);

    crate::close_support::close_first_project(&mut app);

    assert!(app.document_state.projects.is_empty());
    assert!(app.document_state.tabs.is_empty());
    assert!(app.document_state.page_setup_memory.is_empty());
}

/// A dirty tab closed by the user is parked with its unsaved edits; reopening
/// it restores them, so its page setup must survive as well.
#[test]
fn a_dirty_tab_parked_on_close_keeps_its_page_setup() {
    use signex_app::app::{ContextMenuMsg, TabContextAction};
    let mut app = app_with_sheet("A4");
    remember_a3_setup(&mut app);
    app.document_state
        .mark_document_dirty(&PathBuf::from("page-setup.snxsch"));

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(0),
    )));

    assert!(app.document_state.tabs.is_empty());
    assert_eq!(app.document_state.page_setup_memory.len(), 1);
}

/// Two tabs can hold one path; the document is still open until both close.
#[test]
fn closing_one_of_two_tabs_on_a_path_keeps_the_page_setup() {
    use signex_app::app::{ContextMenuMsg, TabContextAction};
    let mut app = app_with_sheet("A4");
    remember_a3_setup(&mut app);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "page-setup twin".to_string(),
        path: PathBuf::from("page-setup.snxsch"),
        cached_document: None,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });

    let close = |app: &mut Signex| {
        let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
            TabContextAction::Close(1),
        )));
    };
    close(&mut app);
    assert_eq!(app.document_state.tabs.len(), 1);
    assert_eq!(app.document_state.page_setup_memory.len(), 1);

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(0),
    )));
    assert!(app.document_state.page_setup_memory.is_empty());
}
