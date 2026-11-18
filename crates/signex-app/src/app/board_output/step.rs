//! The board STEP export (#2488): ask where to save, make the model off the
//! update thread (reading the parts' models from their libraries and
//! running the MCAD kernel both take a while), then write it through the
//! export write path.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_i18n::{Translatable, ids};
use signex_renderer::board3d::{BoardStep, BoardStepInput, StepHeader, StepSchema, board_step};
use signex_types::part::FootprintSourceRef;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::BoardOutputMsg;
use crate::app::handlers::board_3d::read_models;
use crate::app::{Message, Signex};

/// A made STEP model, and the parts' models that could not be read (each
/// drawn as its plain body instead): the source as text, and why.
#[derive(Debug, PartialEq)]
pub struct BoardStepBuilt {
    pub step: BoardStep,
    pub unread_models: Vec<(String, String)>,
}

/// What the model is made from, taken in `update` and moved to the
/// blocking task.
struct StepJob {
    board: PcbBoard,
    sources: Vec<FootprintSourceRef>,
    open_libraries: Vec<(Uuid, PathBuf)>,
    schema: StepSchema,
    header: StepHeader,
    product: String,
}

impl Signex {
    /// The save dialog for the STEP model of the board in view.
    pub(super) fn request_board_step(&mut self) -> Task<Message> {
        let Some((board, _)) = self.board_in_view() else {
            self.explain_no_board();
            return Task::none();
        };
        let target = self.save_target(Some(&board));
        let picker = self.ui_state.io.picker.clone();
        let i18n = self.ui_state.i18n.clone();
        let file_name = target.file_name("board", "", "step");
        Task::perform(
            async move {
                target
                    .dialog(
                        picker
                            .dialog()
                            .title_in(&i18n, ids::BOARD_STEP_EXPORT_TITLE),
                    )
                    .await
                    .filter_in(&i18n, ids::FILE_FILTER_STEP_EXT, &["step", "stp"])
                    .set_file_name(file_name)
                    .save_file()
                    .await
            },
            move |path| match path {
                Some(path) => Message::from(BoardOutputMsg::StepPathChosen { board, path }),
                None => Message::noop(),
            },
        )
    }

    /// Make the STEP model of the open board at `board`, as it is now, off
    /// the update thread; it comes back as `StepBuilt` for `path`.
    pub(super) fn build_board_step(
        &mut self,
        board: &Path,
        path: PathBuf,
        schema: StepSchema,
    ) -> Task<Message> {
        let Some(job) = self.board_step_job(board, &path, schema) else {
            self.refuse_board_gone(board);
            return Task::none();
        };
        let board = board.to_path_buf();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || make_board_step(job))
                    .await
                    .unwrap_or_else(|e| Err(format!("the STEP build stopped: {e}")))
            },
            move |built| {
                Message::from(BoardOutputMsg::StepBuilt {
                    board: board.clone(),
                    path: path.clone(),
                    built,
                })
            },
        )
    }

    /// The inputs of the STEP model of the open board at `board`; `None`
    /// when it is not open.
    fn board_step_job(&self, board: &Path, path: &Path, schema: StepSchema) -> Option<StepJob> {
        let pcb = self.document_state.pcb_board(board)?.clone();
        let mut seen = HashSet::new();
        let sources: Vec<FootprintSourceRef> = pcb
            .footprints
            .iter()
            .filter_map(|f| f.library_ref.clone())
            .filter(|source| seen.insert(source.clone()))
            .collect();
        let open_libraries = self
            .library
            .open_libraries
            .iter()
            .map(|l| (l.library_id, l.root.clone()))
            .collect();
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Some(StepJob {
            board: pcb,
            sources,
            open_libraries,
            schema,
            header: StepHeader {
                file_name,
                timestamp: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                system: format!("Signex {}", env!("CARGO_PKG_VERSION")),
            },
            product: super::file_title(board),
        })
    }

    /// Write a made STEP model to `path`, or say why it could not be made.
    pub(super) fn write_board_step(
        &mut self,
        board: &Path,
        path: PathBuf,
        built: Result<Arc<BoardStepBuilt>, String>,
    ) -> Task<Message> {
        let built = match built {
            Ok(built) => built,
            Err(why) => {
                self.report_export_failure(
                    Translatable::new(ids::BOARD_STEP_FAILED).arg("why", why),
                );
                return Task::none();
            }
        };
        let warnings = step_warnings(&built);
        let log = format!(
            "Wrote {} (STEP model of {}, {} part(s), {} bytes)",
            path.display(),
            super::file_title(board),
            built.step.parts,
            built.step.bytes.len()
        );
        let bytes = Arc::try_unwrap(built)
            .map(|built| built.step.bytes)
            .unwrap_or_else(|shared| shared.step.bytes.clone());
        self.write_export(path, bytes, "STEP", log, None, warnings)
    }
}

/// Read the parts' models and make the STEP model. Blocking: IO and kernel
/// work.
fn make_board_step(job: StepJob) -> Result<Arc<BoardStepBuilt>, String> {
    let read = read_models(&job.sources, &job.open_libraries);
    let models: HashMap<_, _> = read.models.into_iter().collect();
    let step = board_step(&BoardStepInput {
        board: &job.board,
        models: &models,
        schema: job.schema,
        header: job.header,
        product: job.product,
    })
    .map_err(|e| e.to_string())?;
    Ok(Arc::new(BoardStepBuilt {
        step,
        unread_models: read.failures,
    }))
}

/// What a made STEP model warns of: holes left out and models not read.
pub(super) fn step_warnings(built: &BoardStepBuilt) -> Vec<String> {
    let dropped = built.step.dropped_holes;
    let holes = (dropped > 0).then(|| {
        format!(
            "STEP export: {dropped} hole(s) left out of the board body, each meets a hole cut before it"
        )
    });
    let unread = built.unread_models.iter().map(|(source, why)| {
        format!(
            "STEP export: the model of {source} could not be read ({why}); its plain body is used"
        )
    });
    holes.into_iter().chain(unread).collect()
}
