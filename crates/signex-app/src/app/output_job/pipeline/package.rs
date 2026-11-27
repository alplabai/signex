//! Package, the last step of a run (mockup row 114, "Output container"):
//! every file written at once, crash-safely, into the job's folder (a
//! release folder named with the revision and the date, one subfolder per
//! group when asked), the files the run did not write cleared when asked,
//! and a zip of the run beside the folder. Blocking: run off the UI thread.
//! The folder stays inside the project folder, links followed.

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use super::Layout;
use crate::app::handlers::menu::export::job_outputs::JobFile;
use crate::app::output_job::write::{WrittenOutput, confined};

/// The most release folders of one name a day takes (`-2`, `-3`, ...).
const MAX_SAME_DAY: usize = 99;

/// What Package wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct Packaged {
    /// The folder written, the release folder when there is one.
    pub folder: PathBuf,
    pub outputs: Vec<WrittenOutput>,
    /// The zip beside the folder, by name.
    pub zip: Option<String>,
    /// Files Clear the folder deleted, relative to the folder.
    pub removed: Vec<String>,
    /// The files the Gerber Viewer reads, relative to the folder.
    pub viewable: Vec<String>,
}

/// A name the run writes under its folder, `/`-separated, with every part a
/// plain name.
fn plain_relative(name: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for part in name.split('/') {
        let plain = !part.is_empty()
            && part != "."
            && part != ".."
            && !part.contains(['\\', ':'])
            && Path::new(part)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)));
        if !plain {
            return None;
        }
        path.push(part);
    }
    Some(path)
}

/// The release folder under `base`: `name`, else the first of `name-2`,
/// `name-3`, ... that does not exist, so a second run on the same day never
/// writes into the first one's folder.
fn release_folder(base: &Path, name: &str) -> Result<PathBuf, String> {
    let first = base.join(name);
    if !first.exists() {
        return Ok(first);
    }
    (2..=MAX_SAME_DAY)
        .map(|n| base.join(format!("{name}-{n}")))
        .find(|candidate| !candidate.exists())
        .ok_or_else(|| format!("{MAX_SAME_DAY} release folders named {name} exist already"))
}

/// Write `files` as `layout` lays them out.
pub(super) fn write_package(layout: &Layout, files: Vec<JobFile>) -> Result<Packaged, String> {
    confined(&layout.folder, &layout.project_dir)?;
    let folder = match &layout.release {
        Some(name) => release_folder(&layout.folder, name)?,
        None => layout.folder.clone(),
    };
    // Every name is checked before anything is written.
    let mut placed: Vec<(PathBuf, String, JobFile)> = Vec::with_capacity(files.len());
    for file in files {
        let inner = plain_relative(&file.file)
            .ok_or_else(|| format!("{}: not a file name inside the folder", file.file))?;
        let group = layout
            .groups
            .as_ref()
            .and_then(|groups| groups.get(&file.output))
            .map(|group| group.folder());
        let relative = match group {
            Some(group) => Path::new(group).join(inner),
            None => inner,
        };
        let shown = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        placed.push((relative, shown, file));
    }
    std::fs::create_dir_all(&folder)
        .map_err(|e| format!("could not make the folder {}: {e}", folder.display()))?;
    confined(&folder, &layout.project_dir)?;
    let mut outputs: Vec<WrittenOutput> = Vec::new();
    let mut written: BTreeSet<String> = BTreeSet::new();
    let mut viewable = Vec::new();
    for (relative, shown, file) in &placed {
        let path = folder.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not make the folder {}: {e}", parent.display()))?;
        }
        if path.is_dir() {
            return Err(format!("{} is a folder, not a file", path.display()));
        }
        signex_types::atomic_io::atomic_write(&path, &file.bytes)
            .map_err(|e| format!("{e}: could not write {}", path.display()))?;
        written.insert(shown.clone());
        let name = shown.rsplit('/').next().unwrap_or(shown);
        if crate::app::gerber_viewer::opens_by_name(name) && !name.ends_with(".zip") {
            viewable.push(shown.clone());
        }
        match outputs.iter_mut().find(|entry| entry.name == file.output) {
            Some(entry) => entry.files.push(shown.clone()),
            None => outputs.push(WrittenOutput {
                name: file.output.clone(),
                files: vec![shown.clone()],
            }),
        }
    }
    // The job's own folder or the project folder hold the design: Clear
    // never deletes there.
    let holds_design = folder == layout.job_dir || folder == layout.project_dir;
    let removed = match layout.clear && !holds_design {
        true => clear_folder(&folder, &written)?,
        false => Vec::new(),
    };
    let zip = match layout.zip {
        true => Some(write_zip(&folder, &placed)?),
        false => None,
    };
    Ok(Packaged {
        folder,
        outputs,
        zip,
        removed,
        viewable,
    })
}

/// Delete the files in `folder` and its subfolders, one level deep, that
/// are not in `written`. Folders and links are left as they are.
fn clear_folder(folder: &Path, written: &BTreeSet<String>) -> Result<Vec<String>, String> {
    let mut removed = Vec::new();
    let listing = |dir: &Path| {
        std::fs::read_dir(dir).map_err(|e| format!("could not list {}: {e}", dir.display()))
    };
    let mut dirs: Vec<(PathBuf, String)> = vec![(folder.to_path_buf(), String::new())];
    let mut depth = 0;
    while let Some((dir, prefix)) = dirs.pop() {
        for entry in listing(&dir)? {
            let entry = entry.map_err(|e| format!("could not list {}: {e}", dir.display()))?;
            let kind = entry
                .file_type()
                .map_err(|e| format!("could not read {}: {e}", entry.path().display()))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let shown = format!("{prefix}{name}");
            if kind.is_dir() && depth == 0 {
                dirs.push((entry.path(), format!("{shown}/")));
            } else if kind.is_file() && !written.contains(&shown) && !is_design_file(&name) {
                std::fs::remove_file(entry.path())
                    .map_err(|e| format!("could not delete {}: {e}", entry.path().display()))?;
                removed.push(shown);
            }
        }
        depth = 1;
    }
    removed.sort();
    Ok(removed)
}

/// A Signex document (`.snx*`), which Clear never deletes.
fn is_design_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ext.get(..3)
                .is_some_and(|head| head.eq_ignore_ascii_case("snx"))
        })
}

/// Write the run's files into `<folder>.zip` beside `folder`, its entries
/// named as in the folder, with a fixed date so two runs zip the same bytes.
fn write_zip(folder: &Path, placed: &[(PathBuf, String, JobFile)]) -> Result<String, String> {
    let name = folder
        .file_name()
        .map(|n| format!("{}.zip", n.to_string_lossy()))
        .ok_or_else(|| format!("{} has no name to give a zip", folder.display()))?;
    let path = folder.with_file_name(&name);
    let failed = |e: &dyn std::fmt::Display| format!("could not write {}: {e}", path.display());
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
    for (_, shown, file) in placed {
        writer
            .start_file(shown.as_str(), options)
            .map_err(|e| failed(&e))?;
        writer.write_all(&file.bytes).map_err(|e| failed(&e))?;
    }
    let bytes = writer.finish().map_err(|e| failed(&e))?.into_inner();
    signex_types::atomic_io::atomic_write(&path, &bytes).map_err(|e| failed(&e))?;
    Ok(name)
}
