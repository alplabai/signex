//! The Generate job: build the files from a copy of the board and write
//! them, each crash-safely. Runs on a worker, never inside `update`.

use std::path::{Path, PathBuf};

use signex_output::fab::{FabExtras, FabNote, FabOptions, check_job, generate};
use signex_types::pcb::PcbBoard;

use super::documents::{DocumentsJob, MadeFile, make_documents};
use super::{BoardNote, FabKinds, FabProblem};

/// The extensions of the files Generate writes; a file of the same stem
/// with one of them, left by an earlier run, is reported as stale.
const FAB_EXTENSIONS: [&str; 10] = [
    "gbr", "drl", "csv", "ipc", "gbrjob", "xml", "tar", "pdf", "svg", "step",
];

/// What one Generate wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FabWritten {
    /// The files written: the fabrication files in the order the job file
    /// lists them, then the assembly drawings and the STEP model.
    pub files: Vec<String>,
    pub folder: PathBuf,
    pub notes: Vec<FabNote>,
    /// What the assembly drawings and the STEP model leave out (#2489).
    pub board_notes: Vec<BoardNote>,
    /// Files of the same name stem already in the folder that this run did
    /// not write (a layer or a kind dropped since), sorted. They are left
    /// in place, never deleted, and reported.
    pub stale: Vec<String>,
}

/// Everything one Generate makes, taken in `update` and moved to the
/// worker.
pub(crate) struct GenerateJob {
    pub board: PcbBoard,
    /// Write the board's panel (#2410) when it has one.
    pub whole_panel: bool,
    pub name: String,
    pub kinds: FabKinds,
    pub documents: DocumentsJob,
    pub folder: PathBuf,
}

/// Run `job`: make every file first, then write them all; when one cannot
/// be made, nothing is written.
pub(crate) fn run_generate(job: GenerateJob) -> Result<FabWritten, FabProblem> {
    let board = fab_board(job.board, job.whole_panel)?;
    let name = check_job(&board, &job.name)
        .map_err(FabProblem::Refused)?
        .to_string();
    let mut made = Vec::new();
    let notes = if job.kinds.has_fab_files() {
        fab_files(&board, &job.kinds.fab_options(&name), &mut made)?
    } else {
        Vec::new()
    };
    let mut board_notes = Vec::new();
    made.extend(make_documents(
        &board,
        &name,
        job.documents,
        &mut board_notes,
    )?);
    write_made(made, &job.folder, &name, notes, board_notes)
}

/// Build `board`'s fabrication files and write them into `folder`,
/// creating it. A file of the same name is replaced. The copper images hold
/// the zones' copper, pour fills included (#2216), as the canvas and the
/// board PDF draw it; board and footprint text is written in the board
/// stroke font (#2368).
#[cfg(test)]
pub(crate) fn write_fab_outputs(
    board: &PcbBoard,
    options: &FabOptions,
    folder: &Path,
) -> Result<FabWritten, FabProblem> {
    let mut made = Vec::new();
    let notes = fab_files(board, options, &mut made)?;
    write_made(made, folder, options.name.trim(), notes, Vec::new())
}

/// The fabrication files of `board` into `made`; their notes.
fn fab_files(
    board: &PcbBoard,
    options: &FabOptions,
    made: &mut Vec<MadeFile>,
) -> Result<Vec<FabNote>, FabProblem> {
    let extras = FabExtras::of_board(board);
    let set = generate(board, &extras, options).map_err(FabProblem::Refused)?;
    made.extend(set.files.into_iter().map(|file| MadeFile {
        name: file.name,
        bytes: file.contents.into_bytes(),
    }));
    Ok(set.notes)
}

/// Write `made` into `folder`, creating it, each file crash-safely, and
/// list the older files of stem `name` beside them.
fn write_made(
    made: Vec<MadeFile>,
    folder: &Path,
    name: &str,
    notes: Vec<FabNote>,
    board_notes: Vec<BoardNote>,
) -> Result<FabWritten, FabProblem> {
    std::fs::create_dir_all(folder)
        .map_err(|e| FabProblem::WriteFailed(format!("{}: {e}", folder.display())))?;
    let mut files = Vec::with_capacity(made.len());
    for file in made {
        let path = folder.join(&file.name);
        signex_types::atomic_io::atomic_write(&path, &file.bytes)
            .map_err(|e| FabProblem::WriteFailed(format!("{}: {e}", path.display())))?;
        files.push(file.name);
    }
    // Every file is written by now, so a folder that cannot be listed is
    // logged, not turned into a failed write.
    let stale = stale_files(folder, name, &files).unwrap_or_else(|e| {
        crate::diagnostics::log_warning(format!("{}: {e}", folder.display()));
        Vec::new()
    });
    Ok(FabWritten {
        files,
        folder: folder.to_path_buf(),
        notes,
        board_notes,
        stale,
    })
}

/// The board the files are made from: its whole panel (#2410) when asked
/// and the board has one, else the board itself.
fn fab_board(board: PcbBoard, whole_panel: bool) -> Result<PcbBoard, FabProblem> {
    match board.options.panel {
        Some(panel) if whole_panel => pcb_geom::panel::panel_board(&board, &panel)
            .map_err(|e| FabProblem::PanelRefused(e.to_string())),
        _ => Ok(board),
    }
}

/// The `<stem>-*` fabrication files in `folder` that are not in `written`.
fn stale_files(folder: &Path, stem: &str, written: &[String]) -> std::io::Result<Vec<String>> {
    let prefix = format!("{stem}-");
    let mut stale = Vec::new();
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let fab_extension = Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| FAB_EXTENSIONS.contains(&e));
        if fab_extension && name.starts_with(&prefix) && !written.contains(&name) {
            stale.push(name);
        }
    }
    stale.sort();
    Ok(stale)
}
