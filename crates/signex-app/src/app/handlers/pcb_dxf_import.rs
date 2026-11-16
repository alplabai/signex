//! DXF import onto a board (#2410): `import_dxf` puts a file's drawing on
//! the active layer of the board in view (or the `layer` operand's), and
//! `import_dxf_outline` makes its largest closed shape the board outline.
//! A copper layer is refused: a drawing there would be copper nothing
//! electrical reads. The file is picked and read off the UI thread; the
//! import itself is one board undo step built by `signex_engine::pcb::dxf`.
//! A refusal changes nothing and says why in the Messages panel; run with
//! operands, the command is answered with the import's summary or refusal
//! once the file is read. Design note: `docs/BOARD_IMPORT_AND_REUSE.md`,
//! "DXF import".

use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::pcb::dxf::{DxfImportSummary, DxfTarget, check_dxf_target, dxf_import_command};
use signex_i18n::{ids, t};
use signex_types::format::parse_pcb_layer;

use crate::app::agent::WaitingCommand;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{DxfImportMsg, Message, PcbMsg, Signex};
use crate::ignore::IgnoreResult;
use crate::keymap::AppCommandId;
use crate::pcb_canvas::layer_style;

type Ran = (CommandOutcome, Task<Message>);

/// The largest file an import reads: far past any board drawing, and a
/// stray pick of a huge file is refused instead of read into memory.
const MAX_DXF_BYTES: u64 = 64 * 1024 * 1024;

impl Signex {
    pub(crate) fn handle_dxf_import(&mut self, event: DxfImportMsg) -> Task<Message> {
        match event {
            DxfImportMsg::Requested { outline } => self.request_dxf_file(outline),
            DxfImportMsg::Picked {
                board,
                target,
                path: Some(path),
            } => read_dxf(board, target, path, None),
            DxfImportMsg::Picked { path: None, .. } => Task::none(),
            DxfImportMsg::Read {
                board,
                target,
                path,
                text,
                waiting: None,
            } => {
                self.import_dxf_text(&board, target, &path, text)
                    .ignore("the dialog has no caller to answer; the outcome is in Messages");
                Task::none()
            }
            DxfImportMsg::Read {
                board,
                target,
                path,
                text,
                waiting: Some(waiting),
            } => self.finish_waiting_command(waiting, &board, |app| {
                answer(app.import_dxf_text(&board, target, &path, text))
            }),
        }
    }

    /// The command without operands: the file dialog for the board in view,
    /// aimed at its active layer or its outline. A copper active layer is
    /// refused before the dialog opens.
    fn request_dxf_file(&mut self, outline: bool) -> Task<Message> {
        if !self.has_active_pcb() {
            return Task::none();
        }
        let Some(board) = self.active_tab_path() else {
            return Task::none();
        };
        let target = if outline {
            DxfTarget::Outline
        } else {
            DxfTarget::Layer(self.pcb_layer_view(&board).active())
        };
        let i18n = &self.ui_state.i18n;
        if let Err(why) = check_dxf_target(target) {
            let line = t!(i18n, ids::PCB_DXF_LAYER_REFUSED, "why" => why.to_string());
            crate::diagnostics::log_warning(line.into_string());
            return Task::none();
        }
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::FILE_DIALOG_IMPORT_DXF)
            .filter_in(i18n, ids::FILE_FILTER_DXF, &["dxf"])
            .filter_in(i18n, ids::FILE_FILTER_ALL_FILES, &["*"]);
        Task::perform(dialog.pick_file(), move |path| {
            Message::Pcb(PcbMsg::DxfImport(DxfImportMsg::Picked {
                board,
                target,
                path,
            }))
        })
    }

    /// Import the read `text` of `path` onto `board` as one undo step. The
    /// summary on success; the reason, also logged, on a refusal.
    pub(crate) fn import_dxf_text(
        &mut self,
        board: &Path,
        target: DxfTarget,
        path: &Path,
        text: Result<String, String>,
    ) -> Result<DxfImportSummary, String> {
        let file = file_label(path);
        let result = self.run_dxf_import(board, target, text);
        let i18n = &self.ui_state.i18n;
        match &result {
            Ok(summary) => {
                let line = match target {
                    DxfTarget::Layer(layer) => t!(
                        i18n,
                        ids::PCB_DXF_IMPORTED_LAYER,
                        "count" => summary.graphics,
                        "file" => file.clone(),
                        "layer" => layer_style::layer_name(i18n, layer).into_string()
                    ),
                    DxfTarget::Outline => t!(
                        i18n,
                        ids::PCB_DXF_IMPORTED_OUTLINE,
                        "count" => summary.outline_points,
                        "file" => file.clone()
                    ),
                };
                crate::diagnostics::log_info(line.into_string());
                if summary.skipped > 0 {
                    let line = t!(
                        i18n,
                        ids::PCB_DXF_SKIPPED,
                        "count" => summary.skipped,
                        "file" => file
                    );
                    crate::diagnostics::log_info(line.into_string());
                }
            }
            Err(why) => {
                let line = t!(i18n, ids::PCB_DXF_REFUSED, "file" => file, "why" => why.clone());
                crate::diagnostics::log_warning(line.into_string());
            }
        }
        result
    }

    fn run_dxf_import(
        &mut self,
        board: &Path,
        target: DxfTarget,
        text: Result<String, String>,
    ) -> Result<DxfImportSummary, String> {
        let text = text?;
        let i18n = &self.ui_state.i18n;
        let label = t!(i18n, ids::PCB_DXF_IMPORT_STEP).into_string();
        let Some(engine) = self.document_state.board_engine(board) else {
            return Err(t!(i18n, ids::PCB_PRIM_BOARD_GONE, "what" => label.clone()).into_string());
        };
        if engine.board().unknown.newer_minor_source.is_some() {
            return Err(t!(i18n, ids::PCB_PRIM_READ_ONLY).into_string());
        }
        let revision = engine.revision();
        let (command, summary) =
            dxf_import_command(engine.board(), &text, target).map_err(|e| e.to_string())?;
        let step = CheckedStep {
            label: &label,
            expected_revision: revision,
        };
        self.execute_board_commands(board, vec![command], Some(step))
            .map_err(|e| e.to_string())?;
        Ok(summary)
    }
}

