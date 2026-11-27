//! The operand forms of the image commands (#1116, mockup row 204).
//!
//! - `place_graphic path=<image> at=x,y [width=<mm>] [document=..]`: the
//!   file is read and checked off the UI thread before the edit runs
//!   (`edit_reads`); the image lands as one undo step with its centre on
//!   `at` (refused off the grid, as any placement), `width` wide or at its
//!   first size, and a file from outside the project is copied in when the
//!   sheet is saved.
//! - `set_image id=.. field=.. value=.. [document=..]`: one field, as the
//!   Properties ▸ Image row of the same name sets it.
//! - `relink_image path=<image> (file=<asset> | id=..) [document=..]`:
//!   Relink… (every image naming `file`, on every sheet of the project) or
//!   Replace… (the image `id`), one undo step.
//! - `show_image_in_folder id=.. [document=..]`: the file's paths; a window
//!   also shows it in the file manager.
//!
//! Each answers with JSON; see docs/CLI.md, "Images".

use std::collections::HashSet;

use iced::Task;
use serde_json::json;
use signex_types::schematic::SchDrawing;
use uuid::Uuid;

use super::placement_extras_operands::path_operand;
use super::placement_operands::{run_edit, text};
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::images::RelinkScope;
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

/// The `id` operand as an image id.
fn image_id(args: &CommandArgs) -> Result<Uuid, String> {
    let id = text(args, "id")?;
    Uuid::parse_str(id.trim()).map_err(|_| format!("`id` must be an image id (a uuid), got `{id}`"))
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

/// `set_image id=.. field=.. value=.. [document=..]`.
pub(super) fn set_image(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let id = image_id(args)?;
        let field = text(args, "field")?;
        let value = text(args, "value")?;
        let edit = crate::app::images::edit::parse_image_field(field.trim(), &value)?;
        app.edit_image(id, &edit).map(CommandOutcome::Answered)
    })
}

/// The images `relink_image` acts on: `file` or `id`, exactly one.
fn relink_scope(args: &CommandArgs) -> Result<RelinkScope, String> {
    match (args.operand("file"), args.operand("id")) {
        (Some(_), Some(_)) => Err("`relink_image` takes `file` or `id`, not both".to_string()),
        (Some(_), None) => {
            let file = text(args, "file")?;
            let file = signex_types::image_asset::checked_asset_path(&file)
                .map_err(|refusal| refusal.to_string())?;
            Ok(RelinkScope::File(file))
        }
        (None, Some(_)) => image_id(args).map(RelinkScope::Image),
        (None, None) => Err(
            "`relink_image` needs `file` (every image naming it) or `id` (one image)".to_string(),
        ),
    }
}

/// `relink_image path=<image> (file=<asset> | id=..) [document=..]`.
pub(super) fn relink_image(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let path = path_operand(args)?;
        let scope = relink_scope(args)?;
        let picked = app.prefetched_image(&path)?;
        let target = app
            .document_state
            .edit_path()
            .map(std::path::Path::to_path_buf)
            .ok_or_else(|| "no schematic is open".to_string())?;
        let in_project = app.document_state.document_project(&target).is_some();
        let project = match &scope {
            RelinkScope::File(_) if in_project => Some(app.edit_project_set(&target)?.sheets),
            _ => None,
        };
        app.relink_images(&target, &scope, &picked, project.as_ref())
            .map(CommandOutcome::Answered)
    })
}

/// `show_image_in_folder id=.. [document=..]`: answers with the paths; in a
/// window the file manager shows the file too. A read-only sheet is fine.
pub(super) fn show_image_in_folder(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match image_id(args) {
        Ok(id) => id,
        Err(reason) => return (CommandOutcome::disabled(reason), Task::none()),
    };
    let sheet = match args.operand("document") {
        Some(OperandValue::Document(_)) => {
            match super::document_operands::named_sheet(app, args, "show_image_in_folder") {
                Ok(path) => Some(path),
                Err(reason) => return (CommandOutcome::disabled(reason), Task::none()),
            }
        }
        _ => None,
    };
    let run = |app: &mut Signex| match app.image_folder_answer(id) {
        Ok(answer) => {
            let shown = answer["shown"].as_bool().unwrap_or(false);
            let task = if shown {
                app.show_image_in_folder(id)
            } else {
                Task::none()
            };
            (CommandOutcome::Answered(answer), task)
        }
        Err(reason) => (CommandOutcome::disabled(reason), Task::none()),
    };
    match sheet {
        Some(path) => app.on_edit_target(&path, run),
        None => run(app),
    }
}
