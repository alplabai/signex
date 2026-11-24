//! Import DXF into a board sketch (#2794, mockup row 179c). The file is picked
//! and read off the UI thread, then the dialog shows its units, scale and
//! layers, and Next the profile check. Nothing changes until Heal and merge
//! or Import as is, which adds the geometry to the working copy as one sketch
//! undo step and shows what landed until Done; Cancel or Esc drops the
//! dialog.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_engine::pcb::dxf::DxfSource;
use signex_i18n::{RenderTranslatable as _, Translatable, ids, t};
use signex_sketch::import::ImportCheck;
use signex_sketch::plane::PlaneId;

use super::pcb_dxf_import::{file_label, read_text};
use crate::app::state::{DxfImportMode, SketchDxfDialog, SketchDxfMsg, SketchDxfStep};
use crate::app::{DxfImportMsg, Message, PcbMsg, Signex};
use crate::library::editor::footprint::land_sketch_import;
use crate::library::editor::footprint::sketch_redundancy_task::dispatch_redundancy;
use crate::library::editor::footprint::sketch_solve_task::dispatch_pending;

fn dxf(event: DxfImportMsg) -> Message {
    Message::Pcb(PcbMsg::DxfImport(event))
}

/// The message Esc sends while the dialog is open.
pub(crate) fn dialog_cancelled() -> Message {
    dxf(DxfImportMsg::SketchDialog(SketchDxfMsg::Cancelled))
}

/// Read and parse the picked file off the UI thread.
pub(super) fn read_sketch_dxf(board: PathBuf, path: PathBuf) -> Task<Message> {
    let reading = path.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let text = read_text(&reading)?;
                DxfSource::read(&text)
                    .map(Arc::new)
                    .map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| format!("the read did not finish: {e}"))
            .and_then(|source| source)
        },
        move |source| {
            dxf(DxfImportMsg::SketchRead {
                board,
                path,
                source,
            })
        },
    )
}

impl Signex {
    /// Import DXF… in the sketch bar, or the outline import while in sketch:
    /// ask for the file.
    pub(crate) fn request_sketch_dxf_file(&mut self) -> Task<Message> {
        let Some(board) = self.board_sketch_in_view().map(|s| s.board_path.clone()) else {
            return Task::none();
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
            dxf(DxfImportMsg::SketchPicked { board, path })
        })
    }

    /// The file was read: open the dialog, or say why it was not imported.
    /// A sketch that ended meanwhile takes nothing.
    pub(super) fn open_sketch_dxf(
        &mut self,
        board: &Path,
        path: &Path,
        source: Result<Arc<DxfSource>, String>,
    ) {
        let file = file_label(path);
        let source = match source {
            Ok(source) => source,
            Err(why) => {
                let line =
                    t!(self.ui_state.i18n, ids::PCB_DXF_REFUSED, "file" => file, "why" => why);
                crate::diagnostics::log_warning(line.into_string());
                return;
            }
        };
        let origin = self
            .document_state
            .pcb_board(board)
            .map(|b| b.options.origin)
            .unwrap_or_default();
        let Some(session) = self
            .interaction_state
            .board_sketch
            .as_mut()
            .filter(|s| s.board_path == board)
        else {
            return;
        };
        let has_outline = session
            .editor
            .primitive()
            .sketch
            .as_ref()
            .is_some_and(|s| s.entities.iter().any(|e| e.board_outline));
        let dialog = SketchDxfDialog::open(file, source, origin, has_outline);
        session.dxf_dialog = Some(Box::new(dialog));
    }

    pub(super) fn apply_sketch_dxf(&mut self, event: SketchDxfMsg) -> Task<Message> {
        let Some(session) = self.interaction_state.board_sketch.as_mut() else {
            return Task::none();
        };
        match event {
            SketchDxfMsg::Cancelled | SketchDxfMsg::Dismissed => session.dxf_dialog = None,
            SketchDxfMsg::Confirmed(mode) => return self.confirm_sketch_dxf(mode),
            event => {
                if let Some(dialog) = session.dxf_dialog.as_mut() {
                    dialog.apply(event);
                }
            }
        }
        Task::none()
    }

    /// Heal and merge, or Import as is, on the check: the geometry joins the
    /// working copy as one undo step and the card shows what landed. The
    /// board changes only when the sketch is baked.
    fn confirm_sketch_dxf(&mut self, mode: DxfImportMode) -> Task<Message> {
        let Some(session) = self.interaction_state.board_sketch.as_mut() else {
            return Task::none();
        };
        let Some(dialog) = session
            .dxf_dialog
            .as_deref()
            .filter(|d| d.step == SketchDxfStep::Checking)
        else {
            return Task::none();
        };
        let plane = session
            .editor
            .primitive()
            .sketch
            .as_ref()
            .and_then(|sketch| sketch.planes.first())
            .map_or_else(PlaneId::new, |plane| plane.id);
        let Ok(import) = dialog.build_as(plane, mode) else {
            return Task::none();
        };
        let check = import.check;
        let result = SketchDxfDialog::result_of(mode, &import);
        let file = dialog.file.clone();
        let landed = land_sketch_import(&mut session.editor, import, plane, |tag| {
            dialog.role_of(tag)
        });
        if landed {
            if let Some(dialog) = session.dxf_dialog.as_mut() {
                dialog.land(result);
            }
        } else {
            session.dxf_dialog = None;
        }
        let path = session.board_path.clone();
        let solve = dispatch_pending(&mut session.editor, &path).map(Message::Library);
        let redundancy = dispatch_redundancy(&mut session.editor, &path).map(Message::Library);
        log_outcome(&file, check, landed);
        self.refresh_board_sketch_preview();
        self.refresh_panel_ctx();
        Task::batch([solve, redundancy])
    }
}

/// Messages stays en-US whatever the UI language.
fn log_outcome(file: &str, check: ImportCheck, landed: bool) {
    if !landed {
        let line = Translatable::new(ids::BOARD_SKETCH_DXF_REJECTED).arg("file", file);
        crate::diagnostics::log_warning(line.render_en_us());
        return;
    }
    let line = Translatable::new(ids::BOARD_SKETCH_DXF_IMPORTED)
        .arg("file", file)
        .arg(
            "edges",
            (check.lines + check.arcs + check.circles).to_string(),
        )
        .arg("healed", check.healed.to_string())
        .arg("open", check.open_ends.to_string());
    crate::diagnostics::log_info(line.render_en_us());
}

#[cfg(test)]
#[path = "pcb_dxf_sketch_tests.rs"]
mod tests;
