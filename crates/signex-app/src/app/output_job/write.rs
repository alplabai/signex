//! The blocking half of a run: draw the outputs that are drawn off the UI
//! thread, then write every file crash-safely. Nothing is written when an
//! output cannot be built, and a job never writes outside its project.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::app::handlers::menu::export::job_outputs::{JobFile, JobWork};

/// The files one output wrote, by name in the folder.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WrittenOutput {
    pub name: String,
    pub files: Vec<String>,
}

/// What a run wrote: the JSON answer of `run_output_job` and
/// `export_sheet_png`, and the Messages lines.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct JobReport {
    /// The job file; empty for a PNG export.
    pub job: String,
    pub folder: String,
    pub variant: String,
    pub outputs: Vec<WrittenOutput>,
    /// Enabled outputs of a kind this Signex does not write.
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
    /// The run profile (mockup row 114), when the job has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// What Validate found, when the run validated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks: Option<CheckCounts>,
    /// The zip written beside the folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<String>,
    /// Files Clear the folder deleted, as the run did not write them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<String>,
}

/// The errors and warnings a run's Validate found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct CheckCounts {
    pub errors: usize,
    pub warnings: usize,
}

impl JobReport {
    /// How many files the run wrote.
    pub fn file_count(&self) -> usize {
        self.outputs.iter().map(|output| output.files.len()).sum()
    }
}

/// Build `work` and write its files into `folder`. With `inside`, the
/// folder must stay inside that folder (the job's project), checked before
/// it is made and again after. Blocking IO: run it off the UI thread.
pub(super) fn build_and_write(
    folder: &Path,
    inside: Option<&Path>,
    work: Vec<JobWork>,
) -> Result<(Vec<WrittenOutput>, Vec<String>), String> {
    let mut files: Vec<JobFile> = Vec::new();
    let mut warnings = Vec::new();
    for item in work {
        let (built, said) = item.render()?;
        files.extend(built);
        warnings.extend(said);
    }
    if let Some(project) = inside {
        confined(folder, project)?;
    }
    std::fs::create_dir_all(folder)
        .map_err(|e| format!("could not make the folder {}: {e}", folder.display()))?;
    if let Some(project) = inside {
        confined(folder, project)?;
    }
    let mut written: Vec<WrittenOutput> = Vec::new();
    for file in files {
        let path = folder.join(&file.file);
        if path.is_dir() {
            return Err(format!("{} is a folder, not a file", path.display()));
        }
        signex_types::atomic_io::atomic_write(&path, &file.bytes)
            .map_err(|e| format!("{e}: could not write {}", path.display()))?;
        match written.iter_mut().find(|entry| entry.name == file.output) {
            Some(entry) => entry.files.push(file.file),
            None => written.push(WrittenOutput {
                name: file.output,
                files: vec![file.file],
            }),
        }
    }
    Ok((written, warnings))
}

/// `Err` unless the nearest folder of `folder` that exists resolves, links
/// followed, inside `project`.
pub(super) fn confined(folder: &Path, project: &Path) -> Result<(), String> {
    let outside = || {
        format!(
            "the folder {} is not inside the project folder {}",
            folder.display(),
            project.display()
        )
    };
    let nearest: Option<PathBuf> = folder
        .ancestors()
        .find(|ancestor| ancestor.exists())
        .and_then(|ancestor| std::fs::canonicalize(ancestor).ok());
    let project = std::fs::canonicalize(project).map_err(|_| outside())?;
    match nearest {
        Some(nearest) if nearest.starts_with(&project) => Ok(()),
        _ => Err(outside()),
    }
}
