//! The board documents Generate writes beside the fabrication files
//! (#2489): the assembly drawings as one PDF and as an SVG file per side,
//! the same drawings Board Print / PDF and Export Assembly SVG make, and the
//! board STEP model File ▸ Export ▸ STEP 3D Model makes. Made on the worker.

use signex_output::{
    AssemblySvgOptions, BoardLayerSet, BoardPrintOptions, BoardPrintPages, BoardPrintText,
    assembly_drawing, assembly_svg, board_pdf,
};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::BoardSide;

use super::{BoardNote, FabProblem};
use crate::app::board_output::{StepContext, make_step_of};

/// Which documents to make, with what `update` read for them.
pub(crate) struct DocumentsJob {
    /// The assembly PDF's options and the words its title blocks print;
    /// `None` makes none.
    pub pdf: Option<(BoardPrintOptions, BoardPrintText)>,
    pub svg: bool,
    /// What the STEP model is made from; `None` makes none.
    pub step: Option<StepContext>,
}

/// One made file: its name in the output folder, its kind as the
/// `fabrication_outputs` answer names it (`gerber`, `assembly_pdf`, ...)
/// and its bytes.
pub(crate) struct MadeFile {
    pub name: String,
    pub kind: &'static str,
    pub bytes: Vec<u8>,
}

/// The assembly PDF's options: both sides' drawings, a page each, on the
/// page Board Print / PDF was last used with.
pub(crate) fn assembly_pdf_options(last: &BoardPrintOptions) -> BoardPrintOptions {
    BoardPrintOptions {
        sets: vec![BoardLayerSet::AssemblyTop, BoardLayerSet::AssemblyBottom],
        pages: BoardPrintPages::PerSet,
        ..last.clone()
    }
}

/// `<name>-board.step`.
pub(crate) fn step_file_name(name: &str) -> String {
    format!("{name}-board.step")
}

fn svg_file_name(name: &str, side: BoardSide) -> String {
    let side = match side {
        BoardSide::Top => "top",
        BoardSide::Bottom => "bottom",
    };
    format!("{name}-assembly-{side}.svg")
}

/// Make the documents `job` asks for from `board`, files named `name`;
/// what they leave out goes to `notes`. Nothing is written here.
pub(crate) fn make_documents(
    board: &PcbBoard,
    name: &str,
    job: DocumentsJob,
    notes: &mut Vec<BoardNote>,
) -> Result<Vec<MadeFile>, FabProblem> {
    let mut files = Vec::new();
    if let Some((options, text)) = &job.pdf {
        let pdf = board_pdf(board, options, text)
            .map_err(|e| FabProblem::AssemblyRefused(e.to_string()))?;
        if !pdf.uncovered_chars.is_empty() {
            notes.push(BoardNote::CharsNotCovered(
                pdf.uncovered_chars.iter().collect(),
            ));
        }
        files.push(MadeFile {
            name: format!("{name}-assembly.pdf"),
            kind: "assembly_pdf",
            bytes: pdf.bytes,
        });
    }
    if job.svg {
        for side in [BoardSide::Top, BoardSide::Bottom] {
            let look = AssemblySvgOptions {
                mirrored: side == BoardSide::Bottom,
                monochrome: false,
            };
            let drawing = assembly_drawing(board, side);
            files.push(MadeFile {
                name: svg_file_name(name, side),
                kind: "assembly_svg",
                bytes: assembly_svg(&drawing, &look).into_bytes(),
            });
        }
    }
    if let Some(context) = job.step {
        let built = make_step_of(board, context).map_err(FabProblem::StepRefused)?;
        if built.step.dropped_holes > 0 {
            notes.push(BoardNote::HolesLeftOut(built.step.dropped_holes));
        }
        notes.extend(
            built
                .unread_models
                .into_iter()
                .map(|(source, why)| BoardNote::ModelUnread { source, why }),
        );
        files.push(MadeFile {
            name: step_file_name(name),
            kind: "step",
            bytes: built.step.bytes,
        });
    }
    Ok(files)
}
