//! The `[[extras.sheet.images]]` rows (#1116): one per placed image, with
//! its placement above the circuit in the row itself. Images are kept out of
//! the `drawings` array on purpose: an older build keeps an unknown key of
//! `[extras.sheet]` and writes it back, so the rows need no new format
//! token, where an unknown drawing `type` would stop it reading the sheet.
//! In memory an image is a drawing (`SchDrawing::Image`). Format:
//! `docs/SCHEMATIC_IMAGES.md`, "File format".

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schematic::{DrawingPlacement, SchDrawing, SchImage, SchematicSheet};

fn is_false(value: &bool) -> bool {
    !*value
}

/// One image row: the image and whether it paints above the circuit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(in crate::format) struct ImageRow {
    #[serde(flatten)]
    image: SchImage,
    #[serde(default, skip_serializing_if = "is_false")]
    above: bool,
}

/// The rows a save writes, in drawing order.
pub(in crate::format) fn rows_of(sheet: &SchematicSheet) -> Vec<ImageRow> {
    sheet
        .drawings
        .iter()
        .filter_map(|d| match d {
            SchDrawing::Image(image) => Some(ImageRow {
                above: sheet.drawing_placement(image.uuid) == DrawingPlacement::Above,
                image: image.clone(),
            }),
            _ => None,
        })
        .collect()
}

/// The images of `rows` as drawings, painted after the other drawings.
pub(in crate::format) fn drawings(rows: Vec<ImageRow>) -> impl Iterator<Item = SchDrawing> {
    rows.into_iter().map(|row| SchDrawing::Image(row.image))
}

/// `placements` with the images of `rows` that paint above the circuit.
pub(in crate::format) fn with_placements(
    mut placements: BTreeMap<Uuid, DrawingPlacement>,
    rows: &[ImageRow],
) -> BTreeMap<Uuid, DrawingPlacement> {
    for row in rows.iter().filter(|row| row.above) {
        placements.insert(row.image.uuid, DrawingPlacement::Above);
    }
    placements
}

/// Whether `drawing` is written among the `drawings` (an image is not).
pub(in crate::format) fn is_drawing_row(drawing: &SchDrawing) -> bool {
    !matches!(drawing, SchDrawing::Image(_))
}

#[cfg(test)]
#[path = "image_rows_tests.rs"]
mod tests;
