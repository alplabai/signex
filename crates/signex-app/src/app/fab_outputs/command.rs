//! `fabrication_outputs` from its operands (#2470): the dialog's Generate for
//! the CLI and a running Signex, with no dialog. The same job, worker and
//! crash-safe writes as Generate; nothing on the board changes, so there is
//! no undo step. The caller is answered once the files are written
//! ([`CommandOutcome::Deferred`]), with each file's kind and SHA-256. Rules:
//! `docs/FABRICATION_OUTPUTS.md`, "From the command line".

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::{Translatable, ids};

use super::notes::problem_reason;
use super::write::{FileFacts, run_generate_listed};
use super::{
    FabOutput, FabOutputKinds, FabOutputsMsg, FabOutputsState, FabProblem, FabWritten, Listed,
    NoteSeverity, outcome_notes,
};
use crate::app::agent::WaitingCommand;
use crate::app::board_output::output_board;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// The `kinds` tokens in the dialog's order: the `.snxprj` keys of the
/// ticks. `odb_folder` is the ODB++ job as its folder tree.
const KIND_IDS: [&str; 10] = [
    "gerber",
    "drill",
    "centroid",
    "test_netlist",
    "ipc2581",
    "odb",
    "odb_folder",
    "assembly_pdf",
    "assembly_svg",
    "step",
];

/// A run waiting for its files: what its answer says besides them.
#[derive(Debug, Clone, PartialEq)]
pub struct FabCommandRun {
    waiting: WaitingCommand,
    board: PathBuf,
    name: String,
    kinds: FabOutputKinds,
    whole_panel: bool,
}

fn refused(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}

/// `fabrication_outputs document=<board> path=<folder> [kinds=<list>]
/// [name=<stem>] [whole_panel=<bool>]`: what Generate writes, from the board
/// in memory, into `path`. Without `kinds`, the kinds the dialog has ticked
/// for the board (its project's); without `name`, the board's file name;
/// a board with a panel writes the whole panel unless `whole_panel=false`.
pub(crate) fn fabrication_outputs(app: &mut Signex, args: &CommandArgs) -> Ran {
    let board = match output_board(app, args) {
        Ok(board) => board,
        Err(outcome) => return (outcome, Task::none()),
    };
    let Some(OperandValue::Path(folder)) = args.operand("path") else {
        return refused(Translatable::new(ids::FAB_OUTPUTS_COMMAND_NO_PATH));
    };
    let folder = PathBuf::from(folder);
    if let Err(reason) = crate::app::notebook_project::reserved_target(&folder) {
        return refused(reason);
    }
    let mut state = FabOutputsState::new(board.clone(), board_title(app, &board));
    let kinds = match args.operand("kinds") {
        Some(OperandValue::Text(list)) => match parse_kinds(list) {
            Ok(kinds) => kinds,
            Err(reason) => return refused(reason),
        },
        _ => app.fab_output_kinds(&state),
    };
    if let Some(OperandValue::Text(name)) = args.operand("name") {
        state.name = name.clone();
    }
    let has_panel = app
        .document_state
        .pcb_board(&board)
        .is_some_and(|pcb| pcb.options.panel.is_some());
    let whole_panel_off = matches!(args.operand("whole_panel"), Some(OperandValue::Bool(false)));
    state.whole_panel = has_panel && !whole_panel_off;
    // The copy the files are written from, designators as the project's
    // standard shows them (#1416), as Generate takes it.
    let Some(copy) = app
        .document_state
        .pcb_board_shown(&board)
        .map(Cow::into_owned)
    else {
        return refused(Translatable::new(ids::FAB_OUTPUTS_BOARD_GONE));
    };
    let mut job = app.generate_job(&state, kinds, copy);
    // An agent's files stay in its project's folder (#2470).
    let confine = app.agent_write_root(&board);
    job.folder = folder;
    let command = AppCommandId::new("fabrication_outputs").expect("a catalog id is valid");
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let run = FabCommandRun {
        waiting,
        board,
        name: state.name.trim().to_string(),
        kinds,
        whole_panel: state.whole_panel,
    };
    let task = Task::perform(
        crate::app::path_op_jobs::run_blocking(move || {
            crate::app::notebook_project::confined_write(confine.as_deref(), &job.folder)
                .map_err(FabProblem::WriteFailed)?;
            run_generate_listed(job)
        }),
        move |result| {
            Message::from(FabOutputsMsg::CommandWritten {
                run: Box::new(run),
                result: result
                    .map_err(FabProblem::WriteFailed)
                    .and_then(|listed| listed),
            })
        },
    );
    (CommandOutcome::Deferred { ticket }, task)
}

