//! #1116 (mockup row 204): Relink points every image naming a file at
//! another file, Replace one image, each as one undo step, on the headless
//! path (`relink_image`) and the GUI path (the Properties buttons' reads).

use std::sync::Arc;

use signex_types::image_asset::content_hash;
use signex_types::schematic::{Point, SchDrawing, SchImage};
use uuid::Uuid;

use super::tests::{answered, app_in, empty_sheet, images, png, refusal, run_id, undo, write};
use super::{ImagesMsg, RelinkRead, RelinkScope};
use crate::app::{EditMsg, Message};

/// A sheet with two images naming `old.png` and one naming `keep.png`.
fn sheet_with_three(old_hash: &str) -> signex_types::schematic::SchematicSheet {
    let mut sheet = empty_sheet();
    for (i, file) in ["old.png", "old.png", "keep.png"].iter().enumerate() {
        let mut image = SchImage::new(
            Uuid::from_u128(i as u128 + 1),
            Point::new(10.16 + 30.0 * i as f64, 10.16),
            (20.0, 10.0),
            format!("assets/images/{file}"),
            old_hash.to_string(),
        );
        image.source_px = Some([200, 100]);
        sheet.drawings.push(SchDrawing::Image(image));
    }
    sheet
}

fn files(app: &crate::app::Signex) -> Vec<String> {
    images(app).into_iter().map(|i| i.file).collect()
}

#[test]
fn relink_points_every_image_naming_the_file_at_the_new_one_as_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let new = write(&dir.path().join("assets/images/new.png"), &png(100, 100));
    let (mut app, _sheet) = app_in(dir.path(), sheet_with_three("aa"));
    let path = format!("path={}", new.display());

    let answer = answered(run_id(
        &mut app,
        "relink_image",
        &[&path, "file=assets/images/old.png"],
    ));

    assert_eq!(answer["relinked"], 2);
    assert_eq!(answer["file"], "assets/images/new.png");
    assert_eq!(answer["copied_on_save"], false);
    assert_eq!(
        files(&app),
        [
            "assets/images/new.png",
            "assets/images/new.png",
            "assets/images/keep.png"
        ]
    );
    let relinked = &images(&app)[0];
    assert_eq!(relinked.hash, content_hash(&png(100, 100)));
    assert_eq!(relinked.source_px, Some([100, 100]));
    assert!(
        (relinked.height - 20.0).abs() < 1e-9,
        "the height follows the new file's aspect"
    );
    undo(&mut app);
    assert_eq!(files(&app)[0], "assets/images/old.png", "one undo step");
    assert_eq!(files(&app)[1], "assets/images/old.png");
}

#[test]
fn replace_points_one_image_at_the_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let new = write(&dir.path().join("assets/images/new.png"), &png(200, 100));
    let (mut app, _sheet) = app_in(dir.path(), sheet_with_three("aa"));
    let path = format!("path={}", new.display());
    let id = format!("id={}", Uuid::from_u128(2));

    let answer = answered(run_id(&mut app, "relink_image", &[&path, &id]));

    assert_eq!(answer["relinked"], 1);
    assert_eq!(
        files(&app),
        [
            "assets/images/old.png",
            "assets/images/new.png",
            "assets/images/keep.png"
        ]
    );
}

#[test]
fn relink_refuses_both_or_neither_scope_and_a_file_no_image_names() {
    let dir = tempfile::tempdir().unwrap();
    let new = write(&dir.path().join("assets/images/new.png"), &png(10, 10));
    let (mut app, _sheet) = app_in(dir.path(), sheet_with_three("aa"));
    let path = format!("path={}", new.display());
    let id = format!("id={}", Uuid::from_u128(1));

    let both = refusal(run_id(
        &mut app,
        "relink_image",
        &[&path, "file=assets/images/old.png", &id],
    ));
    assert!(both.contains("not both"), "{both}");
    let neither = refusal(run_id(&mut app, "relink_image", &[&path]));
    assert!(neither.contains("needs `file`"), "{neither}");
    let none = refusal(run_id(
        &mut app,
        "relink_image",
        &[&path, "file=assets/images/nobody.png"],
    ));
    assert!(none.contains("no image names"), "{none}");
    let outside = refusal(run_id(&mut app, "relink_image", &[&path, "file=../x.png"]));
    assert!(outside.contains("leaves the project"), "{outside}");
    assert_eq!(files(&app)[0], "assets/images/old.png", "nothing changed");
}

#[test]
fn the_properties_relink_lands_its_read_and_relinks_on_the_sheet_in_view() {
    let dir = tempfile::tempdir().unwrap();
    let new = write(&dir.path().join("assets/images/new.png"), &png(100, 50));
    let (mut app, sheet) = app_in(dir.path(), sheet_with_three("aa"));
    let root = app.image_root(&sheet);
    let picked = super::read::read_picked_image(&new, &root, &[]).map(Arc::new);
    let revision = app
        .document_state
        .engines
        .get(&sheet)
        .map(signex_engine::Engine::revision);

    let _ = app.update(Message::Edit(EditMsg::Images(ImagesMsg::RelinkRead {
        scope: RelinkScope::File("assets/images/old.png".into()),
        target: sheet.clone(),
        revision,
        read: Box::new(RelinkRead {
            picked,
            project: None,
        }),
    })));

    assert_eq!(files(&app)[0], "assets/images/new.png");
    assert_eq!(files(&app)[1], "assets/images/new.png");
}

#[test]
fn a_relink_read_that_lands_after_an_edit_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let new = write(&dir.path().join("assets/images/new.png"), &png(100, 50));
    let (mut app, sheet) = app_in(dir.path(), sheet_with_three("aa"));
    let root = app.image_root(&sheet);
    let picked = super::read::read_picked_image(&new, &root, &[]).map(Arc::new);

    let _ = app.update(Message::Edit(EditMsg::Images(ImagesMsg::RelinkRead {
        scope: RelinkScope::File("assets/images/old.png".into()),
        target: sheet,
        revision: Some(u64::MAX),
        read: Box::new(RelinkRead {
            picked,
            project: None,
        }),
    })));

    assert_eq!(files(&app)[0], "assets/images/old.png");
}
