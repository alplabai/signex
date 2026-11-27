//! The Generate job: build the files from a copy of the board and write
//! them, each crash-safely. Runs on a worker, never inside `update`.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use signex_output::fab::{FabExtras, FabFileKind, FabNote, FabOptions, check_job, generate};
use signex_types::pcb::PcbBoard;

use super::documents::{DocumentsJob, MadeFile, make_documents};
use super::{BoardNote, FabOutputKinds, FabProblem, fab_options};

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
    pub kinds: FabOutputKinds,
    pub documents: DocumentsJob,
    pub folder: PathBuf,
    /// The standard `board`'s designators are shown under (#1416): the
    /// test netlist writes them stored.
    pub designator_standard: Option<signex_types::designator::DesignatorStandard>,
}

/// One written file as the `fabrication_outputs` answer lists it (#2470):
/// its name in the output folder, its kind, its size and the SHA-256 of its
/// bytes in lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts {
    pub name: String,
    pub kind: &'static str,
    pub bytes: usize,
    pub sha256: String,
}

impl FileFacts {
    fn of(file: &MadeFile) -> Self {
        Self {
            name: file.name.clone(),
            kind: file.kind,
            bytes: file.bytes.len(),
            sha256: Sha256::digest(&file.bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        }
    }
}

/// What a Generate wrote, with each file's facts in the same order.
pub type Listed = (FabWritten, Vec<FileFacts>);

/// Run `job`: make every file first, then write them all; when one cannot
/// be made, nothing is written.
pub(crate) fn run_generate(job: GenerateJob) -> Result<FabWritten, FabProblem> {
    run_generate_listed(job).map(|(written, _)| written)
}

/// [`run_generate`], with the facts of every file it wrote.
pub(crate) fn run_generate_listed(job: GenerateJob) -> Result<Listed, FabProblem> {
    let folder = job.folder.clone();
    let made = make_generate(job)?;
    let facts = made.files.iter().map(FileFacts::of).collect();
    write_made(
        made.files,
        &folder,
        &made.name,
        made.notes,
        made.board_notes,
    )
    .map(|written| (written, facts))
}

/// The files one Generate makes, not written yet: an Export Job run (#1299)
/// writes them with the rest of its outputs.
pub(crate) struct Made {
    pub name: String,
    pub files: Vec<MadeFile>,
    pub notes: Vec<FabNote>,
    pub board_notes: Vec<BoardNote>,
}

/// Make every file of `job`; when one cannot be made, none is.
pub(crate) fn make_generate(job: GenerateJob) -> Result<Made, FabProblem> {
    let board = fab_board(job.board, job.whole_panel)?;
    let name = check_job(&board, &job.name)
        .map_err(FabProblem::Refused)?
        .to_string();
    let mut made = Vec::new();
    let notes = if job.kinds.has_fab_files() {
        let options = FabOptions {
            designator_standard: job.designator_standard,
            ..fab_options(&job.kinds, &name)
        };
        fab_files(&board, &options, &mut made)?
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
    Ok(Made {
        name,
        files: made,
        notes,
        board_notes,
    })
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
        kind: kind_id(&file.kind),
        name: file.name,
        bytes: file.contents.into_bytes(),
    }));
    Ok(set.notes)
}

/// The answer's word for a fabrication file's kind.
fn kind_id(kind: &FabFileKind) -> &'static str {
    match kind {
        FabFileKind::Gerber { .. } => "gerber",
        FabFileKind::Drill { .. } => "drill",
        FabFileKind::DrillTable => "drill_table",
        FabFileKind::Centroid => "centroid",
        FabFileKind::TestNetlist => "test_netlist",
        FabFileKind::Ipc2581 => "ipc2581",
        FabFileKind::Odb => "odb",
        FabFileKind::Job => "job",
    }
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
    // Every name is checked before anything is written.
    let paths = made
        .iter()
        .map(|file| file_path(folder, &file.name))
        .collect::<Result<Vec<_>, _>>()?;
    std::fs::create_dir_all(folder)
        .map_err(|e| FabProblem::WriteFailed(format!("{}: {e}", folder.display())))?;
    let mut files = Vec::with_capacity(made.len());
    for (file, path) in made.into_iter().zip(paths) {
        let failed =
            |e: std::io::Error| FabProblem::WriteFailed(format!("{}: {e}", path.display()));
        // The ODB++ folder form names files `<name>-odb/<job>/...`: every
        // folder on the way is made here, not left to the write.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(failed)?;
        }
        signex_types::atomic_io::atomic_write(&path, &file.bytes).map_err(failed)?;
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

/// Where the file named `name` (`/`-separated, as the writers name the
/// ODB++ folder's files) goes under `folder`. A name that is empty, absolute
/// or steps out of the folder is refused, and so is one with a link on its
/// way below the folder (#2983): a committed `<name>-odb` link would carry
/// the files out of it.
fn file_path(folder: &Path, name: &str) -> Result<PathBuf, FabProblem> {
    let mut path = folder.to_path_buf();
    for part in name.split('/') {
        let plain = std::path::Path::new(part)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
        if part.is_empty() || !plain || part.contains('\\') {
            return Err(FabProblem::WriteFailed(format!(
                "{name}: not a file name inside the output folder"
            )));
        }
        path.push(part);
        if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(FabProblem::WriteFailed(format!(
                "{} is a symbolic link; writing through a link is refused",
                path.display()
            )));
        }
    }
    Ok(path)
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

#[cfg(test)]
mod path_tests {
    use super::{MadeFile, file_path, write_made};
    use crate::app::fab_outputs::FabProblem;

    fn made(name: &str) -> MadeFile {
        MadeFile {
            name: name.to_string(),
            kind: "odb",
            bytes: b"x".to_vec(),
        }
    }

    /// The ODB++ folder form: every folder on a file's way is made.
    #[test]
    fn a_nested_name_gets_every_folder_on_its_way() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("out");
        let written = write_made(
            vec![made("b-odb/b/matrix/matrix"), made("b-odb/b/misc/info")],
            &out,
            "b",
            Vec::new(),
            Vec::new(),
        )
        .expect("written");
        assert_eq!(written.files.len(), 2);
        assert!(
            out.join("b-odb")
                .join("b")
                .join("matrix")
                .join("matrix")
                .is_file()
        );
        assert!(
            out.join("b-odb")
                .join("b")
                .join("misc")
                .join("info")
                .is_file()
        );
    }

    /// A name that steps out of the output folder writes nothing at all.
    #[test]
    fn a_name_outside_the_folder_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("out");
        for name in ["b-odb/../../x", "", "b-odb//x", "/x"] {
            assert!(file_path(&out, name).is_err(), "{name:?}");
        }
        let result = write_made(
            vec![made("b-ok.gbr"), made("b-odb/../../x")],
            &out,
            "b",
            Vec::new(),
            Vec::new(),
        );
        assert!(
            matches!(result, Err(FabProblem::WriteFailed(_))),
            "{result:?}"
        );
        assert!(!out.exists(), "nothing was written");
    }
}
