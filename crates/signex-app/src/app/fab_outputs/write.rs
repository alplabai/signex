//! The Generate job: build the files from a copy of the board and write
//! them, each crash-safely. Runs on a worker, never inside `update`.

use std::path::Path;

use signex_output::fab::{FabExtras, FabNote, FabOptions, generate};
use signex_types::pcb::PcbBoard;

use super::FabProblem;

/// The extensions of the files Generate writes; a file of the same stem
/// with one of them, left by an earlier run, is reported as stale.
const FAB_EXTENSIONS: [&str; 5] = ["gbr", "drl", "csv", "ipc", "gbrjob"];

/// What one Generate wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FabWritten {
    /// The files written, in the order the job file lists them.
    pub files: Vec<String>,
    pub folder: std::path::PathBuf,
    pub notes: Vec<FabNote>,
    /// Files of the same name stem already in the folder that this run did
    /// not write (a layer or a kind dropped since), sorted. They are left
    /// in place, never deleted, and reported.
    pub stale: Vec<String>,
}

/// Build `board`'s files and write them into `folder`, creating it. A file
/// of the same name is replaced. The copper images hold the zones' copper,
/// pour fills included (#2216), as the canvas and the board PDF draw it;
/// board text strokes (#2387) are not drawn yet, and the notes say so.
pub(crate) fn write_fab_outputs(
    board: &PcbBoard,
    options: &FabOptions,
    folder: &Path,
) -> Result<FabWritten, FabProblem> {
    let extras = FabExtras::of_board(board);
    let set = generate(board, &extras, options).map_err(FabProblem::Refused)?;
    std::fs::create_dir_all(folder)
        .map_err(|e| FabProblem::WriteFailed(format!("{}: {e}", folder.display())))?;
    let mut files = Vec::with_capacity(set.files.len());
    for file in &set.files {
        let path = folder.join(&file.name);
        signex_types::atomic_io::atomic_write(&path, file.contents.as_bytes())
            .map_err(|e| FabProblem::WriteFailed(format!("{}: {e}", path.display())))?;
        files.push(file.name.clone());
    }
    // Every file is written by now, so a folder that cannot be listed is
    // logged, not turned into a failed write.
    let stale = stale_files(folder, options.name.trim(), &files).unwrap_or_else(|e| {
        crate::diagnostics::log_warning(format!("{}: {e}", folder.display()));
        Vec::new()
    });
    Ok(FabWritten {
        files,
        folder: folder.to_path_buf(),
        notes: set.notes,
        stale,
    })
}

/// The board the files are made from: its whole panel (#2410) when asked
/// and the board has one, else the board itself.
pub(crate) fn fab_board(board: PcbBoard, whole_panel: bool) -> Result<PcbBoard, FabProblem> {
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
