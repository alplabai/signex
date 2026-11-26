//! The operand form of Place ▸ Graphic… (#1116):
//! `place_graphic path=<image> at=x,y [width=<mm>] [document=..]`. The file
//! is read and checked off the UI thread before the edit runs
//! (`edit_reads`); the image lands as one undo step with its centre on `at`
//! (refused off the grid, as any placement), `width` wide or at its first
//! size, and a file from outside the project is copied in when the sheet is
//! saved. Answers with the image as JSON.

use std::collections::HashSet;

use iced::Task;
use serde_json::json;
use signex_types::schematic::SchDrawing;
use uuid::Uuid;

use super::placement_extras_operands::path_operand;
use super::placement_operands::{run_edit, text};
use super::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// The optional `width` operand: a length in mm above zero.
fn width_operand(args: &CommandArgs) -> Result<Option<f64>, String> {
    if args.operand("width").is_none() {
        return Ok(None);
    }
    let raw = text(args, "width")?;
    let width: f64 = raw
        .trim()
        .trim_end_matches("mm")
        .trim()
        .parse()
        .map_err(|_| format!("`width` must be a length in mm, got `{raw}`"))?;
    if !(width.is_finite() && width > 0.0) {
        return Err(format!("`width` must be above zero, got `{raw}`"));
    }
    Ok(Some(width))
}

/// The ids of the images on the sheet the edit acts on.
fn image_ids(app: &Signex) -> HashSet<Uuid> {
    app.document_state
        .edit_engine()
        .map(|engine| {
            engine
                .document()
                .drawings
                .iter()
                .filter(|d| matches!(d, SchDrawing::Image(_)))
                .map(SchDrawing::uuid)
                .collect()
        })
        .unwrap_or_default()
}

/// `place_graphic path=<image> at=x,y [width=<mm>] [document=..]`.
pub(super) fn place_graphic(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let path = path_operand(args)?;
        let at = app.grid_point(args, "at")?;
        let width = width_operand(args)?;
        let picked = app.prefetched_image(&path)?;
        let target = app
            .document_state
            .edit_path()
            .map(std::path::Path::to_path_buf)
            .ok_or_else(|| "no schematic is open".to_string())?;
        let copied = app.image_fragment(&picked, width, &target)?;
        let before = image_ids(app);
        app.place_fragment_at(&copied, at)?;
        app.note_image_copy(&target, &picked);
        let placed = app.document_state.edit_engine().and_then(|engine| {
            engine.document().drawings.iter().find_map(|d| match d {
                SchDrawing::Image(image) if !before.contains(&image.uuid) => Some(image.clone()),
                _ => None,
            })
        });
        let Some(image) = placed else {
            return Ok(CommandOutcome::Answered(json!({ "placed": 0 })));
        };
        Ok(CommandOutcome::Answered(json!({
            "placed": 1,
            "id": image.uuid.to_string(),
            "file": image.file,
            "hash": image.hash,
            "format": picked.facts.format.as_str(),
            "width_px": picked.facts.width_px,
            "height_px": picked.facts.height_px,
            "size": [image.width, image.height],
            "copied_on_save": picked.needs_copy,
        })))
    })
}
