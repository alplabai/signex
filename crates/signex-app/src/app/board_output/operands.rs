//! The board output commands from their operands (#2404, #2402, #2488): an
//! agent or the CLI names the board (`document`) and gets the table as JSON,
//! or the CLI writes the CSV, PDF or STEP model to `path`. The same builders
//! and export write path as the dialogs. The CSV and the PDF are answered
//! once written (#2470), so an agent on the socket may run them.

use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::RenderTranslatable as _;
use signex_output::{
    BoardLayerSet, BoardPrintOptions, BoardPrintPages, BoardPrintScale, BoardReportKind,
};
use signex_renderer::board3d::StepSchema;

use super::{BoardOutputMsg, CommandWritten};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::contracts::ExportWriteOutcome;
use crate::app::document_ids::DocumentKind;
use crate::app::handlers::menu::export::write::WriteRule;
use crate::app::notebook_project::{because, export_target, overwrite};
use crate::app::{ExportMsg, Message, Signex};
use crate::keymap::AppCommandId;

pub(super) type Ran = (CommandOutcome, Task<Message>);

pub(super) fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

pub(super) fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

pub(super) fn flag(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(value)) => Some(*value),
        _ => None,
    }
}

pub(super) fn file(args: &CommandArgs) -> Option<PathBuf> {
    match args.operand("path") {
        Some(OperandValue::Path(path)) => Some(PathBuf::from(path)),
        _ => None,
    }
}

impl Signex {
    /// The open board `document` names. Refused with the reason when it
    /// names none, names another kind, or the board is not open. Length
    /// tuning (#2389) reads its board the same way.
    pub(crate) fn named_output_board(&self, args: &CommandArgs) -> Result<PathBuf, String> {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            return Err("needs the operand `document`".to_string());
        };
        let entry = self.resolve_document_target(named)?;
        if entry.kind != DocumentKind::Pcb {
            return Err(format!(
                "{} is a {}, not a board",
                entry.name,
                entry.kind.as_str()
            ));
        }
        let path = entry
            .path
            .ok_or_else(|| format!("{} has no file", entry.name))?;
        if self.document_state.pcb_board(&path).is_none() {
            return Err(format!("{} is not open: open the board first", entry.name));
        }
        Ok(path)
    }
}

/// [`Signex::named_output_board`], refused with the same reasons as every
/// board output command: `fabrication_outputs` (#2470) reads its board so.
pub(crate) fn output_board(app: &Signex, args: &CommandArgs) -> Result<PathBuf, CommandOutcome> {
    app.named_output_board(args)
        .map_err(|reason| refused(reason).0)
}

/// `board_report_<kind> document=<board>`: the table as JSON.
fn report_answer(app: &mut Signex, args: &CommandArgs, kind: BoardReportKind) -> Ran {
    let board = match app.named_output_board(args) {
        Ok(board) => board,
        Err(reason) => return refused(reason),
    };
    let Some(report) = app.board_report_of(&board, kind) else {
        return refused("the board is not open");
    };
    let mut answer = serde_json::to_value(&report).unwrap_or(Value::Null);
    answer["document"] = json!(board.display().to_string());
    (CommandOutcome::Answered(answer), Task::none())
}

pub(crate) fn board_report_bom(app: &mut Signex, args: &CommandArgs) -> Ran {
    report_answer(app, args, BoardReportKind::Bom)
}

pub(crate) fn board_report_netlist(app: &mut Signex, args: &CommandArgs) -> Ran {
    report_answer(app, args, BoardReportKind::Netlist)
}

pub(crate) fn board_report_net_length(app: &mut Signex, args: &CommandArgs) -> Ran {
    report_answer(app, args, BoardReportKind::NetLength)
}

pub(crate) fn board_report_components(app: &mut Signex, args: &CommandArgs) -> Ran {
    report_answer(app, args, BoardReportKind::Components)
}

