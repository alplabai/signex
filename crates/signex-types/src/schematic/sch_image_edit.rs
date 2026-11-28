//! Edits of a placed image (#1116, mockup row 204): the Properties rows
//! (Keep aspect ratio, Scale, W / H, Reset to file size, Opacity, Border),
//! the eight resize handles and Relink. Pure: the app sends the result
//! through `Command::UpdateSchDrawing`, so every edit is one undo step.
//! Rules: docs/SCHEMATIC_IMAGES.md, "Editing an image".

use super::sch_image::IMAGE_MIN_EDGE_MM;
use super::{Point, SchImage};
use crate::image_asset::px_to_mm;

/// Largest edge an image is sized to: one metre.
pub const IMAGE_MAX_EDGE_MM: f64 = 1000.0;

/// One edit of a placed image.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageEdit {
    /// The width in mm; the height follows while the aspect is kept.
    Width(f64),
    /// The height in mm; the width follows while the aspect is kept.
    Height(f64),
    /// Percent of the file's own size (96 dpi); 100 is Reset to file size.
    Scale(f64),
    KeepAspect(bool),
    /// Percent, 0 to 100.
    Opacity(u8),
    Border(bool),
    /// The whole box, from a handle drag: its top-left corner and size.
    Bounds {
        position: Point,
        size: (f64, f64),
    },
}

/// Why an image edit was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageEditRefusal {
    #[error("an image edge must be a length from 0.1 mm to 1000 mm")]
    BadLength,
    #[error("the scale must be a percentage above zero")]
    BadScale,
    #[error("the opacity is a percentage from 0 to 100")]
    BadOpacity,
    #[error("the file's own size is not known, so the image has no scale")]
    NoFileSize,
    #[error("a position must be a finite point")]
    BadPosition,
}

/// Whether `mm` is an edge an image takes.
fn edge_holds(mm: f64) -> bool {
    mm.is_finite() && (IMAGE_MIN_EDGE_MM..=IMAGE_MAX_EDGE_MM).contains(&mm)
}

fn checked(size: (f64, f64)) -> Result<(f64, f64), ImageEditRefusal> {
    if edge_holds(size.0) && edge_holds(size.1) {
        Ok(size)
    } else {
        Err(ImageEditRefusal::BadLength)
    }
}

impl SchImage {
    /// The file's own size in mm at 96 dpi, when its pixels are known.
    pub fn natural_size_mm(&self) -> Option<(f64, f64)> {
        let [w, h] = self.source_px?;
        (w > 0 && h > 0).then(|| (px_to_mm(w), px_to_mm(h)))
    }

    /// The width as a percentage of the file's own width.
    pub fn scale_percent(&self) -> Option<f64> {
        let (natural_w, _) = self.natural_size_mm()?;
        Some(self.width / natural_w * 100.0)
    }

    /// This image with `edit` applied; `Ok(None)` when nothing changes.
    pub fn with_edit(&self, edit: &ImageEdit) -> Result<Option<SchImage>, ImageEditRefusal> {
        let mut next = self.clone();
        match edit {
            ImageEdit::Width(width) => {
                let height = if self.keep_aspect {
                    self.height * width / self.width
                } else {
                    self.height
                };
                (next.width, next.height) = checked((*width, height))?;
            }
            ImageEdit::Height(height) => {
                let width = if self.keep_aspect {
                    self.width * height / self.height
                } else {
                    self.width
                };
                (next.width, next.height) = checked((width, *height))?;
            }
            ImageEdit::Scale(percent) => {
                if !(percent.is_finite() && *percent > 0.0) {
                    return Err(ImageEditRefusal::BadScale);
                }
                let (w, h) = self.natural_size_mm().ok_or(ImageEditRefusal::NoFileSize)?;
                let factor = percent / 100.0;
                (next.width, next.height) = checked((w * factor, h * factor))?;
            }
            ImageEdit::KeepAspect(on) => next.keep_aspect = *on,
            ImageEdit::Opacity(percent) => {
                if *percent > 100 {
                    return Err(ImageEditRefusal::BadOpacity);
                }
                next.opacity = *percent;
            }
            ImageEdit::Border(on) => next.border = *on,
            ImageEdit::Bounds { position, size } => {
                if !(position.x.is_finite() && position.y.is_finite()) {
                    return Err(ImageEditRefusal::BadPosition);
                }
                (next.width, next.height) = checked(*size)?;
                next.position = *position;
            }
        }
        Ok((next != *self).then_some(next))
    }

