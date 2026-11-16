//! The operand forms of the placement extras (#1667):
//! `autoplace_fields id=<part>`, `place_design_block path=.. at=x,y` and
//! `import_graphics path=.. at=x,y`. Each ends in the commit its key and its
//! click end in, as one undo step. The file is read off the UI thread before
//! the edit runs (`edit_reads`). The anchor is the grid-snapped centre of
//! the content, as the cursor holds it, and `at` is refused off the grid as
//! for any placement.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use serde_json::json;

use super::placement_operands::{run_edit, text};
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// The `path` operand.
pub(super) fn path_operand(args: &CommandArgs) -> Result<PathBuf, String> {
    match args.operand("path") {
        Some(OperandValue::Path(path)) => Ok(PathBuf::from(path)),
        _ => Err("operand `path` is missing".to_string()),
    }
}

/// `autoplace_fields id=<part uuid> [document=..]`: answers with whether a
/// field moved.
pub(super) fn autoplace_fields(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let id = text(args, "id")?;
        let uuid = uuid::Uuid::parse_str(id.trim())
            .map_err(|_| format!("`id` must be a part id (a uuid), got `{id}`"))?;
        let is_part = app
            .document_state
            .edit_engine()
            .is_some_and(|engine| engine.document().symbols.iter().any(|s| s.uuid == uuid));
        if !is_part {
            return Err(format!("no part {uuid} on this sheet"));
        }
        let moved = app.autoplace_fields_of(vec![uuid])?;
        Ok(CommandOutcome::Answered(json!({
            "id": uuid.to_string(),
            "moved": moved,
        })))
    })
}

/// `place_design_block path=<.snxsch> at=x,y [document=..]`: answers with
/// how many objects were placed and how many sheet symbols were left out.
pub(super) fn place_design_block(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let path = path_operand(args)?;
        let at = app.grid_point(args, "at")?;
        let sheet = app
            .prefetched_sheet(&path)?
            .ok_or_else(|| format!("there is no file at {}", path.display()))?;
        let block = app.design_block_from(&path, Ok(Arc::new(sheet)))?;
        let placed = block.copied.payload.item_count();
        app.place_fragment_at(&block.copied, at)?;
        Ok(CommandOutcome::Answered(json!({
            "placed": placed,
            "skipped_sheet_symbols": block.skipped_sheet_symbols,
        })))
    })
}

/// `import_graphics path=<.dxf> at=x,y [document=..]`: answers with how
/// many drawings were placed and how many entities were skipped.
pub(super) fn import_graphics(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let path = path_operand(args)?;
        let at = app.grid_point(args, "at")?;
        let dxf = app.prefetched_text(&path)?;
        let graphics = signex_engine::drawings_from_dxf(&dxf, app.imported_graphics_width_mm())
            .map(Arc::new)
            .map_err(|error| format!("{}: {error}", path.display()));
        let skipped = graphics.as_ref().map_or(0, |g| g.skipped);
        let copied = app.graphics_from(&path, graphics)?;
        let placed = copied.payload.item_count();
        app.place_fragment_at(&copied, at)?;
        Ok(CommandOutcome::Answered(json!({
            "placed": placed,
            "skipped": skipped,
        })))
    })
}
