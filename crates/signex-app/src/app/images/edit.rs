//! The edits of a placed image (#1116, mockup row 204): the Properties ▸
//! Image rows, the resize handles and `set_image`. A row, a handle drop and
//! an operand run call the same method here, so one edit is one engine
//! command and one undo step on every surface. The rules are
//! `SchImage::with_edit`'s.

use serde_json::{Value, json};
use signex_types::schematic::{DrawingPlacement, ImageEdit, SchDrawing, SchImage};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::mutation_gateway::MutationOutcome;

/// A field `set_image` sets.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ImageFieldEdit {
    Image(ImageEdit),
    Placement(DrawingPlacement),
}

/// The fields of `set_image`, in the order its refusal lists them.
pub(crate) const IMAGE_FIELDS: &[&str] = &[
    "width",
    "height",
    "scale",
    "keep_aspect",
    "opacity",
    "border",
    "placement",
    "bounds",
];

fn number(field: &str, value: &str) -> Result<f64, String> {
    let trimmed = value.trim().trim_end_matches(['%']).trim_end_matches("mm");
    signex_types::numeric::parse_finite_f64(trimmed.trim())
        .map_err(|_| format!("`{field}` takes a number, got `{value}`"))
}

fn flag(field: &str, value: &str) -> Result<bool, String> {
    match value.trim() {
        "true" | "on" | "yes" => Ok(true),
        "false" | "off" | "no" => Ok(false),
        _ => Err(format!("`{field}` takes true or false, got `{value}`")),
    }
}

/// The edit `field=value` names, as `set_image` reads it. `scale=file`
/// resets the image to the file's own size, as Reset to file size does.
pub(crate) fn parse_image_field(field: &str, value: &str) -> Result<ImageFieldEdit, String> {
    let edit = match field {
        "width" => ImageEdit::Width(number(field, value)?),
        "height" => ImageEdit::Height(number(field, value)?),
        "scale" if value.trim() == "file" => ImageEdit::Scale(100.0),
        "scale" => ImageEdit::Scale(number(field, value)?),
        "keep_aspect" => ImageEdit::KeepAspect(flag(field, value)?),
        "opacity" => {
            let percent = number(field, value)?;
            if !(0.0..=100.0).contains(&percent) {
                return Err(format!("`opacity` is 0 to 100, got `{value}`"));
            }
            ImageEdit::Opacity(percent.round() as u8)
        }
        "border" => ImageEdit::Border(flag(field, value)?),
        "placement" => {
            return match value.trim() {
                "behind" => Ok(ImageFieldEdit::Placement(DrawingPlacement::Behind)),
                "above" => Ok(ImageFieldEdit::Placement(DrawingPlacement::Above)),
                _ => Err(format!("`placement` is behind or above, got `{value}`")),
            };
        }
        "bounds" => {
            let parts: Vec<&str> = value.split(',').collect();
            let [x, y, w, h] = parts.as_slice() else {
                return Err(format!("`bounds` is x,y,width,height in mm, got `{value}`"));
            };
            ImageEdit::Bounds {
                position: signex_types::schematic::Point::new(number(field, x)?, number(field, y)?),
                size: (number(field, w)?, number(field, h)?),
            }
        }
        _ => {
            return Err(format!(
                "`field` is one of {}, got `{field}`",
                IMAGE_FIELDS.join(", ")
            ));
        }
    };
    Ok(ImageFieldEdit::Image(edit))
}

/// The image `id` on `sheet`.
pub(crate) fn image_in(
    sheet: &signex_types::schematic::SchematicSheet,
    id: Uuid,
) -> Option<&SchImage> {
    sheet.drawings.iter().find_map(|d| match d {
        SchDrawing::Image(image) if image.uuid == id => Some(image),
        _ => None,
    })
}

/// An image as an answer names it.
pub(crate) fn image_json(image: &SchImage, placement: DrawingPlacement) -> Value {
    json!({
        "id": image.uuid.to_string(),
        "file": image.file,
        "hash": image.hash,
        "position": [image.position.x, image.position.y],
        "size": [image.width, image.height],
        "scale": image.scale_percent(),
        "source_px": image.source_px,
        "keep_aspect": image.keep_aspect,
        "opacity": image.opacity,
        "border": image.border,
        "placement": match placement {
            DrawingPlacement::Behind => "behind",
            DrawingPlacement::Above => "above",
        },
        "locked": image.locked,
    })
}

impl Signex {
    /// The image `id` on the sheet an edit acts on.
    pub(crate) fn image_on_edit_sheet(&self, id: Uuid) -> Result<SchImage, String> {
        let engine = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?;
        image_in(engine.document(), id)
            .cloned()
            .ok_or_else(|| format!("no image {id} on this sheet"))
    }

    /// The image `id` as the answer of an edit names it.
    pub(crate) fn image_answer(&self, id: Uuid) -> Result<Value, String> {
        let engine = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?;
        let sheet = engine.document();
        let image = image_in(sheet, id).ok_or_else(|| format!("no image {id} on this sheet"))?;
        Ok(image_json(image, sheet.drawing_placement(id)))
    }

    /// Run `command` on the sheet an edit acts on, unless a lock holds the
    /// image: then the lock's reason, with nothing sent.
    fn commit_image_command(&mut self, command: signex_engine::Command) -> Result<(), String> {
        let engine = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?;
        if let Some(reason) = engine.lock_refusal(&command) {
            return Err(reason);
        }
        let (outcome, reason) = self.apply_engine_commands_reporting(vec![command], true, true);
        match outcome {
            MutationOutcome::Changed | MutationOutcome::Unchanged => Ok(()),
            MutationOutcome::Failed => {
                Err(reason.unwrap_or_else(|| "the engine refused the edit".to_string()))
            }
        }
    }

    /// Apply `edit` to the image `id` on the sheet an edit acts on, as one
    /// undo step; an edit that changes nothing sends nothing. Answers with
    /// the image as it is after.
    pub(crate) fn edit_image(&mut self, id: Uuid, edit: &ImageFieldEdit) -> Result<Value, String> {
        let image = self.image_on_edit_sheet(id)?;
        match edit {
            ImageFieldEdit::Image(edit) => {
                let next = image
                    .with_edit(edit)
                    .map_err(|refusal| refusal.to_string())?;
                if let Some(next) = next {
                    self.commit_image_command(signex_engine::Command::UpdateSchDrawing {
                        drawing: SchDrawing::Image(next),
                    })?;
                }
            }
            ImageFieldEdit::Placement(placement) => {
                let current = self
                    .document_state
                    .edit_engine()
                    .map(|engine| engine.document().drawing_placement(id));
                if current != Some(*placement) {
                    self.commit_image_command(signex_engine::Command::SetDrawingPlacement {
                        ids: vec![id],
                        placement: *placement,
                    })?;
                }
            }
        }
        self.image_answer(id)
    }
}