/// The outcome a command run with operands answers with: the summary, or
/// why nothing was imported.
fn answer(result: Result<DxfImportSummary, String>) -> CommandOutcome {
    match result.and_then(|s| serde_json::to_value(s).map_err(|e| e.to_string())) {
        Ok(summary) => CommandOutcome::Answered(summary),
        Err(reason) => CommandOutcome::Disabled { reason },
    }
}

/// Read the file off the UI thread. A file that is not UTF-8 is read
/// lossily: DXF text outside ASCII is only ever names and labels.
fn read_dxf(
    board: PathBuf,
    target: DxfTarget,
    path: PathBuf,
    waiting: Option<WaitingCommand>,
) -> Task<Message> {
    let reading = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || read_text(&reading))
                .await
                .map_err(|e| format!("the read did not finish: {e}"))
                .and_then(|text| text)
        },
        move |text| {
            Message::Pcb(PcbMsg::DxfImport(DxfImportMsg::Read {
                board,
                target,
                path,
                text,
                waiting,
            }))
        },
    )
}

fn read_text(path: &Path) -> Result<String, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_DXF_BYTES {
        return Err(format!(
            "the file is {size} bytes, more than the {MAX_DXF_BYTES} a DXF import reads"
        ));
    }
    std::fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .map_err(|e| e.to_string())
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The command run without operands: the file dialog, for the outline or
/// the active layer.
pub(crate) fn requested(outline: bool) -> Message {
    Message::Pcb(PcbMsg::DxfImport(DxfImportMsg::Requested { outline }))
}

/// `import_dxf path=.. [layer=..] [document=..]`: the file onto `layer` (a
/// layer token; the active layer when left out) of the named board, else
/// the board in view.
pub(crate) fn import_dxf(app: &mut Signex, args: &CommandArgs) -> Ran {
    operand_import(app, args, "import_dxf", false)
}

/// `import_dxf_outline path=.. [document=..]`: the file's largest closed
/// shape as the board outline.
pub(crate) fn import_dxf_outline(app: &mut Signex, args: &CommandArgs) -> Ran {
    operand_import(app, args, "import_dxf_outline", true)
}

/// Checked up front, then answered once the file is read
/// ([`CommandOutcome::Deferred`]): the summary, or why nothing changed.
fn operand_import(app: &mut Signex, args: &CommandArgs, id: &str, outline: bool) -> Ran {
    let refused = |reason: String| (CommandOutcome::Disabled { reason }, Task::none());
    let command = match AppCommandId::new(id) {
        Ok(command) => command,
        Err(reason) => return refused(reason),
    };
    let path = match args.operand("path") {
        Some(OperandValue::Path(path)) => PathBuf::from(path),
        _ => return refused(format!("`{id}` needs the operand `path`")),
    };
    let board = match operand_board(app, args, id) {
        Ok(board) => board,
        Err(reason) => return refused(reason),
    };
    let target = if outline {
        DxfTarget::Outline
    } else {
        match args.operand("layer") {
            Some(OperandValue::Text(token)) => match parse_pcb_layer(token) {
                Some(layer) => DxfTarget::Layer(layer),
                None => return refused(format!("`{id}`: {token:?} is not a board layer")),
            },
            _ => DxfTarget::Layer(app.pcb_layer_view(&board).active()),
        }
    };
    if let Err(why) = check_dxf_target(target) {
        return refused(format!("`{id}`: {why}"));
    }
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let task = read_dxf(board, target, path, Some(waiting));
    (CommandOutcome::Deferred { ticket }, task)
}

/// The board the `document` operand names, else the board in view.
fn operand_board(app: &Signex, args: &CommandArgs, id: &str) -> Result<PathBuf, String> {
    let Some(OperandValue::Document(named)) = args.operand("document") else {
        return match (app.has_active_pcb(), app.active_tab_path()) {
            (true, Some(path)) => Ok(path),
            _ => Err(format!(
                "`{id}` needs a board in view or the operand `document`"
            )),
        };
    };
    let entry = app.resolve_document_target(named)?;
    if entry.kind != DocumentKind::Pcb {
        return Err(format!(
            "`{id}` imports onto a board, and {} is not one",
            entry.name
        ));
    }
    let path = entry
        .path
        .ok_or_else(|| format!("{} has no file", entry.name))?;
    if app.document_state.board_engine(&path).is_none() {
        return Err(format!("`{id}`: {} is not open", entry.name));
    }
    Ok(path)
}

#[cfg(test)]
#[path = "pcb_dxf_import_tests.rs"]
mod tests;
