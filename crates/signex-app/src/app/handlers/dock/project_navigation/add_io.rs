//! Blocking file work behind the Add Existing / Add New flows. Each function
//! runs on a `spawn_blocking` worker (never inside `update`) and reports a
//! failure as text, which the completion handler sends to the Messages panel.

use std::path::{Path, PathBuf};

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
    /// Outside — copy to `dest` first.
    Copy { src: PathBuf, dest: PathBuf },
    /// The pick has no file name.
    Unnamed(PathBuf),
}

pub(crate) fn plan_existing(project_dir: &Path, picked: PathBuf) -> ExistingPlan {
    if picked.starts_with(project_dir) {
        return ExistingPlan::InProject(picked);
    }
    match picked.file_name() {
        Some(name) => ExistingPlan::Copy {
            dest: project_dir.join(name),
            src: picked,
        },
        None => ExistingPlan::Unnamed(picked),
    }
}

/// Carry out one plan; `Ok` is the path to register.
pub(crate) fn run_existing(plan: ExistingPlan) -> Result<PathBuf, String> {
    match plan {
        ExistingPlan::InProject(path) => Ok(path),
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
