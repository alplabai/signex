//! The Generate job: build the files from a copy of the board and write
//! them, each crash-safely. Runs on a worker, never inside `update`.

use std::path::Path;

use signex_output::fab::{FabExtras, FabNote, FabOptions, generate};
use signex_types::pcb::PcbBoard;

use super::FabProblem;

/// What one Generate wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FabWritten {
    /// The files written, in the order the job file lists them.
    pub files: Vec<String>,
    pub folder: std::path::PathBuf,
    pub notes: Vec<FabNote>,
}

/// Build `board`'s files and write them into `folder`, creating it. A file
/// of the same name is replaced. Zone fills (#2216) and board text strokes
/// (#2387) join through [`FabExtras`] once those land; until then the
/// notes say what is left out.
pub(crate) fn write_fab_outputs(
    board: &PcbBoard,
    options: &FabOptions,
    folder: &Path,
) -> Result<FabWritten, FabProblem> {
    let set = generate(board, &FabExtras::default(), options).map_err(FabProblem::Refused)?;
    std::fs::create_dir_all(folder)
        .map_err(|e| FabProblem::WriteFailed(format!("{}: {e}", folder.display())))?;
    let mut files = Vec::with_capacity(set.files.len());
    for file in &set.files {
        let path = folder.join(&file.name);
        signex_types::atomic_io::atomic_write(&path, file.contents.as_bytes())
            .map_err(|e| FabProblem::WriteFailed(format!("{}: {e}", path.display())))?;
        files.push(file.name.clone());
    }
    Ok(FabWritten {
        files,
        folder: folder.to_path_buf(),
        notes: set.notes,
    })
}
