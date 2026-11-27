//! #1116 (mockup row 204): Properties ▸ Image as it lays out: the file with
//! Replace… and Show in folder, Size, Appearance, and for a missing file the
//! banner with Relink… and Remove image.

use signex_types::schematic::{
    Point, SchDrawing, SchImage, SchematicSheet, SelectedItem, SelectedKind,
};
use uuid::Uuid;

use crate::app::{Signex, TabInfo, TabKind};

fn app_with(image: SchImage) -> Signex {
    let (mut app, _boot) = Signex::new();
    let mut sheet = SchematicSheet {
        paper_size: "A4".into(),
        ..Default::default()
    };
    let id = image.uuid;
    sheet.drawings.push(SchDrawing::Image(image));
    let dir = std::env::temp_dir().join("signex-image-panel-tests");
    let path = dir.join("power.snxsch");
    app.document_state.tabs.push(TabInfo {
        title: path.display().to_string(),
        path,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.apply_loaded_schematic(Some(sheet), false, false, false);
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Drawing)];
    app.update_selection_info();
    app.refresh_panel_ctx();
    app
}

fn image() -> SchImage {
    let mut image = SchImage::new(
        Uuid::from_u128(44),
        Point::new(10.16, 94.49),
        (96.52, 46.97),
        "assets/images/power-path.svg".into(),
        "aa".into(),
    );
    image.source_px = Some([1200, 584]);
    image
}

fn shown(app: &Signex) -> Vec<String> {
    let ctx = app.document_view.panel_ctx.clone();
    let view = super::view_selected_element_properties(
        &ctx,
        iced::Color::WHITE,
        iced::Color::WHITE,
        iced::Color::BLACK,
    );
    crate::layout_probe::lay_out_texts(view, iced::Size::new(400.0, 4000.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn an_image_shows_its_file_size_and_appearance_rows() {
    let app = app_with(image());
    let texts = shown(&app);
    for wanted in [
        "power-path.svg",
        "assets/images/power-path.svg",
        "Replace…",
        "Show in folder",
        "Keep aspect ratio",
        "Reset to file size",
        "Behind circuit",
        "Line",
    ] {
        assert!(
            texts.iter().any(|t| t == wanted),
            "{wanted} is not shown: {texts:?}"
        );
    }
    assert!(!texts.iter().any(|t| t == "File not found."));
    assert!(!texts.iter().any(|t| t == "Relink…"));
}

#[test]
fn a_missing_file_shows_the_banner_with_relink_and_remove() {
    let mut app = app_with(image());
    app.document_view.panel_ctx.annotation.image.missing = true;
    let texts = shown(&app);
    for wanted in [
        "File not found.",
        "Relink…",
        "Remove image",
        "not found · last 1200 × 584 px",
    ] {
        assert!(
            texts.iter().any(|t| t == wanted),
            "{wanted} is not shown: {texts:?}"
        );
    }
    assert!(!texts.iter().any(|t| t == "Show in folder"));
}

#[test]
fn the_panel_state_says_missing_when_the_canvas_could_not_read_the_file() {
    let mut app = app_with(image());
    let _ = app.request_image_loads();
    assert!(!app.image_panel_state().missing, "still being read");
    let sheet = app.document_state.active_path.clone().unwrap();

    let _ = app.update(crate::app::Message::Edit(crate::app::EditMsg::Images(
        crate::app::ImagesMsg::PictureRead {
            key: crate::app::images::ImageKey::of(&image()),
            sheet,
            read: Err(signex_types::image_asset::ImageReadError::Changed),
        },
    )));

    assert!(app.image_panel_state().missing);
    app.refresh_panel_ctx();
    let texts = shown(&app);
    assert!(texts.iter().any(|t| t == "File not found."), "{texts:?}");
}
