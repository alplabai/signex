//! The blocking half of a block diagram export: read the `.snxblk`, draw
//! it, write the SVG or PDF crash-safely. Plain data in and out, no
//! `Signex` state, run on the blocking pool; the result goes back to
//! `update` as a message.

use std::path::{Path, PathBuf};

use signex_output::{
    BlockDiagramFormat, BlockDiagramLook, BlockDiagramPdfOptions, BlockDiagramPdfText,
    block_diagram_drawing, block_diagram_pdf, block_diagram_svg,
};
use signex_types::block_diagram::read_block_diagram;

/// The largest diagram an export reads: far past any real one, so a stray
/// pick of a huge file is refused instead of read into memory.
const MAX_DIAGRAM_BYTES: u64 = 64 * 1024 * 1024;

/// One export: which diagram, where to, and how it looks.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportJob {
    pub diagram: PathBuf,
    pub path: PathBuf,
    pub format: BlockDiagramFormat,
    pub look: BlockDiagramLook,
    pub pdf: BlockDiagramPdfOptions,
    /// The PDF title block's words, in the user's language.
    pub text: BlockDiagramPdfText,
}

/// What a written export holds, for the Messages line and the answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Exported {
    pub blocks: usize,
    pub connections: usize,
    pub groups: usize,
    /// Bytes written.
    pub bytes: usize,
    /// The diagram's extent, diagram mm: `[min x, min y, max x, max y]`.
    pub extent_mm: [f64; 4],
    /// Page mm per diagram mm on the PDF page; `None` for SVG (1:1).
    pub scale: Option<f64>,
    /// Characters no bundled font covers (PDF only), sorted and unique.
    pub uncovered_chars: Vec<char>,
    /// The diagram is from a newer minor and opens read-only; it still
    /// exports.
    pub read_only: bool,
}

/// Run `job`: the reason text when nothing was written.
pub fn run(job: &ExportJob) -> Result<Exported, String> {
    let size = std::fs::metadata(&job.diagram)
        .map_err(|e| e.to_string())?
        .len();
    if size > MAX_DIAGRAM_BYTES {
        return Err(format!(
            "the file is {size} bytes, more than the {MAX_DIAGRAM_BYTES} an export reads"
        ));
    }
    let read = read_block_diagram(&job.diagram).map_err(|e| e.to_string())?;
    let diagram = &read.diagram;
    let drawing = block_diagram_drawing(diagram, &job.look);
    let (bytes, scale, uncovered_chars) = match job.format {
        BlockDiagramFormat::Svg => (block_diagram_svg(&drawing).into_bytes(), None, Vec::new()),
        BlockDiagramFormat::Pdf => {
            let out =
                block_diagram_pdf(&drawing, &job.pdf, &job.text).map_err(|e| e.to_string())?;
            (out.bytes, Some(out.scale), out.uncovered_chars)
        }
    };
    signex_types::atomic_io::atomic_write(&job.path, &bytes).map_err(|e| e.to_string())?;
    Ok(Exported {
        blocks: diagram.blocks.len(),
        connections: diagram.connections.len(),
        groups: diagram.groups.len(),
        bytes: bytes.len(),
        extent_mm: [drawing.min.x, drawing.min.y, drawing.max.x, drawing.max.y],
        scale,
        uncovered_chars,
        read_only: read.is_read_only(),
    })
}

/// Whether `path` names a block diagram file by its extension, any case.
pub fn is_block_diagram(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ext.eq_ignore_ascii_case(signex_types::block_diagram::BLOCK_DIAGRAM_EXTENSION)
        })
}
