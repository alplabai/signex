//! DXF import onto a board (#2410): `import_dxf` puts a file's drawing on
//! the active layer of the board in view (or the `layer` operand's), and
//! `import_dxf_outline` makes its largest closed shape the board outline.
//! The file is picked and read off the UI thread; the import itself is one
//! board undo step built by `signex_engine::pcb::dxf`. A refusal changes
//! nothing and says why in the Messages panel. Design note:
//! `docs/BOARD_IMPORT_AND_REUSE.md`, "DXF import".

use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::pcb::dxf::{DxfImportSummary, DxfTarget, dxf_import_command};
use signex_i18n::{ids, t};
use signex_types::format::parse_pcb_layer;

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{DxfImportMsg, Message, PcbMsg, Signex};
use crate::pcb_canvas::layer_style;

type Ran = (CommandOutcome, Task<Message>);

impl Signex {
    pub(crate) fn handle_dxf_import(&mut self, event: DxfImportMsg) -> Task<Message> {
        match event {
            DxfImportMsg::Requested { outline } => self.request_dxf_file(outline),
            DxfImportMsg::Picked {
                board,
                target,
                path: Some(path),
            } => read_dxf(board, target, path),
            DxfImportMsg::Picked { path: None, .. } => Task::none(),
            DxfImportMsg::Read {
                board,
                target,
                path,
                text,
            } => {
                let _ = self.import_dxf_text(&board, target, &path, text);
                Task::none()
            }
        }
    }

    /// The command without operands: the file dialog for the board in view,
    /// aimed at its active layer or its outline.
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

/// Read the file off the UI thread. A file that is not UTF-8 is read
/// lossily: DXF text outside ASCII is only ever names and labels.
fn read_dxf(board: PathBuf, target: DxfTarget, path: PathBuf) -> Task<Message> {
    let reading = path.clone();
    Task::perform(
        async move {
            std::fs::read(&reading)
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .map_err(|e| e.to_string())
        },
        move |text| {
            Message::Pcb(PcbMsg::DxfImport(DxfImportMsg::Read {
                board,
                target,
                path,
                text,
            }))
        },
    )
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
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

fn operand_import(app: &mut Signex, args: &CommandArgs, id: &str, outline: bool) -> Ran {
    let refused = |reason: String| (CommandOutcome::Disabled { reason }, Task::none());
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
    (CommandOutcome::Ran, read_dxf(board, target, path))
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
