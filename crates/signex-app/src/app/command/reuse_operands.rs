//! The snippet and template commands from their operands (#1283):
//! `save_snippet name=.. [scope=..] [ids=..] [document=..] [overwrite=..]`,
//! `place_snippet name=.. [scope=..] at=x,y [document=..]`, and the queries
//! `list_snippets` and `list_templates [kind=..]`. A snippet is placed as a
//! design block (`placement_extras_operands`); its file is read off the UI
//! thread first (`edit_reads`), and a save or a listing is answered when its
//! file work lands.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use serde_json::json;
use signex_types::object_id::UuidV7Ids;
use signex_types::reuse::{ReuseKind, ReuseScope};
use signex_types::schematic::SchematicSheet;

use super::document_operands::named_sheet;
use super::placement_operands::run_edit;
use super::queries::text_operand;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::design_reuse::{self, ListAnswer, SavedSnippet};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

/// The `scope` operand; `project` when left out.
pub(super) fn scope_operand(args: &CommandArgs) -> Result<ReuseScope, String> {
    design_reuse::scope_of(text_operand(args, "scope"))
}

/// The snippet `name` of `scope` an edit of `sheet` names: its scope, its
/// checked name and its file. Reads no disk.
pub(super) fn snippet_file(
    app: &Signex,
    args: &CommandArgs,
    sheet: &Path,
) -> Result<(ReuseScope, String, PathBuf), String> {
    let scope = scope_operand(args)?;
    let name = design_reuse::checked_name(text_operand(args, "name").unwrap_or(""), "snippet")?;
    let root = app.reuse_root(scope, Some(sheet))?;
    let file = ReuseKind::Snippet.entry_path(&root, &name);
    Ok((scope, name, file))
}

/// `place_snippet name=.. [scope=..] at=x,y [document=..]`: the snippet
/// placed with its centre on `at` as one undo step; answers with what was
/// placed.
pub(super) fn place_snippet(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let sheet = app
            .document_state
            .edit_path()
            .map(Path::to_path_buf)
            .ok_or("no schematic is open")?;
        let (scope, name, file) = snippet_file(app, args, &sheet)?;
        let at = app.grid_point(args, "at")?;
        let read = app
            .prefetched_sheet(&file)?
            .ok_or_else(|| format!("the {} has no snippet named {name}", scope.token()))?;
        let block = app.design_block_from(&file, Ok(Arc::new(read)))?;
        let placed = block.copied.payload.item_count();
        app.place_fragment_at(&block.copied, at)?;
        Ok(CommandOutcome::Answered(json!({
            "name": name,
            "scope": scope.token(),
            "file": file,
            "placed": placed,
            "skipped_sheet_symbols": block.skipped_sheet_symbols,
        })))
    })
}

/// `save_snippet name=.. [scope=..] [ids=..] [document=..]
/// [overwrite=..]`: the items `ids` names (the selection on the sheet in
/// view when left out) written as the snippet's file; answered when the
/// write lands.
pub(super) fn save_snippet(app: &mut Signex, args: &CommandArgs) -> Ran {
    let overwrite = matches!(args.operand("overwrite"), Some(OperandValue::Bool(true)));
    match snippet_to_save(app, args) {
        Ok((saved, text)) => app.start_snippet_write(args, saved, text, overwrite),
        Err(reason) => refused(reason),
    }
}

/// What `save_snippet` writes: the snippet and its file's text.
fn snippet_to_save(app: &Signex, args: &CommandArgs) -> Result<(SavedSnippet, String), String> {
    let sheet = match args.operand("document") {
        Some(_) => named_sheet(app, args, "a snippet")?,
        None => app
            .document_state
            .edit_path()
            .map(Path::to_path_buf)
            .ok_or("`save_snippet` needs a schematic sheet in view or the operand `document`")?,
    };
    let scope = scope_operand(args)?;
    let name = design_reuse::checked_name(text_operand(args, "name").unwrap_or(""), "snippet")?;
    let root = app.reuse_root(scope, Some(&sheet))?;
    let engine = app
        .document_state
        .engines
        .get(&sheet)
        .ok_or("the sheet is not open")?;
    let items = match args.targets("ids") {
        Some(ids) => ids
            .iter()
            .map(|id| {
                engine
                    .item_by_uuid(*id)
                    .ok_or_else(|| format!("no object {id} on the sheet"))
            })
            .collect::<Result<Vec<_>, _>>()?,
        None if app.document_state.active_path.as_deref() == Some(sheet.as_path()) => {
            app.interaction_state.active_canvas().selected.clone()
        }
        None => return Err("name the objects to save with `ids`".to_string()),
    };
    let fragment = engine
        .snippet_sheet(&items, &mut UuidV7Ids)
        .map_err(|error| format!("the snippet could not be made ({error})"))?
        .ok_or("nothing named can be saved as a snippet: name parts, wires, labels or text")?;
    let count = signex_engine::PlacementPayload::new(fragment.clone()).item_count();
    let text = snippet_text(fragment, &engine.document().paper_size)?;
    let saved = SavedSnippet {
        file: ReuseKind::Snippet.entry_path(&root, &name),
        name,
        scope,
        items: count,
        document: sheet,
    };
    Ok((saved, text))
}

/// The file text of the snippet `fragment`: a sheet of its own, with the
/// header a new sheet has and the paper size of the sheet it came from.
fn snippet_text(fragment: SchematicSheet, paper_size: &str) -> Result<String, String> {
    let blank = crate::app::handlers::document_files::blank_schematic_sheet_for_new_doc();
    let sheet = SchematicSheet {
        version: blank.version,
        generator: blank.generator,
        generator_version: blank.generator_version,
        root_sheet_page: blank.root_sheet_page,
        paper_size: paper_size.to_string(),
        ..fragment
    };
    signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .map_err(|error| format!("the snippet could not be written out: {error}"))
}

impl Signex {
    /// `list_snippets`: the snippets of the project in view and of the
    /// user's library, answered when the folders are read.
    pub(super) fn list_snippets(&mut self, args: &CommandArgs) -> Ran {
        self.start_reuse_listing(
            "list_snippets",
            args,
            vec![ReuseKind::Snippet],
            ListAnswer::Snippets,
        )
    }

    /// `list_templates [kind=sheet|project]`: the sheet templates of the
    /// project in view and of the library, and the library's project
    /// templates, answered when the folders are read.
    pub(super) fn list_templates(&mut self, args: &CommandArgs) -> Ran {
        let kinds = match text_operand(args, "kind").map(str::trim) {
            None => vec![ReuseKind::SheetTemplate, ReuseKind::ProjectTemplate],
            Some("sheet") => vec![ReuseKind::SheetTemplate],
            Some("project") => vec![ReuseKind::ProjectTemplate],
            Some(other) => {
                return refused(format!("`kind` is `sheet` or `project`, not `{other}`"));
            }
        };
        self.start_reuse_listing("list_templates", args, kinds, ListAnswer::Templates)
    }
}
