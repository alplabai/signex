//! `back_annotate_schematics document=.. [dry_run=..]` (#2470): Back Annotate
//! of the project `document` belongs to, without the dialog. The read is the
//! dialog's (`read::compute`), its closed sheets are stated again on the same
//! worker, and the rows the dialog ticks by default are written through the
//! same one-undo-group write as its Execute. The answer lists every row.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::pcb::eco::{BackBlock, BackChange, BackChangeList, BackRow};
use signex_i18n::{ids, t};

use super::read::compute;
use super::{BackAnnotateMsg, BackRead, OperandRead};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

const ID: &str = "back_annotate_schematics";

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (not_run(reason), Task::none())
}

/// The refusal of a run, the reason under the catalog's "did not run" line.
fn not_run(why: impl Into<String>) -> CommandOutcome {
    let reason =
        signex_i18n::Translatable::new(ids::BACK_ANNOTATE_COMMAND_REFUSED).arg("why", why.into());
    CommandOutcome::Disabled { reason }
}

/// The project `document` names (itself, its board or a sheet of it) and
/// the open board it names.
fn named_project_board(app: &Signex, args: &CommandArgs) -> Result<(ProjectId, PathBuf), String> {
    let Some(OperandValue::Document(target)) = args.operand("document") else {
        return Err(format!("`{ID}` needs the operand `document`"));
    };
    let project = app.project_named(target).map_err(|why| match why {
        crate::app::document_ids::NoProjectNamed::Unresolved(why) => why,
        crate::app::document_ids::NoProjectNamed::NotInProject(name) => {
            format!("`{ID}`: {name} is in no open project")
        }
    })?;
    let board = Signex::project_board(project)
        .ok_or_else(|| format!("`{ID}`: the project names no board"))?;
    if app.document_state.board_engine(&board).is_none() {
        return Err(format!(
            "`{ID}`: {} is not open; open it first",
            board.display()
        ));
    }
    Ok((project.id, board))
}

/// `back_annotate_schematics` from its operands: read on a worker, answered
/// when the read lands ([`CommandOutcome::Deferred`]).
pub(crate) fn back_annotate_schematics(app: &mut Signex, args: &CommandArgs) -> Ran {
    let command = match AppCommandId::new(ID) {
        Ok(command) => command,
        Err(why) => return refused(why),
    };
    let (project, board) = match named_project_board(app, args) {
        Ok(found) => found,
        Err(why) => return refused(why),
    };
    let request = match app.back_annotate_request(project, &board, None) {
        Ok(request) => request,
        Err(why) => return refused(format!("`{ID}`: {why}")),
    };
    let dry_run = matches!(args.operand("dry_run"), Some(OperandValue::Bool(true)));
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let task = Task::perform(
        async move {
            tokio::task::spawn_blocking(move || read_and_state(request))
                .await
                .unwrap_or_else(|e| Err(format!("the worker stopped: {e}")))
        },
        move |result| {
            Message::from(BackAnnotateMsg::OperandRead(Box::new(OperandRead {
                waiting,
                project,
                board,
                dry_run,
                result,
            })))
        },
    );
    (CommandOutcome::Deferred { ticket }, task)
}

/// The read, then the closed sheets it loaded stated again: the files that
/// changed on disk since. Blocking.
fn read_and_state(
    request: super::read::BackRequest,
) -> Result<(Arc<BackRead>, Vec<PathBuf>), String> {
    let read = compute(request)?;
    let stamps: Vec<_> = read
        .loaded
        .keys()
        .map(|file| (file.clone(), read.stamps.get(file).copied().flatten()))
        .collect();
    let changed = crate::app::sheet_write::disk_changed(&stamps);
    Ok((Arc::new(read), changed))
}

fn block_name(block: &BackBlock) -> &'static str {
    match block {
        BackBlock::NeedsAnnotation => "needs_annotation",
        BackBlock::DuplicateDesignator(_) => "duplicate_designator",
        BackBlock::BoardNotAnnotated => "board_not_annotated",
        BackBlock::SharedSheet => "shared_sheet",
        BackBlock::ChannelDesignator => "channel_designator",
        BackBlock::FootprintFromBoard => "footprint_from_board",
    }
}

