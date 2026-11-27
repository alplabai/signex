//! #1116 (mockup row 204): the Place Image dialog on the GUI path (the
//! folder read lands, a thumbnail is chosen, the Width field, Place puts the
//! image on the cursor) and its view.

use super::super::tests::{app_in, empty_sheet, land_image_messages, png, write};
use super::{PlaceDialog, PlaceDialogMsg};
use crate::app::images::ImagesMsg;
use crate::app::{EditMsg, Message, Signex};

fn send(app: &mut Signex, msg: PlaceDialogMsg) -> iced::Task<Message> {
    app.update(Message::Edit(EditMsg::Images(ImagesMsg::Dialog(msg))))
}

/// An app whose project has two images, with the dialog open and its
/// folder read.
fn open_dialog(dir: &std::path::Path) -> Signex {
    write(&dir.join("assets/images/b-wide.png"), &png(400, 100));
    write(&dir.join("assets/images/a-logo.png"), &png(100, 100));
    let (mut app, _sheet) = app_in(dir, empty_sheet());
    let task = app.update(Message::Edit(EditMsg::Images(ImagesMsg::PlaceRequested)));
    assert!(
        app.ui_state.image_cache.dialog.is_some(),
        "the dialog opens"
    );
    land_image_messages(&mut app, task);
    app
}

fn dialog(app: &Signex) -> &PlaceDialog {
    app.ui_state.image_cache.dialog.as_deref().expect("open")
}

#[test]
fn the_dialog_lists_the_project_images_and_chooses_the_first_at_its_first_size() {
    let dir = tempfile::tempdir().unwrap();
    let app = open_dialog(dir.path());
    let d = dialog(&app);
    assert!(!d.loading);
    let names: Vec<&str> = d.files.iter().map(|f| f.name()).collect();
    assert_eq!(names, ["a-logo.png", "b-wide.png"]);
    assert_eq!(d.chosen, Some(0));
    assert!(d.files.iter().all(|f| !f.is_new && f.thumb.is_some()));
    // The free area of an empty A4 page, less the inset, on the 2.54 grid.
    let width: f64 = d.width.parse().expect("a width");
    assert!(width > 100.0 && (width / 2.54 - (width / 2.54).round()).abs() < 1e-6);
}

#[test]
fn place_puts_the_chosen_image_on_the_cursor_at_the_typed_width_and_closes() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = open_dialog(dir.path());
    let _ = send(&mut app, PlaceDialogMsg::Chosen(1));
    let _ = send(&mut app, PlaceDialogMsg::WidthTyped("40.64".into()));
    assert_eq!(dialog(&app).placed_width(), Some(40.64));

    let _ = send(&mut app, PlaceDialogMsg::PlacePressed);

    assert!(
        app.ui_state.image_cache.dialog.is_none(),
        "the dialog closes"
    );
    assert!(
        app.interaction_state.placement.is_some(),
        "the image is on the cursor"
    );
    let _ = app.update(Message::CanvasEvent(crate::canvas::CanvasEvent::Clicked {
        world_x: 101.6,
        world_y: 101.6,
    }));
    let placed = super::super::tests::images(&app);
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].file, "assets/images/b-wide.png");
    assert!((placed[0].width - 40.64).abs() < 1e-9);
    assert!((placed[0].height - 10.16).abs() < 1e-9);
}

#[test]
fn a_width_that_does_not_read_keeps_the_dialog_open_and_places_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = open_dialog(dir.path());
    let _ = send(&mut app, PlaceDialogMsg::WidthTyped("wide".into()));
    assert_eq!(dialog(&app).placed_width(), None);
    let _ = send(&mut app, PlaceDialogMsg::PlacePressed);
    assert!(app.ui_state.image_cache.dialog.is_some());
    assert!(app.interaction_state.placement.is_none());
    // The file's own size needs no width.
    let _ = send(&mut app, PlaceDialogMsg::FileSizePicked(true));
    assert!((dialog(&app).placed_width().unwrap() - 26.458_333).abs() < 1e-3);
}

#[test]
fn the_filter_narrows_the_thumbnails_and_cancel_closes() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = open_dialog(dir.path());
    let _ = send(&mut app, PlaceDialogMsg::FilterChanged("wide".into()));
    let shown: Vec<usize> = dialog(&app).shown().map(|(i, _)| i).collect();
    assert_eq!(shown, [1]);
    let _ = send(&mut app, PlaceDialogMsg::Closed);
    assert!(app.ui_state.image_cache.dialog.is_none());
}

#[test]
fn a_picked_file_from_outside_joins_the_list_as_new() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let picked = write(&outside.path().join("scope.png"), &png(200, 100));
    let mut app = open_dialog(dir.path());
    let task = app.update(Message::Edit(EditMsg::Images(ImagesMsg::FilePicked(Some(
        picked,
    )))));
    land_image_messages(&mut app, task);
    let d = dialog(&app);
    let chosen = &d.files[d.chosen.expect("chosen")];
    assert_eq!(chosen.file, "assets/images/scope.png");
    assert!(chosen.is_new, "copied on Save");
    assert!(
        app.interaction_state.placement.is_none(),
        "not on the cursor yet"
    );
}

#[test]
fn the_card_shows_the_title_the_thumbnails_and_the_facts() {
    let dir = tempfile::tempdir().unwrap();
    let app = open_dialog(dir.path());
    let tokens = &app.document_state.panel_ctx.tokens;
    let view = super::view(dialog(&app), tokens, &app.ui_state.i18n);
    let texts: Vec<String> =
        crate::layout_probe::lay_out_texts(view, iced::Size::new(1400.0, 900.0))
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
    for wanted in [
        "Place Image",
        "on main.snxsch",
        "a-logo.png",
        "b-wide.png",
        "assets/images/a-logo.png",
        "100 × 100 px · 1 KB",
        "Add image file…",
        "The sheet stores the path, not the picture.",
    ] {
        assert!(
            texts.iter().any(|t| t == wanted),
            "{wanted} is not shown: {texts:?}"
        );
    }
}
