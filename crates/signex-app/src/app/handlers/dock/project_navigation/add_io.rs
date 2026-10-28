//! Blocking file work behind the Add Existing / Add New flows. Each function
//! runs on a `spawn_blocking` worker (never inside `update`) and reports a
//! failure as text, which the completion handler sends to the Messages panel.

use std::path::{Path, PathBuf};

use crate::app::state::library_entry::is_inside_project;
use crate::app::state::scope::CaseFold;

/// Run `work` on the blocking pool; a worker that died becomes the error.
pub(crate) async fn on_blocking_pool<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .unwrap_or_else(|e| Err(format!("worker thread failed: {e}")))
}

/// One picked file of an Add Existing batch, planned in `update` (pure path
/// maths) and carried out on the worker.
pub(crate) enum ExistingPlan {
    /// Already inside the project directory — register as is.
    InProject(PathBuf),
    /// A `.snxlib` outside the project: registered where it is, copying
    /// nothing. A library is a directory (`symbols/`, `footprints/`, `sims/`
    /// and any version-control setup sit beside the `.snxlib`), so copying
    /// the one file would leave its rows dangling (#1214).
    InPlace(PathBuf),
    /// Outside — copy to `dest` first.
    Copy { src: PathBuf, dest: PathBuf },
    /// The pick has no file name.
    Unnamed(PathBuf),
}

/// Inside-the-project is the same fold-aware call the registration makes, so
/// copy-or-in-place and the stored kind cannot disagree.
pub(crate) fn plan_existing(project_dir: &Path, picked: PathBuf, fold: CaseFold) -> ExistingPlan {
    if is_inside_project(project_dir, &picked, fold) {
        return ExistingPlan::InProject(picked);
    }
    if is_snxlib(&picked) {
        return ExistingPlan::InPlace(picked);
    }
    match picked.file_name() {
        Some(name) => ExistingPlan::Copy {
            dest: project_dir.join(name),
            src: picked,
        },
        None => ExistingPlan::Unnamed(picked),
    }
}

pub(crate) fn is_snxlib(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("snxlib"))
}

/// Carry out one plan; `Ok` is the path to register.
pub(crate) fn run_existing(plan: ExistingPlan) -> Result<PathBuf, String> {
    match plan {
        ExistingPlan::InProject(path) | ExistingPlan::InPlace(path) => Ok(path),
        ExistingPlan::Unnamed(path) => Err(format!("{} has no file name", path.display())),
        ExistingPlan::Copy { src, dest } => {
            let mut from = std::fs::File::open(&src)
                .map_err(|e| format!("copy of {} failed: {e}", src.display()))?;
            create_noclobber(&dest, |tmp| std::io::copy(&mut from, tmp).map(|_| ()))
                .map_err(|e| format!("copy of {} failed: {e}", src.display()))?;
            Ok(dest)
        }
    }
}

impl crate::app::Signex {
    /// The worker body of an Add Existing copy, exposed so a regression test
    /// can run what the spawned task runs without an async runtime.
    #[doc(hidden)]
    pub fn copy_into_project_blocking(src: PathBuf, dest: PathBuf) -> Result<PathBuf, String> {
        run_existing(ExistingPlan::Copy { src, dest })
    }

    /// The worker body of Add New Schematic, exposed for the same reason.
    #[doc(hidden)]
    pub fn write_blank_schematic_blocking(path: &Path) -> Result<(), String> {
        write_blank_schematic(path)
    }
}

/// Fill a temp file next to `dest` via `fill`, fsync it, then publish it with
/// a no-clobber rename. The existence check and the publish are one atomic
/// step (`persist_noclobber` fails if `dest` exists), so two overlapping
/// writers can never truncate each other's file, and a failed `fill` drops
/// the temp, leaving no partial `dest`.
pub(crate) fn create_noclobber(
    dest: &Path,
    fill: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let dir = dest.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    fill(tmp.as_file_mut())?;
    tmp.as_file().sync_all()?;
    tmp.persist_noclobber(dest).map_err(|e| {
        if e.error.kind() == std::io::ErrorKind::AlreadyExists {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("destination already exists: {}", dest.display()),
            )
        } else {
            e.error
        }
    })?;
    Ok(())
}

