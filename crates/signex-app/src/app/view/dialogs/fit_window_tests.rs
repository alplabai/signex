//! #1227 — a dialog's footer buttons stay inside a small logical window.
//!
//! At a 200 % OS scale a 1920 x 1080 display leaves 960 x 540 logical px.
//! The ERC and Annotate dialogs were hard-sized 1000 x 600 / 1100 x 760,
//! so `.clip(true)` cut the footer, which holds their buttons.

use crate::app::Signex;
use crate::layout_probe::{find, lay_out_texts};
use iced::Size;

const SMALL_WINDOW: Size = Size::new(960.0, 540.0);

fn app_at_small_window() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.window_size = (SMALL_WINDOW.width, SMALL_WINDOW.height);
    app
}

fn assert_labels_inside_window(element: iced::Element<'_, crate::app::Message>, labels: &[&str]) {
    let texts = lay_out_texts(element, SMALL_WINDOW);
    for label in labels {
        let b = find(&texts, label).bounds;
        assert!(
            b.x >= 0.0
                && b.y >= 0.0
                && b.x + b.width <= SMALL_WINDOW.width
                && b.y + b.height <= SMALL_WINDOW.height,
            "{label:?} falls outside {SMALL_WINDOW:?}: {b:?}"
        );
    }
}

#[test]
fn erc_footer_buttons_stay_inside_a_960_by_540_window() {
    let app = app_at_small_window();
    assert_labels_inside_window(
        app.view_erc_dialog(),
        &["Electrical Rules Check", "Run ERC", "Close"],
    );
}

#[test]
fn annotate_footer_buttons_stay_inside_a_960_by_540_window() {
    let app = app_at_small_window();
    assert_labels_inside_window(
        app.view_annotate_dialog(),
        &[
            "Annotate",
            "Lock",
            "Re-prefix to standard",
            "Reset All",
            "Reset & Renumber",
            "Accept Changes",
        ],
    );
}

#[test]
fn bom_and_export_modals_are_capped_to_the_window() {
    let app = app_at_small_window();
    for (w, h) in [app.bom_modal_size(), app.pdf_modal_size()] {
        assert!(
            w <= SMALL_WINDOW.width && h <= SMALL_WINDOW.height,
            "modal {w}x{h} exceeds {SMALL_WINDOW:?}"
        );
    }
}

#[test]
fn modals_keep_their_design_size_in_a_roomy_window() {
    assert_eq!(
        super::fit_modal_size((1920.0, 1080.0), (1000.0, 600.0)),
        (1000.0, 600.0)
    );
}

fn blank_preview() -> crate::app::state::PreviewState {
    use crate::app::state::{PageSetupInputs, PdfPreviewTab, PdfQuality, PreviewState};
    PreviewState {
        source: std::path::PathBuf::new(),
        pages: Vec::new(),
        raster_source: None,
        selected: 0,
        pdf_options: signex_output::PdfOptions::default(),
        page_setup: PageSetupInputs::from_options(&signex_output::PdfOptions::default()),
        specific_page_input: "1".to_string(),
        range_note: None,
        zoom: 1.0,
        active_tab: PdfPreviewTab::Preview,
        pan: (0.0, 0.0),
        panning: None,
        sheet_files: Vec::new(),
        selected_files: std::collections::HashSet::new(),
        variants: Vec::new(),
        quality: PdfQuality::Draft72,
        raster_generation: 0,
        source_path: None,
        export_status: None,
    }
}

/// A detached export window opens at an OS-logical size, which a high
/// interface scale shrinks: its footer must still sit inside it.
#[test]
fn detached_export_footer_stays_inside_a_small_window() {
    let mut app = app_at_small_window();
    app.document_state.preview = Some(blank_preview());
    assert_labels_inside_window(
        app.view_print_preview_body(),
        &["Export PDF", "No files selected for export"],
    );
}

