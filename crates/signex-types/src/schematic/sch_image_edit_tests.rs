//! #1116: the edits of a placed image and its resize handles.

use uuid::Uuid;

use super::*;

/// A 1200 × 600 px file shown 100 × 50 mm at (10, 20).
fn image() -> SchImage {
    let mut image = SchImage::new(
        Uuid::from_u128(7),
        Point::new(10.0, 20.0),
        (100.0, 50.0),
        "assets/images/plot.png".into(),
        "aa".into(),
    );
    image.source_px = Some([1200, 600]);
    image
}

fn edited(edit: ImageEdit) -> SchImage {
    image()
        .with_edit(&edit)
        .expect("the edit holds")
        .expect("the edit changes the image")
}

#[test]
fn width_and_height_keep_the_aspect_until_it_is_switched_off() {
    let wider = edited(ImageEdit::Width(200.0));
    assert_eq!((wider.width, wider.height), (200.0, 100.0));
    let taller = edited(ImageEdit::Height(25.0));
    assert_eq!((taller.width, taller.height), (50.0, 25.0));
    let free = image()
        .with_edit(&ImageEdit::KeepAspect(false))
        .unwrap()
        .unwrap();
    let stretched = free.with_edit(&ImageEdit::Width(80.0)).unwrap().unwrap();
    assert_eq!((stretched.width, stretched.height), (80.0, 50.0));
}

#[test]
fn scale_is_a_share_of_the_files_own_size_and_100_resets_it() {
    // 1200 px at 96 dpi is 317.5 mm.
    let reset = edited(ImageEdit::Scale(100.0));
    assert!((reset.width - 317.5).abs() < 1e-9);
    assert!((reset.height - 158.75).abs() < 1e-9);
    assert!((reset.scale_percent().unwrap() - 100.0).abs() < 1e-9);
    let half = edited(ImageEdit::Scale(50.0));
    assert!((half.width - 158.75).abs() < 1e-9);
    let mut unknown = image();
    unknown.source_px = None;
    assert_eq!(
        unknown.with_edit(&ImageEdit::Scale(100.0)),
        Err(ImageEditRefusal::NoFileSize)
    );
}

#[test]
fn a_size_out_of_range_or_a_bad_value_is_refused_and_an_unchanged_one_is_none() {
    let img = image();
    for bad in [
        ImageEdit::Width(0.0),
        ImageEdit::Width(f64::NAN),
        ImageEdit::Height(-3.0),
        ImageEdit::Width(5000.0),
    ] {
        assert_eq!(
            img.with_edit(&bad),
            Err(ImageEditRefusal::BadLength),
            "{bad:?}"
        );
    }
    assert_eq!(
        img.with_edit(&ImageEdit::Scale(0.0)),
        Err(ImageEditRefusal::BadScale)
    );
    assert_eq!(
        img.with_edit(&ImageEdit::Opacity(101)),
        Err(ImageEditRefusal::BadOpacity)
    );
    assert_eq!(img.with_edit(&ImageEdit::Width(100.0)), Ok(None));
    assert_eq!(img.with_edit(&ImageEdit::Border(false)), Ok(None));
}

#[test]
fn opacity_border_and_bounds_are_stored() {
    assert_eq!(edited(ImageEdit::Opacity(40)).opacity, 40);
    assert!(edited(ImageEdit::Border(true)).border);
    let moved = edited(ImageEdit::Bounds {
        position: Point::new(0.0, 5.0),
        size: (30.0, 40.0),
    });
    assert_eq!(moved.position, Point::new(0.0, 5.0));
    assert_eq!((moved.width, moved.height), (30.0, 40.0));
}

#[test]
fn a_relinked_image_keeps_its_place_and_width_and_takes_the_new_aspect() {
    let relinked = image().relinked("assets/images/b.png", "bb", [400, 400]);
    assert_eq!(relinked.file, "assets/images/b.png");
    assert_eq!(relinked.hash, "bb");
    assert_eq!(relinked.source_px, Some([400, 400]));
    assert_eq!(relinked.position, Point::new(10.0, 20.0));
    assert_eq!((relinked.width, relinked.height), (100.0, 100.0));
    let mut free = image();
    free.keep_aspect = false;
    let kept = free.relinked("assets/images/b.png", "bb", [400, 400]);
    assert_eq!((kept.width, kept.height), (100.0, 50.0));
}

#[test]
fn a_corner_keeps_the_aspect_and_its_opposite_corner_stays() {
    let at = Point::new(10.0, 20.0);
    let (pos, size) = resized_by_handle(
        at,
        (100.0, 50.0),
        ImageHandle::BottomRight,
        (20.0, 0.0),
        true,
    );
    assert_eq!(pos, at);
    assert_eq!(size, (120.0, 60.0));
    let (pos, size) =
        resized_by_handle(at, (100.0, 50.0), ImageHandle::TopLeft, (10.0, 10.0), true);
    // The larger share wins: the width keeps 90 % and the height follows.
    assert_eq!(size, (90.0, 45.0));
    assert_eq!(pos, Point::new(20.0, 25.0));
}

#[test]
fn a_side_handle_stretches_only_without_the_aspect() {
    let at = Point::new(0.0, 0.0);
    let kept = resized_by_handle(at, (100.0, 50.0), ImageHandle::Right, (30.0, 0.0), true);
    assert_eq!(kept, (at, (100.0, 50.0)));
    let (pos, size) = resized_by_handle(at, (100.0, 50.0), ImageHandle::Left, (30.0, 9.0), false);
    assert_eq!(pos, Point::new(30.0, 0.0));
    assert_eq!(size, (70.0, 50.0));
    let (_, flat) = resized_by_handle(at, (100.0, 50.0), ImageHandle::Top, (0.0, 80.0), false);
    assert_eq!(flat.1, 0.1);
}

#[test]
fn every_handle_sits_on_the_box() {
    let at = Point::new(0.0, 0.0);
    let spots: Vec<Point> = ImageHandle::ALL
        .iter()
        .map(|h| h.at(at, (10.0, 4.0)))
        .collect();
    assert_eq!(
        spots,
        [
            Point::new(0.0, 0.0),
            Point::new(5.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 2.0),
            Point::new(10.0, 4.0),
            Point::new(5.0, 4.0),
            Point::new(0.0, 4.0),
            Point::new(0.0, 2.0),
        ]
    );
    assert_eq!(ImageHandle::ALL.iter().filter(|h| h.is_corner()).count(), 4);
}
