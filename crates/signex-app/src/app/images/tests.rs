//! #1116 (mockup row 204): Place ▸ Graphic… on the GUI path (the picked
//! file is read off the UI thread, the image goes on the cursor and one
//! click places it) and the headless path (`place_graphic path= at=`), the
//! copy into the project on save, the placeholder for a missing or changed
//! file, and each refusal.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_types::image_asset::{ImageReadError, ImageRefusal, content_hash};
use signex_types::schematic::{Point, SchDrawing, SchImage, SchematicSheet};
use uuid::Uuid;

use super::{ImageEntry, ImageKey, ImagesMsg};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;

/// A PNG header of `w` × `h` pixels: enough to be checked and placed.
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend_from_slice(&w.to_be_bytes());
    bytes.extend_from_slice(&h.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="4" height="2"/></svg>"#;

fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

/// An app with a loose sheet `main.snxsch` in `dir`: its images are relative
/// to `dir`.
fn app_in(dir: &Path, sheet: SchematicSheet) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 2.54;
    let path = dir.join("main.snxsch");
    add_sheet_with(&mut app, &path, sheet);
    (app, path)
}

fn write(path: &Path, bytes: &[u8]) -> PathBuf {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    path.to_path_buf()
}

fn run(app: &mut Signex, operands: &[String]) -> CommandOutcome {
    let command = AppCommandId::new("place_graphic").expect("a catalog id");
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&command, &refs).expect("operands");
    app.run_operands_settled(&command, &args).0
}

fn place(app: &mut Signex, file: &Path, extra: &[&str]) -> CommandOutcome {
    let mut operands = vec![format!("path={}", file.display()), "at=50.8,50.8".into()];
    operands.extend(extra.iter().map(|s| s.to_string()));
    run(app, &operands)
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn images(app: &Signex) -> Vec<SchImage> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .drawings
        .iter()
        .filter_map(|d| match d {
            SchDrawing::Image(image) => Some(image.clone()),
            _ => None,
        })
        .collect()
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

/// Run `task` and feed back only the image messages it yields.
fn land_image_messages(app: &mut Signex, task: iced::Task<Message>) {
    for message in crate::persist::run_task::outputs(task) {
        if matches!(message, Message::Edit(EditMsg::Images(_))) {
            let _ = app.update(message);
        }
    }
}

#[test]
fn headless_place_references_the_file_and_the_save_copies_it_into_the_project() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let bytes = png(96, 48);
    let picked = write(&outside.path().join("Logo Mark.png"), &bytes);
    let (mut app, sheet) = app_in(project.path(), empty_sheet());

    let answer = answered(place(&mut app, &picked, &["width=25.4"]));

    assert_eq!(answer["placed"], 1);
    assert_eq!(answer["file"], "assets/images/Logo-Mark.png");
    assert_eq!(answer["hash"], content_hash(&bytes));
    assert_eq!(answer["format"], "png");
    assert_eq!(answer["copied_on_save"], true);
    let [image]: [SchImage; 1] = images(&app).try_into().expect("one image");
    assert_eq!(image.file, "assets/images/Logo-Mark.png");
    assert!((image.width - 25.4).abs() < 1e-9 && (image.height - 12.7).abs() < 1e-9);
    assert_eq!(image.source_px, Some([96, 48]));
    let target = project.path().join("assets/images/Logo-Mark.png");
    assert!(!target.exists(), "nothing is written before the save");

    let copies = app.take_image_copies(&sheet);
    assert_eq!(copies.len(), 1);
    let results = super::copy::write_image_copies(copies);
    assert!(
        results.iter().all(|(_, result)| result.is_ok()),
        "{results:?}"
    );
    assert_eq!(std::fs::read(&target).unwrap(), bytes);
    app.image_copies_written(results);
    assert!(app.document_state.image_copies.is_empty());

    undo(&mut app);
    assert!(images(&app).is_empty(), "one undo step");
}

