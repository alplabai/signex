//! The board outputs of an Export Job (#1299, mockup row 114): Gerber, NC
//! drill, pick and place, IPC-D-356, IPC-2581, ODB++, the STEP model and the
//! assembly PDF, made by the Fabrication Outputs writer (`fab_outputs`) from
//! a copy of the project's board as it is in memory, one output at a time.
//! The writer takes no variant, so an output that lists parts is refused for
//! a variant other than the base one (docs/OUTPUT_JOBS.md, "Board outputs").

use std::path::{Path, PathBuf};

use signex_i18n::I18n;
use signex_renderer::board3d::StepSchema;
use signex_types::output_job::{OutputKind, PlannedOutput};
use signex_types::pcb::PcbBoard;
use signex_types::project::{FabOutputKinds, is_base_variant};

use crate::app::Signex;
use crate::app::fab_outputs::{
    DocumentsJob, GenerateJob, assembly_pdf_options, board_note_text, make_generate, note_text,
    problem_text, step_file_name,
};
use crate::app::handlers::menu::export::job_outputs::{JobFile, JobWork};

/// The writer's kinds for one board output: only `kind` on.
pub(crate) fn kinds_of(kind: OutputKind, odb_folder: bool) -> FabOutputKinds {
    let mut kinds = FabOutputKinds {
        gerber: false,
        drill: false,
        centroid: false,
        test_netlist: false,
        ipc2581: false,
        odb: false,
        odb_folder,
        assembly_pdf: false,
        assembly_svg: false,
        step: false,
    };
    match kind {
        OutputKind::Gerber => kinds.gerber = true,
        OutputKind::NcDrill => kinds.drill = true,
        OutputKind::PickAndPlace => kinds.centroid = true,
        OutputKind::TestPoints => kinds.test_netlist = true,
        OutputKind::Ipc2581 => kinds.ipc2581 = true,
        OutputKind::Odb => kinds.odb = true,
        OutputKind::Step => kinds.step = true,
        OutputKind::AssemblyPdf => kinds.assembly_pdf = true,
        OutputKind::SchematicPdf
        | OutputKind::SchematicPng
        | OutputKind::Bom
        | OutputKind::Netlist => {}
    }
    kinds
}

/// Whether the output lists the board's parts, which a variant changes.
fn lists_parts(kind: OutputKind) -> bool {
    matches!(
        kind,
        OutputKind::PickAndPlace
            | OutputKind::AssemblyPdf
            | OutputKind::Ipc2581
            | OutputKind::Odb
            | OutputKind::Step
    )
}

impl Signex {
    /// The board of the project the open sheet `source` belongs to: its path
    /// and a copy as it is in memory. `Err` when the project names none or
    /// it is not open.
    pub(crate) fn job_board(&self, source: &Path) -> Result<(PathBuf, PcbBoard), String> {
        let project = self
            .document_state
            .document_project(source)
            .ok_or_else(|| {
                "the job's sheet belongs to no open project, so it has no board".to_string()
            })?;
        let path = Self::project_board(project)
            .ok_or_else(|| format!("the project {} names no board", project.data.name))?;
        let board = self
            .document_state
            .pcb_board_shown(&path)
            .map(std::borrow::Cow::into_owned)
            .ok_or_else(|| format!("the board {} is not open: open it first", path.display()))?;
        Ok((path, board))
    }

    /// The work of the board output `output` in `variant`.
    pub(crate) fn board_job_work(
        &self,
        source: &Path,
        output: &PlannedOutput,
        variant: &str,
    ) -> Result<JobWork, String> {
        if lists_parts(output.kind) && !is_base_variant(variant) {
            return Err(format!(
                "{}: the board writer does not apply variants yet, so it cannot write variant {variant}",
                output.name
            ));
        }
        let (path, board) = self.job_board(source)?;
        let name = output.file.clone();
        let kinds = kinds_of(output.kind, output.odb_folder);
        let pdf = kinds.assembly_pdf.then(|| {
            let options = assembly_pdf_options(&self.ui_state.board_output.print_options);
            let title = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let text = crate::app::board_output::print_text(&self.ui_state.i18n, &title);
            (options, text)
        });
        let step = kinds.step.then(|| {
            self.step_context(
                &board,
                step_file_name(&name),
                name.clone(),
                StepSchema::default(),
            )
        });
        let whole_panel = board.options.panel.is_some();
        let job = GenerateJob {
            board,
            whole_panel,
            name,
            kinds,
            documents: DocumentsJob {
                pdf,
                svg: false,
                step,
            },
            folder: PathBuf::new(),
            designator_standard: self.document_state.designator_standard_for(&path),
            gerber: output.gerber.clone(),
        };
        Ok(JobWork::Board {
            output: output.name.clone(),
            i18n: self.ui_state.i18n.clone(),
            job: Box::new(job),
        })
    }
}

/// Make a board output's files: blocking, off the UI thread. The writer's
/// notes become the output's warnings.
pub(crate) fn render_board(
    output: String,
    i18n: &I18n,
    job: GenerateJob,
) -> Result<(Vec<JobFile>, Vec<String>), String> {
    let made = make_generate(job)
        .map_err(|problem| format!("{output}: {}", problem_text(&problem, i18n).into_string()))?;
    let mut warnings: Vec<String> = made
        .notes
        .into_iter()
        .map(|note| format!("{output}: {}", note_text(note, i18n).into_string()))
        .collect();
    warnings.extend(
        made.board_notes
            .iter()
            .map(|note| format!("{output}: {}", board_note_text(note, i18n).into_string())),
    );
    let files = made
        .files
        .into_iter()
        .map(|file| JobFile {
            output: output.clone(),
            file: file.name,
            bytes: file.bytes,
        })
        .collect();
    Ok((files, warnings))
}