/// Write a blank schematic at `path`, refusing to overwrite.
pub(crate) fn write_blank_schematic(path: &Path) -> Result<(), String> {
    let sheet = crate::app::handlers::document_files::blank_schematic_sheet_for_new_doc();
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .map_err(|e| format!("serialise blank sheet: {e}"))?;
    create_noclobber(path, |f| std::io::Write::write_all(f, text.as_bytes()))
        .map_err(|e| format!("write blank sheet to {}: {e}", path.display()))
}

pub(crate) use crate::app::contracts::ChildSheetWriteError;

/// Write `sheet` as a new schematic at `path`, refusing to overwrite: the
/// child sheet created from a sheet symbol. Same temp + fsync + no-clobber
/// rename as every new document.
pub(crate) fn write_new_schematic(
    path: &Path,
    sheet: signex_types::schematic::SchematicSheet,
) -> Result<(), ChildSheetWriteError> {
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .map_err(|e| ChildSheetWriteError::Failed(format!("serialise sheet: {e}")))?;
    create_noclobber(path, |f| std::io::Write::write_all(f, text.as_bytes())).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            ChildSheetWriteError::AlreadyExists
        } else {
            ChildSheetWriteError::Failed(format!("write sheet to {}: {e}", path.display()))
        }
    })
}

/// [`write_new_schematic`] on the blocking pool; a worker that died is a failure.
pub(crate) async fn write_new_schematic_off_thread(
    path: PathBuf,
    sheet: signex_types::schematic::SchematicSheet,
) -> Result<(), ChildSheetWriteError> {
    tokio::task::spawn_blocking(move || write_new_schematic(&path, sheet))
        .await
        .unwrap_or_else(|e| {
            Err(ChildSheetWriteError::Failed(format!(
                "worker thread failed: {e}"
            )))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_copy_to_the_same_dest_fails_and_keeps_the_first() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a.txt"), d.path().join("b.txt"));
        std::fs::write(&a, "first").unwrap();
        std::fs::write(&b, "second").unwrap();
        let proj = d.path().join("proj");
        std::fs::create_dir(&proj).unwrap();
        let dest = proj.join("x.txt");
        let plan = |src| ExistingPlan::Copy {
            src,
            dest: dest.clone(),
        };
        run_existing(plan(a)).unwrap();
        let err = run_existing(plan(b)).unwrap_err();
        assert!(err.contains("already exists"), "{err}");
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "first");
        assert_eq!(std::fs::read_dir(&proj).unwrap().count(), 1);
    }

    #[test]
    fn an_outside_snxlib_is_planned_in_place_and_copies_nothing() {
        let d = tempfile::tempdir().unwrap();
        let proj = d.path().join("proj");
        std::fs::create_dir(&proj).unwrap();
        let lib = d.path().join("Lib.snxlib");
        std::fs::write(&lib, "x").unwrap();
        let plan = plan_existing(&proj, lib.clone(), CaseFold::default());
        assert!(matches!(plan, ExistingPlan::InPlace(_)));
        assert_eq!(run_existing(plan).unwrap(), lib);
        assert_eq!(std::fs::read_dir(&proj).unwrap().count(), 0);
    }

    #[test]
    fn a_failed_copy_leaves_no_dest_file() {
        let d = tempfile::tempdir().unwrap();
        let proj = d.path().join("proj");
        std::fs::create_dir(&proj).unwrap();
        let dest = proj.join("x.txt");
        let plan = ExistingPlan::Copy {
            src: d.path().join("missing.txt"),
            dest: dest.clone(),
        };
        assert!(run_existing(plan).is_err());
        assert!(!dest.exists());
        assert_eq!(std::fs::read_dir(&proj).unwrap().count(), 0);
    }

    #[test]
    fn a_second_blank_schematic_is_refused_and_the_first_is_intact() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.snxsch");
        write_blank_schematic(&p).unwrap();
        let first = std::fs::read_to_string(&p).unwrap();
        let err = write_blank_schematic(&p).unwrap_err();
        assert!(err.contains("already exists"), "{err}");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), first);
    }
}