#[test]
fn the_same_picture_picked_twice_shares_one_asset() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let first = write(&outside.path().join("a/logo.png"), &png(10, 10));
    let second = write(&outside.path().join("b/copy.png"), &png(10, 10));
    let (mut app, _sheet) = app_in(project.path(), empty_sheet());

    answered(place(&mut app, &first, &[]));
    answered(place(&mut app, &second, &[]));

    let placed = images(&app);
    assert_eq!(placed.len(), 2);
    assert!(placed.iter().all(|i| i.file == "assets/images/logo.png"));
    assert_eq!(app.document_state.image_copies.len(), 1);
}

#[test]
fn a_file_already_in_the_project_is_placed_where_it_is() {
    let project = tempfile::tempdir().unwrap();
    let svg = write(
        &project.path().join("assets/images/mark.svg"),
        SVG.as_bytes(),
    );
    let (mut app, _sheet) = app_in(project.path(), empty_sheet());

    let answer = answered(place(&mut app, &svg, &[]));

    assert_eq!(answer["file"], "assets/images/mark.svg");
    assert_eq!(answer["format"], "svg");
    assert_eq!(answer["copied_on_save"], false);
    assert!(app.document_state.image_copies.is_empty());
}

#[test]
fn a_name_taken_by_other_content_is_not_replaced() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(&project.path().join("assets/images/logo.png"), &png(1, 1));
    let picked = write(&outside.path().join("logo.png"), &png(2, 2));
    let (mut app, _sheet) = app_in(project.path(), empty_sheet());

    let answer = answered(place(&mut app, &picked, &[]));

    assert_ne!(answer["file"], "assets/images/logo.png");
    assert_eq!(
        std::fs::read(project.path().join("assets/images/logo.png")).unwrap(),
        png(1, 1)
    );
}

#[test]
fn an_oversized_file_is_refused_and_places_nothing() {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().join("huge.png");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(signex_types::image_asset::MAX_IMAGE_BYTES + 1)
        .unwrap();
    let (mut app, _sheet) = app_in(project.path(), empty_sheet());

    let reason = refusal(place(&mut app, &path, &[]));

    assert!(reason.contains("an image may be up to"), "{reason}");
    assert!(images(&app).is_empty());
}

#[test]
fn an_svg_with_an_external_href_is_refused() {
    let project = tempfile::tempdir().unwrap();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><image href="https://example.com/x.png"/></svg>"#;
    let path = write(&project.path().join("remote.svg"), svg.as_bytes());
    let (mut app, _sheet) = app_in(project.path(), empty_sheet());

    let reason = refusal(place(&mut app, &path, &[]));

    assert!(reason.contains("outside itself"), "{reason}");
    assert!(images(&app).is_empty());
}

#[test]
fn a_file_that_is_not_an_image_is_refused() {
    let project = tempfile::tempdir().unwrap();
    let path = write(&project.path().join("notes.png"), b"hello");
    let (mut app, _sheet) = app_in(project.path(), empty_sheet());

    let reason = refusal(place(&mut app, &path, &[]));

    assert!(reason.contains("not a PNG"), "{reason}");
}

#[test]
fn a_bad_width_is_refused() {
    let project = tempfile::tempdir().unwrap();
    let path = write(&project.path().join("a.png"), &png(4, 4));
    let (mut app, _sheet) = app_in(project.path(), empty_sheet());

    let reason = refusal(place(&mut app, &path, &["width=-3"]));

    assert!(reason.contains("above zero"), "{reason}");
}

/// A sheet from an untrusted project may name any path: an absolute one or
/// one that steps out is refused before anything is read.
#[test]
fn a_placed_image_path_outside_the_project_is_never_read() {
    let project = tempfile::tempdir().unwrap();
    assert!(matches!(
        super::read::read_placed_image(project.path(), "/etc/passwd", "x"),
        Err(ImageReadError::Refused(ImageRefusal::AbsolutePath(_)))
    ));
    assert!(matches!(
        super::read::read_placed_image(project.path(), "../secret.png", "x"),
        Err(ImageReadError::Refused(ImageRefusal::LeavesProject(_)))
    ));
}

fn sheet_with_image(file: &str, hash: &str) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.drawings.push(SchDrawing::Image(SchImage::new(
        Uuid::now_v7(),
        Point::new(10.16, 10.16),
        (25.4, 12.7),
        file.into(),
        hash.into(),
    )));
    sheet
}