impl Signex {
    /// The files of `run` are written, or why not: the Messages line
    /// Generate logs, and the caller answered.
    pub(super) fn fab_command_written(
        &mut self,
        run: FabCommandRun,
        result: Result<Listed, FabProblem>,
    ) -> Task<Message> {
        let target = run.board.clone();
        let waiting = run.waiting.clone();
        self.finish_waiting_command(waiting, &target, move |app| {
            app.log_fab_outcome(result.as_ref().map(|(written, _)| written));
            match result {
                Ok((written, files)) => {
                    CommandOutcome::Answered(answer(app, &run, &written, &files))
                }
                Err(problem) => CommandOutcome::Disabled {
                    reason: problem_reason(&problem),
                },
            }
        })
    }
}

/// The open tab's title of `board`, as the dialog's subtitle and the
/// assembly PDF's title block print it; the file name when no tab shows it.
fn board_title(app: &Signex, board: &Path) -> String {
    app.document_state
        .tabs
        .iter()
        .find(|tab| tab.path == board)
        .map(|tab| tab.title.clone())
        .or_else(|| board.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

/// The kinds `list` names, comma separated.
fn parse_kinds(list: &str) -> Result<FabOutputKinds, Translatable> {
    let mut kinds = FabOutputKinds {
        gerber: false,
        drill: false,
        centroid: false,
        test_netlist: false,
        ipc2581: false,
        odb: false,
        odb_folder: false,
        assembly_pdf: false,
        assembly_svg: false,
        step: false,
    };
    for token in list.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let flag = match token {
            "gerber" => &mut kinds.gerber,
            "drill" => &mut kinds.drill,
            "centroid" => &mut kinds.centroid,
            "test_netlist" => &mut kinds.test_netlist,
            "ipc2581" => &mut kinds.ipc2581,
            "odb" => &mut kinds.odb,
            "odb_folder" => {
                kinds.odb = true;
                &mut kinds.odb_folder
            }
            "assembly_pdf" => &mut kinds.assembly_pdf,
            "assembly_svg" => &mut kinds.assembly_svg,
            "step" => &mut kinds.step,
            other => {
                return Err(Translatable::new(ids::FAB_OUTPUTS_COMMAND_BAD_KIND)
                    .arg("kind", other.to_string())
                    .arg("kinds", KIND_IDS.join(", ")));
            }
        };
        *flag = true;
    }
    if FabOutput::ALL.iter().any(|kind| kind.is_on(&kinds)) {
        Ok(kinds)
    } else {
        Err(Translatable::new(ids::FAB_OUTPUTS_COMMAND_NO_KIND))
    }
}

/// The `kinds` tokens of `kinds`, as the answer lists them.
fn kind_ids(kinds: &FabOutputKinds) -> Vec<&'static str> {
    let on = [
        kinds.gerber,
        kinds.drill,
        kinds.centroid,
        kinds.test_netlist,
        kinds.ipc2581,
        kinds.odb && !kinds.odb_folder,
        kinds.odb && kinds.odb_folder,
        kinds.assembly_pdf,
        kinds.assembly_svg,
        kinds.step,
    ];
    KIND_IDS
        .iter()
        .zip(on)
        .filter(|(_, on)| *on)
        .map(|(id, _)| *id)
        .collect()
}

/// The answer of a written run. Paths are the host's; a note is the
/// Messages wording without its bidi isolation marks.
fn answer(app: &Signex, run: &FabCommandRun, written: &FabWritten, files: &[FileFacts]) -> Value {
    let files: Vec<Value> = files
        .iter()
        .map(|file| {
            let path = file
                .name
                .split('/')
                .fold(written.folder.clone(), |path, part| path.join(part));
            json!({
                "name": file.name,
                "path": path.display().to_string(),
                "kind": file.kind,
                "bytes": file.bytes,
                "sha256": file.sha256,
            })
        })
        .collect();
    let notes: Vec<Value> = outcome_notes(written, &app.ui_state.i18n)
        .into_iter()
        .map(|(severity, line)| {
            let severity = match severity {
                NoteSeverity::Warning => "warning",
                NoteSeverity::Error => "error",
            };
            let text = line.into_string().replace(['\u{2068}', '\u{2069}'], "");
            json!({ "severity": severity, "text": text })
        })
        .collect();
    json!({
        "document": run.board.display().to_string(),
        "folder": written.folder.display().to_string(),
        "name": run.name,
        "kinds": kind_ids(&run.kinds),
        "whole_panel": run.whole_panel,
        "files": files,
        "notes": notes,
        "stale": written.stale,
    })
}