/// One row of the answer: what it changes, on which sheet file, and whether
/// it applies (ticked and not blocked).
fn row_answer(row: &BackRow, read: &BackRead) -> Value {
    let file = |sheet: &signex_types::schematic::InstancePath, units: &[uuid::Uuid]| {
        read.files
            .get(&(sheet.clone(), units.to_vec()))
            .map(|part| part.file.display().to_string())
    };
    let (kind, footprint, reference, old, new, sheet, path) = match &row.change {
        BackChange::Designator {
            footprint,
            sheet,
            units,
            from,
            to,
        } => (
            "designator",
            footprint,
            from,
            from,
            to,
            Some(sheet),
            file(sheet, units),
        ),
        BackChange::Value {
            footprint,
            sheet,
            units,
            reference,
            from,
            to,
        } => (
            "value",
            footprint,
            reference,
            from,
            to,
            Some(sheet),
            file(sheet, units),
        ),
        BackChange::Footprint {
            footprint,
            reference,
            board,
            schematic,
        } => (
            "footprint",
            footprint,
            reference,
            schematic,
            board,
            None,
            None,
        ),
    };
    json!({
        "kind": kind,
        "footprint": footprint.to_string(),
        "designator": reference,
        "old": old,
        "new": new,
        "placement": sheet.map(ToString::to_string),
        "sheet": path,
        "applies": row.applies(),
        "blocked": row.blocked.as_ref().map(block_name),
    })
}

fn list_answer(board: &std::path::Path, list: &BackChangeList, read: &BackRead) -> Value {
    let rows: Vec<Value> = list.rows.iter().map(|row| row_answer(row, read)).collect();
    json!({
        "board": board.display().to_string(),
        "rows": rows,
    })
}

impl Signex {
    /// The operand form's read landed: answer its caller with the list, and
    /// unless it is a dry run write the rows that apply as one undo group.
    pub(super) fn back_annotate_operand_read(&mut self, read: OperandRead) -> Task<Message> {
        let OperandRead {
            waiting,
            project,
            board,
            dry_run,
            result,
        } = read;
        let target = board.clone();
        self.finish_waiting_command(waiting, &target, |app| {
            app.back_annotate_answer(project, &board, dry_run, result)
        })
    }

    fn back_annotate_answer(
        &mut self,
        project: ProjectId,
        board: &std::path::Path,
        dry_run: bool,
        result: Result<(Arc<BackRead>, Vec<PathBuf>), String>,
    ) -> CommandOutcome {
        let (read, changed) = match result {
            Ok(found) => found,
            Err(why) => return not_run(format!("`{ID}`: {why}")),
        };
        for (library, why) in &read.library_errors {
            crate::diagnostics::log_warning(format!(
                "Back Annotate: library {} did not open: {why}",
                library.display()
            ));
        }
        // The dialog's default ticks: shared values blocked, a rename that
        // would put a designator on two parts unticked.
        let mut list = read.list.clone();
        self.block_values_shared_elsewhere(project, &read, &mut list);
        super::duplicates::untick_duplicate_renames(&mut list, None);
        if self.back_read_stale(&list, &read, board) {
            let why = t!(self.ui_state.i18n, ids::BACK_ANNOTATE_STALE).into_string();
            return not_run(format!("`{ID}`: nothing was changed: {why}"));
        }
        let mut answer = list_answer(board, &list, &read);
        let applies = list.rows.iter().filter(|row| row.applies()).count();
        if dry_run || applies == 0 {
            answer["applied"] = json!(false);
            answer["sheets_changed"] = json!(0);
            return CommandOutcome::Answered(answer);
        }
        if let Some(file) = changed.first() {
            return not_run(format!(
                "`{ID}`: nothing was changed: {} changed on disk since it was read",
                file.display()
            ));
        }
        let edits = match self.back_annotate_edits(project, &list, &read) {
            Ok(edits) => edits,
            Err(why) => return not_run(format!("`{ID}`: {why}")),
        };
        // Every closed sheet the read loaded was stated again on the worker.
        let checked: BTreeSet<PathBuf> = read.loaded.keys().cloned().collect();
        match self.write_back_annotation(&read, edits, &checked) {
            Ok(sheets) => {
                crate::diagnostics::log_info(
                    t!(
                        self.ui_state.i18n,
                        ids::BACK_ANNOTATE_DONE,
                        "count" => applies,
                        "sheets" => sheets
                    )
                    .into_string(),
                );
                answer["applied"] = json!(true);
                answer["sheets_changed"] = json!(sheets);
                CommandOutcome::Answered(answer)
            }
            Err(why) => not_run(format!("`{ID}`: nothing was changed: {why}")),
        }
    }
}