    /// This image showing another file (Relink, Replace): the box keeps its
    /// place and width; while the aspect is kept the height follows the new
    /// file's.
    pub fn relinked(&self, file: &str, hash: &str, source_px: [u32; 2]) -> SchImage {
        let mut next = self.clone();
        next.file = file.to_string();
        next.hash = hash.to_string();
        next.source_px = Some(source_px);
        let [w, h] = source_px;
        if self.keep_aspect && w > 0 && h > 0 {
            let height = self.width * f64::from(h) / f64::from(w);
            if edge_holds(height) {
                next.height = height;
            }
        }
        next
    }
}

/// One of the eight handles of a selected image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl ImageHandle {
    pub const ALL: [Self; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Right,
        Self::BottomRight,
        Self::Bottom,
        Self::BottomLeft,
        Self::Left,
    ];

    /// Which edge each axis moves: -1 the left or top one, 1 the right or
    /// bottom one, 0 none.
    fn sides(self) -> (i8, i8) {
        match self {
            Self::TopLeft => (-1, -1),
            Self::Top => (0, -1),
            Self::TopRight => (1, -1),
            Self::Right => (1, 0),
            Self::BottomRight => (1, 1),
            Self::Bottom => (0, 1),
            Self::BottomLeft => (-1, 1),
            Self::Left => (-1, 0),
        }
    }

    pub fn is_corner(self) -> bool {
        let (x, y) = self.sides();
        x != 0 && y != 0
    }

    /// Where the handle sits on the box with top-left `position` and `size`.
    pub fn at(self, position: Point, size: (f64, f64)) -> Point {
        let (sx, sy) = self.sides();
        let along = |side: i8, start: f64, length: f64| match side {
            -1 => start,
            0 => start + length / 2.0,
            _ => start + length,
        };
        Point::new(along(sx, position.x, size.0), along(sy, position.y, size.1))
    }
}

/// The box after `handle` is dragged by `delta` (mm): the opposite edge or
/// corner stays put. While the aspect is kept a corner keeps it (the larger
/// change wins) and a side handle does nothing (mockup row 204); an edge
/// never shrinks below the smallest an image takes.
pub fn resized_by_handle(
    position: Point,
    size: (f64, f64),
    handle: ImageHandle,
    delta: (f64, f64),
    keep_aspect: bool,
) -> (Point, (f64, f64)) {
    if keep_aspect && !handle.is_corner() {
        return (position, size);
    }
    let (sx, sy) = handle.sides();
    let stretched = |side: i8, length: f64, d: f64| match side {
        -1 => length - d,
        0 => length,
        _ => length + d,
    };
    let mut width = stretched(sx, size.0, delta.0).max(IMAGE_MIN_EDGE_MM);
    let mut height = stretched(sy, size.1, delta.1).max(IMAGE_MIN_EDGE_MM);
    if keep_aspect && size.0 > 0.0 && size.1 > 0.0 {
        let aspect = size.0 / size.1;
        if width / size.0 >= height / size.1 {
            height = (width / aspect).max(IMAGE_MIN_EDGE_MM);
        } else {
            width = (height * aspect).max(IMAGE_MIN_EDGE_MM);
        }
    }
    let x = if sx < 0 {
        position.x + size.0 - width
    } else {
        position.x
    };
    let y = if sy < 0 {
        position.y + size.1 - height
    } else {
        position.y
    };
    (Point::new(x, y), (width, height))
}