/// `export_board_report document=<board> kind=<kind> path=<file>
/// [overwrite=true]`: the CSV, written off the UI thread; the command is
/// answered once it is written, with what is written where.
pub(crate) fn export_board_report(app: &mut Signex, args: &CommandArgs) -> Ran {
    let board = match app.named_output_board(args) {
        Ok(board) => board,
        Err(reason) => return refused(reason),
    };
    let Some(kind) = text(args, "kind").and_then(BoardReportKind::from_id) else {
        let ids: Vec<&str> = BoardReportKind::ALL.iter().map(|k| k.id()).collect();
        return refused(format!("`kind` is one of {}", ids.join(", ")));
    };
    let Some(path) = file(args) else {
        return refused("needs the operand `path`");
    };
    let (path, rule) = match command_target(app, &board, &path, "csv", args) {
        Ok(found) => found,
        Err(ran) => return ran,
    };
    let Some(report) = app.board_report_of(&board, kind) else {
        return refused("the board is not open");
    };
    let answer = json!({
        "document": board.display().to_string(),
        "kind": kind.id(),
        "path": path.display().to_string(),
        "rows": report.rows.len(),
    });
    let file = CommandFile {
        id: "export_board_report",
        board,
        what: "CSV",
        success_log: super::reports::report_written_log(&report, &path),
        bytes: report.to_csv(),
        warnings: Vec::new(),
        path,
        rule,
        answer,
    };
    write_command_file(app, args, file)
}

/// The file a board export command writes (#2470): `path` checked by the
/// one export rule (`notebook_project::export_target`: the `extension`, no
/// Signex document, no `.git` folder, no file replaced without
/// `overwrite=true`) and refused while its project folder is being renamed;
/// with it, how the write lands (`WriteRule`: an agent's confinement, and
/// whether a file already there may be replaced).
fn command_target(
    app: &Signex,
    board: &Path,
    path: &Path,
    extension: &str,
    args: &CommandArgs,
) -> Result<(PathBuf, WriteRule), Ran> {
    let disabled = |reason| (CommandOutcome::Disabled { reason }, Task::none());
    let folder = board.parent().unwrap_or(Path::new(""));
    // Whether a file is already there is decided by the write, which claims
    // the name (`write_new`): no disk read here, and no file that appears
    // meanwhile is replaced.
    let target = export_target(folder, path, extension, true).map_err(disabled)?;
    if let Err(why) = app.refuse_while_project_moves(&target, "export to") {
        return Err(disabled(because(why.to_string())));
    }
    let rule = WriteRule {
        confine: app.agent_write_root(board),
        replace: overwrite(args),
    };
    Ok((target, rule))
}

/// A board export command's file and its answer.
struct CommandFile {
    id: &'static str,
    board: PathBuf,
    what: &'static str,
    path: PathBuf,
    rule: WriteRule,
    bytes: Vec<u8>,
    success_log: String,
    warnings: Vec<String>,
    answer: Value,
}

/// Write `file` off the UI thread and answer the command once it is
/// written, or refuse it with the write's failure (#2470): an agent is
/// never told a file was written that was not.
fn write_command_file(app: &mut Signex, args: &CommandArgs, file: CommandFile) -> Ran {
    let command = AppCommandId::new(file.id).expect("a catalog id is valid");
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let CommandFile {
        board,
        what,
        path,
        rule,
        bytes,
        success_log,
        warnings,
        answer,
        ..
    } = file;
    let written = CommandWritten {
        waiting,
        board,
        answer,
        path: path.clone(),
        result: Ok(()),
        success_log: success_log.clone(),
        warnings: warnings.clone(),
    };
    let task = app
        .write_command_export(rule, path, bytes, what, success_log, warnings)
        .map(move |message| match message {
            Message::Export(ExportMsg::FileWritten(outcome)) => {
                Message::from(BoardOutputMsg::CommandWritten(Box::new(CommandWritten {
                    result: outcome.result.clone(),
                    ..written.clone()
                })))
            }
            other => other,
        });
    (CommandOutcome::Deferred { ticket }, task)
}

impl Signex {
    /// A board export command's file was written, or not: the export's
    /// Messages lines, then the command answered or refused.
    pub(super) fn finish_command_export(&mut self, written: CommandWritten) -> Task<Message> {
        let CommandWritten {
            waiting,
            board,
            answer,
            path,
            result,
            success_log,
            warnings,
        } = written;
        let logged = self.handle_export_written(ExportWriteOutcome {
            path,
            result: result.clone(),
            success_log,
            advisory: None,
            warnings,
            preview_source: None,
        });
        let answered = self.finish_waiting_command(waiting, &board, move |_| match result {
            Ok(()) => CommandOutcome::Answered(answer),
            Err(why) => CommandOutcome::Disabled {
                reason: because(why),
            },
        });
        Task::batch([logged, answered])
    }
}

