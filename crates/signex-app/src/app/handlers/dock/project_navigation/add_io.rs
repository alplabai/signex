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
            if dest.exists() {
                return Err(format!("destination already exists: {}", dest.display()));
            }
            std::fs::copy(&src, &dest)
                .map_err(|e| format!("copy of {} failed: {e}", src.display()))?;
            Ok(dest)
        }
    }
}

/// Write a blank schematic at `path`, refusing to overwrite.
pub(crate) fn write_blank_schematic(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Err(format!("destination already exists: {}", path.display()));
    }
    let sheet = crate::app::handlers::document_files::blank_schematic_sheet_for_new_doc();
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .map_err(|e| format!("serialise blank sheet: {e}"))?;
    signex_types::atomic_io::atomic_write(path, text.as_bytes())
        .map_err(|e| format!("write blank sheet to {}: {e}", path.display()))
}