/// The windows narrower than a card's design width plus both margins: the
/// card is centred on its fitted width, so it must be that wide too, or its
/// right edge (the footer buttons) leaves the window.
fn narrow_windows(design_w: f32) -> [f32; 4] {
    let margin = super::widgets::MODAL_WINDOW_MARGIN;
    [
        design_w,
        design_w + 10.0,
        design_w + margin,
        design_w + 2.0 * margin - 1.0,
    ]
}

fn assert_card_inside(
    app: &mut Signex,
    design_w: f32,
    view: impl for<'a> Fn(&'a Signex) -> iced::Element<'a, crate::app::Message>,
    labels: &[&str],
) {
    for width in narrow_windows(design_w) {
        let window = Size::new(width, 700.0);
        app.ui_state.window_size = (window.width, window.height);
        let texts = lay_out_texts(view(app), window);
        for label in labels {
            let b = find(&texts, label).bounds;
            assert!(
                b.x >= 0.0 && b.x + b.width <= window.width - super::widgets::MODAL_WINDOW_MARGIN,
                "{label:?} leaves a {width} px window: {b:?}"
            );
        }
    }
}

#[test]
fn the_remove_card_fits_a_window_just_wider_than_its_design() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.remove_dialog = Some(crate::app::RemoveDialogState {
        target_path: "p.snxsch".into(),
        tree_path: vec![0],
        project_id: None,
        display_name: "p".into(),
        is_outside_project: false,
        unsaved_prompt: false,
        is_directory_package: false,
    });
    assert_card_inside(
        &mut app,
        super::sizes::REMOVE.0,
        |app| app.view_remove_dialog(),
        &["Remove from Project", "Cancel"],
    );
}

#[test]
fn the_quit_confirm_card_fits_a_window_just_wider_than_its_design() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.app_quit_confirm = Some(crate::app::AppQuitConfirmState {
        dirty_paths: vec![std::path::PathBuf::from("a.snxsch")],
        dirty_rows: Vec::new(),
    });
    assert_card_inside(
        &mut app,
        super::sizes::APP_QUIT_CONFIRM.0,
        |app| app.view_app_quit_confirm(),
        &["Cancel", "Save All"],
    );
}

#[test]
fn the_autosave_recovery_card_fits_a_window_just_wider_than_its_design() {
    let (mut app, _boot) = Signex::new();
    let offer = crate::autosave::RecoveryOffer {
        path: "a.snxsch".into(),
        age_secs: 30,
        older_than_file: false,
    };
    assert_card_inside(
        &mut app,
        super::sizes::AUTOSAVE_RECOVERY.0,
        |app| app.view_autosave_recovery(&offer),
        &["Discard", "Restore"],
    );
}

#[test]
fn the_remove_unsaved_edits_card_fits_a_window_just_wider_than_its_design() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.remove_dialog = Some(crate::app::RemoveDialogState {
        target_path: "p.snxsch".into(),
        tree_path: vec![0],
        project_id: None,
        display_name: "p".into(),
        is_outside_project: false,
        unsaved_prompt: true,
        is_directory_package: false,
    });
    assert_card_inside(
        &mut app,
        super::sizes::REMOVE.0,
        |app| app.view_remove_dialog(),
        &["Cancel", "Save"],
    );
}

#[test]
fn the_create_sheet_card_fits_a_window_just_wider_than_its_design() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.create_child_sheet_confirm =
        Some(crate::app::contracts::CreateChildSheetConfirmState {
            symbol: uuid::Uuid::from_u128(1),
            name: "Power".to_string(),
            parent: "proj/top.snxsch".into(),
            project_dir: "proj".into(),
            path_text: "power.snxsch".to_string(),
            error: None,
            exists: false,
        });
    assert_card_inside(
        &mut app,
        super::sizes::CREATE_CHILD_SHEET.0,
        |app| app.view_create_child_sheet_confirm(),
        &["Later", "Create"],
    );
}