/// The print options the operands ask for, from the defaults the dialog
/// first opens with. Refused when a value is not one the option takes.
pub(super) fn print_options(args: &CommandArgs) -> Result<BoardPrintOptions, String> {
    let mut options = BoardPrintOptions::default();
    if let Some(layers) = text(args, "layers") {
        let mut sets = Vec::new();
        for id in layers.split(',').filter(|id| !id.trim().is_empty()) {
            let set = BoardLayerSet::from_id(id).ok_or_else(|| {
                let ids: Vec<&str> = BoardLayerSet::ALL.iter().map(|s| s.id()).collect();
                format!(
                    "`{}` is not a layer set (one of {})",
                    id.trim(),
                    ids.join(", ")
                )
            })?;
            if !sets.contains(&set) {
                sets.push(set);
            }
        }
        options.sets = sets;
    }
    if let Some(pages) = text(args, "pages") {
        options.pages = match pages.trim() {
            "per_set" => BoardPrintPages::PerSet,
            "composite" => BoardPrintPages::Composite,
            other => return Err(format!("`pages` is per_set or composite, not `{other}`")),
        };
    }
    if let Some(scale) = text(args, "scale") {
        options.scale = match scale.trim() {
            "fit" => BoardPrintScale::Fit,
            "one_to_one" | "1:1" => BoardPrintScale::OneToOne,
            other => return Err(format!("`scale` is fit or one_to_one, not `{other}`")),
        };
    }
    let flags: [(&str, &mut bool); 5] = [
        ("monochrome", &mut options.monochrome),
        ("mirror_bottom", &mut options.mirror_bottom),
        ("outline", &mut options.outline),
        ("drill_marks", &mut options.drill_marks),
        ("title_block", &mut options.title_block),
    ];
    for (name, slot) in flags {
        if let Some(on) = flag(args, name) {
            *slot = on;
        }
    }
    if options.page_plan().is_empty() {
        return Err("`layers` names no layer set to print".to_string());
    }
    Ok(options)
}

/// `export_board_pdf document=<board> path=<file> [options]`: the PDF,
/// written off the UI thread; the answer lists the pages.
pub(crate) fn export_board_pdf(app: &mut Signex, args: &CommandArgs) -> Ran {
    let board = match app.named_output_board(args) {
        Ok(board) => board,
        Err(reason) => return refused(reason),
    };
    let Some(path) = file(args) else {
        return refused("needs the operand `path`");
    };
    let (path, rule) = match command_target(app, &board, &path, "pdf", args) {
        Ok(found) => found,
        Err(ran) => return ran,
    };
    let options = match print_options(args) {
        Ok(options) => options,
        Err(reason) => return refused(reason),
    };
    let output = match app.board_pdf_of(&board, &options) {
        Ok(output) => output,
        Err(Some(why)) => return refused(why.render_en_us()),
        Err(None) => return refused("the board is not open"),
    };
    let pages = serde_json::to_value(&output.pages).unwrap_or(Value::Null);
    let warnings = super::print::pdf_warnings(&output);
    let answer = json!({
        "document": board.display().to_string(),
        "path": path.display().to_string(),
        "pages": pages,
        "warnings": warnings,
    });
    let file = CommandFile {
        id: "export_board_pdf",
        board,
        what: "PDF",
        success_log: super::print::pdf_written_log(&output, &path),
        bytes: output.bytes,
        warnings,
        path,
        rule,
        answer,
    };
    write_command_file(app, args, file)
}

/// `export_board_step document=<board> path=<file> [schema=ap214|ap242]`:
/// the board and its parts as a STEP model, made and written off the UI
/// thread; the answer says what is written where.
pub(crate) fn export_board_step(app: &mut Signex, args: &CommandArgs) -> Ran {
    let board = match app.named_output_board(args) {
        Ok(board) => board,
        Err(reason) => return refused(reason),
    };
    let Some(path) = file(args) else {
        return refused("needs the operand `path`");
    };
    let schema = match text(args, "schema") {
        None => StepSchema::default(),
        Some(id) => match StepSchema::from_id(id) {
            Some(schema) => schema,
            None => {
                let ids: Vec<&str> = StepSchema::ALL.iter().map(|s| s.id()).collect();
                return refused(format!("`schema` is one of {}", ids.join(", ")));
            }
        },
    };
    let answer = json!({
        "document": board.display().to_string(),
        "path": path.display().to_string(),
        "schema": schema.id(),
    });
    let task = app.build_board_step(&board, path, schema);
    (CommandOutcome::Answered(answer), task)
}