#[test]
fn a_missing_or_changed_file_draws_the_placeholder_and_is_said_once() {
    let project = tempfile::tempdir().unwrap();
    write(
        &project.path().join("assets/images/changed.png"),
        &png(3, 3),
    );
    let mut sheet = sheet_with_image("assets/images/gone.png", "ab");
    let changed = SchImage::new(
        Uuid::now_v7(),
        Point::new(50.8, 10.16),
        (25.4, 25.4),
        "assets/images/changed.png".into(),
        content_hash(&png(4, 4)),
    );
    sheet.drawings.push(SchDrawing::Image(changed.clone()));
    let (mut app, path) = app_in(project.path(), sheet);

    let task = app.request_image_loads();
    land_image_messages(&mut app, task);

    let cache = &app.ui_state.image_cache;
    let gone = ImageKey {
        file: "assets/images/gone.png".into(),
        hash: "ab".into(),
    };
    assert!(matches!(cache.entry(&gone), Some(ImageEntry::Missing)));
    assert!(matches!(
        cache.entry(&ImageKey::of(&changed)),
        Some(ImageEntry::Missing)
    ));
    assert!(cache.has_warned(&path));
    // The sheet keeps both references.
    assert_eq!(images(&app).len(), 2);
}

#[test]
fn a_present_file_is_read_off_the_ui_thread_into_the_cache() {
    let project = tempfile::tempdir().unwrap();
    let bytes = png(5, 5);
    write(&project.path().join("assets/images/ok.png"), &bytes);
    let (mut app, path) = app_in(
        project.path(),
        sheet_with_image("assets/images/ok.png", &content_hash(&bytes)),
    );

    let task = app.request_image_loads();
    let key = ImageKey {
        file: "assets/images/ok.png".into(),
        hash: content_hash(&bytes),
    };
    assert!(matches!(
        app.ui_state.image_cache.entry(&key),
        Some(ImageEntry::Loading)
    ));
    land_image_messages(&mut app, task);

    assert!(matches!(
        app.ui_state.image_cache.entry(&key),
        Some(ImageEntry::Raster(_))
    ));
    assert!(!app.ui_state.image_cache.has_warned(&path));
}

#[test]
fn gui_pick_puts_the_image_on_the_cursor_and_one_click_places_it() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let picked = write(&outside.path().join("scope.png"), &png(200, 100));
    let (mut app, sheet) = app_in(project.path(), empty_sheet());

    let task = app.update(Message::Edit(EditMsg::Images(ImagesMsg::FilePicked(Some(
        picked,
    )))));
    land_image_messages(&mut app, task);
    assert!(
        app.interaction_state.placement.is_some(),
        "the image is on the cursor"
    );
    assert_eq!(app.document_state.image_copies.len(), 1);

    let _ = app.update(Message::CanvasEvent(crate::canvas::CanvasEvent::Clicked {
        world_x: 101.6,
        world_y: 101.6,
    }));
    let [image]: [SchImage; 1] = images(&app).try_into().expect("one image");
    assert_eq!(image.file, "assets/images/scope.png");
    // The first size fills the empty page's free area, keeping the aspect.
    assert!((image.width / image.height - 2.0).abs() < 1e-9);
    assert_eq!(app.take_image_copies(&sheet).len(), 1);

    undo(&mut app);
    assert!(images(&app).is_empty());
}

#[test]
fn a_gui_read_that_lands_after_the_tool_changed_is_dropped() {
    let project = tempfile::tempdir().unwrap();
    let picked = write(&project.path().join("a.png"), &png(4, 4));
    let (mut app, sheet) = app_in(project.path(), empty_sheet());
    let root = app.image_root(&sheet);
    let read = super::read::read_picked_image(&picked, &root, &[]).map(std::sync::Arc::new);
    let stale = app.interaction_state.placement_epoch;
    app.interaction_state.placement_epoch += 1;

    let _ = app.update(Message::Edit(EditMsg::Images(ImagesMsg::FileRead {
        path: picked,
        target: sheet,
        epoch: stale,
        read,
    })));

    assert!(app.interaction_state.placement.is_none());
}

#[test]
fn place_graphic_is_no_longer_a_planned_tool() {
    let action = crate::active_bar::ActiveBarAction::PlaceGraphic;
    assert!(!crate::active_bar::is_planned(&action));
}
