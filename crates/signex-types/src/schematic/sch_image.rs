//! An image placed on a sheet (#1116, mockup row 204): a reference to a
//! file under the project's `assets/images/`, never the picture itself. The
//! sheet keeps the image's place and size whether or not the file is there;
//! a missing file draws a placeholder. See docs/SCHEMATIC_IMAGES.md.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Point, SchematicSheet, SheetSize};
use crate::format::UnknownFields;

/// Space kept between a first size and the free area's edges: two steps of
/// the 2.54 mm grid.
pub const IMAGE_FIRST_SIZE_INSET_MM: f64 = 5.08;

/// The grid a first width is rounded down to.
pub const IMAGE_FIRST_SIZE_GRID_MM: f64 = 2.54;

/// Narrowest free area a first size fills; with less room an image takes
/// its own size.
pub const IMAGE_FIRST_SIZE_MIN_MM: f64 = 10.16;

/// Smallest edge an image is sized to.
pub const IMAGE_MIN_EDGE_MM: f64 = 0.1;

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn full() -> u8 {
    100
}

fn is_full(value: &u8) -> bool {
    *value == 100
}

/// One placed image; `position` is its top-left corner. An image never
/// turns or flips: a rotate or mirror of the selection moves its centre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchImage {
    pub uuid: Uuid,
    pub position: Point,
    pub width: f64,
    pub height: f64,
    /// The file, relative to the project root (`assets/images/logo.svg`).
    pub file: String,
    /// `image_asset::content_hash` of the file as placed: a file that no
    /// longer matches draws the placeholder, as a missing one does.
    pub hash: String,
    /// The file's own size in pixels, so Reset to file size and the
    /// Properties facts work while the file is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_px: Option<[u32; 2]>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub keep_aspect: bool,
    /// Percent, 0 to 100.
    #[serde(default = "full", skip_serializing_if = "is_full")]
    pub opacity: u8,
    /// A line drawn around the image.
    #[serde(default, skip_serializing_if = "is_false")]
    pub border: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
    /// Keys of the image's row this build does not know, written back.
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl SchImage {
    /// An image of `size` (mm) with its top-left corner at `position`.
    pub fn new(uuid: Uuid, position: Point, size: (f64, f64), file: String, hash: String) -> Self {
        Self {
            uuid,
            position,
            width: size.0.max(IMAGE_MIN_EDGE_MM),
            height: size.1.max(IMAGE_MIN_EDGE_MM),
            file,
            hash,
            source_px: None,
            keep_aspect: true,
            opacity: 100,
            border: false,
            locked: false,
            unknown: UnknownFields::default(),
        }
    }

    /// Top-left and bottom-right corners.
    pub fn bounds(&self) -> (Point, Point) {
        (
            self.position,
            Point::new(self.position.x + self.width, self.position.y + self.height),
        )
    }

    pub fn centre(&self) -> Point {
        Point::new(
            self.position.x + self.width / 2.0,
            self.position.y + self.height / 2.0,
        )
    }

    /// Map the image through `f` (move, rotate, mirror): its centre follows
    /// `f` and it stays upright, as a table does.
    pub fn map_points(&mut self, f: impl Fn(Point) -> Point) {
        let centre = self.centre();
        let moved = f(centre);
        self.position.x += moved.x - centre.x;
        self.position.y += moved.y - centre.y;
    }

    /// The opacity as a 0 to 1 factor.
    pub fn alpha(&self) -> f32 {
        f32::from(self.opacity.min(100)) / 100.0
    }

    /// The file name, without its folder.
    pub fn file_name(&self) -> &str {
        self.file.rsplit('/').next().unwrap_or(&self.file)
    }
}

/// The first size of an image whose own size is `natural` (mm), in a free
/// area of `free` (mm): as large as fits inside the area less
/// [`IMAGE_FIRST_SIZE_INSET_MM`] on each side, keeping the aspect ratio,
/// the width rounded down to [`IMAGE_FIRST_SIZE_GRID_MM`]. With no room for
/// [`IMAGE_FIRST_SIZE_MIN_MM`] the image takes its own size (mockup row
/// 204, decided 2026-11-26).
pub fn first_size_mm(natural: (f64, f64), free: (f64, f64)) -> (f64, f64) {
    let (nw, nh) = natural;
    if !(nw.is_finite() && nh.is_finite() && nw > 0.0 && nh > 0.0) {
        return (IMAGE_FIRST_SIZE_MIN_MM, IMAGE_FIRST_SIZE_MIN_MM);
    }
    let room_w = free.0 - 2.0 * IMAGE_FIRST_SIZE_INSET_MM;
    let room_h = free.1 - 2.0 * IMAGE_FIRST_SIZE_INSET_MM;
    if !(room_w >= IMAGE_FIRST_SIZE_MIN_MM && room_h >= IMAGE_FIRST_SIZE_MIN_MM) {
        return natural;
    }
    let scale = (room_w / nw).min(room_h / nh);
    // The epsilon keeps a width that is a whole number of steps, give or take
    // the float error, on that step.
    let steps = ((nw * scale) / IMAGE_FIRST_SIZE_GRID_MM + 1e-9).floor();
    let width = steps * IMAGE_FIRST_SIZE_GRID_MM;
    let width = width.max(IMAGE_FIRST_SIZE_GRID_MM);
    (width, width * nh / nw)
}

/// The free area of `sheet` an image's first size fills: the whole page when
/// it is empty, else the larger of the strip right of its content and the
/// strip below it, where content reaches as far as the objects' points.
pub fn free_area_mm(sheet: &SchematicSheet) -> (f64, f64) {
    let (size, orientation) = SheetSize::parse(&sheet.paper_size);
    let (page_w, page_h) = size.dimensions_mm(orientation);
    let points = sheet
        .symbols
        .iter()
        .map(|s| s.position)
        .chain(sheet.wires.iter().flat_map(|w| [w.start, w.end]))
        .chain(sheet.buses.iter().flat_map(|b| [b.start, b.end]))
        .chain(sheet.labels.iter().map(|l| l.position))
        .chain(sheet.junctions.iter().map(|j| j.position))
        .chain(sheet.text_notes.iter().map(|n| n.position))
        .chain(sheet.child_sheets.iter().flat_map(|c| {
            [
                c.position,
                Point::new(c.position.x + c.size.0, c.position.y + c.size.1),
            ]
        }))
        .chain(sheet.drawings.iter().filter_map(|d| match d {
            super::SchDrawing::Image(image) => Some(image.bounds().1),
            super::SchDrawing::Table(table) => Some(table.bounds().1),
            _ => None,
        }));
    let (mut max_x, mut max_y, mut any) = (0.0_f64, 0.0_f64, false);
    for p in points.filter(|p| p.x.is_finite() && p.y.is_finite()) {
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
        any = true;
    }
    if !any {
        return (page_w, page_h);
    }
    let right = ((page_w - max_x).max(0.0), page_h);
    let below = (page_w, (page_h - max_y).max(0.0));
    if right.0 * right.1 >= below.0 * below.1 {
        right
    } else {
        below
    }
}
