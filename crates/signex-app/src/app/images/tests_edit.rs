//! #1116 (mockup row 204): an image's fields on the headless path
//! (`set_image`) and the GUI path (Properties ▸ Image, the resize handles),
//! each one edit and one undo step, and Show in folder.

use signex_types::schematic::{DrawingPlacement, Point, SelectedItem, SelectedKind};

use super::ImagesMsg;
use super::tests::{
    answered, app_in, empty_sheet, images, place, png, refusal, run_id, undo, write,
};
use crate::app::{EditMsg, Message, Signex};
use crate::panels::{AnnotationPanelMsg, ImagePanelField, PanelMsg};

/// An app with one 200 × 100 px image placed 50.8 mm wide; its id.
fn app_with_image(dir: &std::path::Path) -> (Signex, String) {
    let file = write(&dir.join("assets/images/plot.png"), &png(200, 100));
    let (mut app, _sheet) = app_in(dir, empty_sheet());
    let answer = answered(place(&mut app, &file, &["width=50.8"]));
    (app, answer["id"].as_str().expect("an id").to_string())
}

fn set(
    app: &mut Signex,
    id: &str,
    field: &str,
    value: &str,
) -> crate::app::command::CommandOutcome {
    let id = format!("id={id}");
    let field = format!("field={field}");
    let value = format!("value={value}");
    run_id(app, "set_image", &[&id, &field, &value])
}

fn select_image(app: &mut Signex) {
    let id = images(app)[0].uuid;
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Drawing)];
    app.update_selection_info();
    app.refresh_panel_ctx();
}

fn panel(app: &mut Signex, msg: super::ImagePanelMsg) {
    let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::Annotation(AnnotationPanelMsg::Image(msg)),
    )));
}

#[test]
fn set_image_sets_one_field_as_one_undo_step_and_answers_the_image() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_image(dir.path());

    let answer = answered(set(&mut app, &id, "width", "25.4"));
    assert_eq!(answer["size"][0], 25.4);
    let height = answer["size"][1].as_f64().unwrap();
    assert!((height - 12.7).abs() < 1e-9, "the height follows: {height}");
    undo(&mut app);
    assert!((images(&app)[0].width - 50.8).abs() < 1e-9, "one undo step");

    let answer = answered(set(&mut app, &id, "scale", "file"));
    // 200 px at 96 dpi is 52.916… mm: Reset to file size.
    assert!((answer["scale"].as_f64().unwrap() - 100.0).abs() < 1e-9);

    answered(set(&mut app, &id, "keep_aspect", "false"));
    let answer = answered(set(&mut app, &id, "height", "40"));
    assert_eq!(answer["size"][1], 40.0);
    assert!((answer["size"][0].as_f64().unwrap() - 52.916_666).abs() < 1e-3);

    let answer = answered(set(&mut app, &id, "opacity", "40"));
    assert_eq!(answer["opacity"], 40);
    let answer = answered(set(&mut app, &id, "border", "true"));
    assert_eq!(answer["border"], true);
    let answer = answered(set(&mut app, &id, "placement", "above"));
    assert_eq!(answer["placement"], "above");
    let sheet = app.document_state.active_engine().unwrap().document();
    assert_eq!(
        sheet.drawing_placement(images(&app)[0].uuid),
        DrawingPlacement::Above
    );
    let answer = answered(set(&mut app, &id, "bounds", "10.16,20.32,30,15"));
    assert_eq!(answer["position"][0], 10.16);
    assert_eq!(answer["size"][0], 30.0);
}

#[test]
fn set_image_refuses_a_bad_field_value_or_id() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_image(dir.path());
    assert!(refusal(set(&mut app, &id, "colour", "red")).contains("`field` is one of"));
    assert!(refusal(set(&mut app, &id, "width", "0")).contains("from 0.1 mm"));
    assert!(refusal(set(&mut app, &id, "opacity", "120")).contains("0 to 100"));
    assert!(refusal(set(&mut app, &id, "placement", "middle")).contains("behind or above"));
    let other = uuid::Uuid::now_v7().to_string();
    assert!(refusal(set(&mut app, &other, "width", "10")).contains("no image"));
    assert!(
        (images(&app)[0].width - 50.8).abs() < 1e-9,
        "nothing changed"
    );
}

#[test]
fn the_properties_rows_edit_the_selected_image_on_enter() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _id) = app_with_image(dir.path());
    select_image(&mut app);

    panel(
        &mut app,
        super::ImagePanelMsg::FieldTyped(ImagePanelField::Width, "30".into()),
    );
    assert!(
        (images(&app)[0].width - 50.8).abs() < 1e-9,
        "a keystroke is a draft"
    );
    panel(
        &mut app,
        super::ImagePanelMsg::FieldSubmitted(ImagePanelField::Width),
    );
    let image = &images(&app)[0];
    assert!((image.width - 30.0).abs() < 1e-9);
    assert!((image.height - 15.0).abs() < 1e-9);

    panel(&mut app, super::ImagePanelMsg::KeepAspectToggled(false));
    assert!(!images(&app)[0].keep_aspect);
    panel(&mut app, super::ImagePanelMsg::BorderPicked(true));
    assert!(images(&app)[0].border);
    panel(&mut app, super::ImagePanelMsg::ResetToFileSizePressed);
    assert!((images(&app)[0].scale_percent().unwrap() - 100.0).abs() < 1e-9);
    panel(
        &mut app,
        super::ImagePanelMsg::FieldTyped(ImagePanelField::Opacity, "25".into()),
    );
    panel(
        &mut app,
        super::ImagePanelMsg::FieldSubmitted(ImagePanelField::Opacity),
    );
    assert_eq!(images(&app)[0].opacity, 25);

    panel(&mut app, super::ImagePanelMsg::RemovePressed);
    assert!(images(&app).is_empty(), "Remove image deletes it");
    undo(&mut app);
    assert_eq!(images(&app).len(), 1, "as one undo step");
}

#[test]
fn a_handle_drop_is_one_edit_of_the_box() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _id) = app_with_image(dir.path());
    let id = images(&app)[0].uuid;

    let _ = app.update(Message::Edit(EditMsg::Images(ImagesMsg::HandleDropped {
        id,
        position: Point::new(12.7, 12.7),
        size: (60.96, 30.48),
    })));

    let image = &images(&app)[0];
    assert_eq!(image.position, Point::new(12.7, 12.7));
    assert_eq!((image.width, image.height), (60.96, 30.48));
    undo(&mut app);
    assert!((images(&app)[0].width - 50.8).abs() < 1e-9);
}

#[test]
fn show_image_in_folder_answers_the_paths_and_a_headless_run_launches_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_image(dir.path());
    app.ui_state.io.opener = crate::io_ports::Opener::Refuse;
    let operand = format!("id={id}");

    let answer = answered(run_id(&mut app, "show_image_in_folder", &[&operand]));

    assert_eq!(
        answer["path"],
        dir.path()
            .join("assets/images/plot.png")
            .display()
            .to_string()
    );
    assert_eq!(
        answer["folder"],
        dir.path().join("assets/images").display().to_string()
    );
    assert_eq!(answer["shown"], false);
}
