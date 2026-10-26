//! File ▸ New Project: where the project lives and the off-thread writes.
//!
//! The Save-As dialog yields `<parent>/<stem>.snxprj`; the project itself is
//! the folder `<parent>/<stem>/` holding `<stem>.snxprj` and `<stem>.snxsch`.
//! Writing into `<parent>` directly would turn a shared folder (a Desktop) into
//! the project directory (#1205).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// OS-generated files that do not make a folder "in use".
const OS_CLUTTER: [&str; 3] = [".DS_Store", "Thumbs.db", "desktop.ini"];

/// The `.snxprj` must end in `.snxprj`, have a parent and a UTF-8 stem.
pub(super) fn new_project_parts(project_path: &Path) -> Result<(&Path, String)> {
    if project_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("snxprj"))
        != Some(true)
    {
        anyhow::bail!(
            "new project path must end in .snxprj (got {})",
            project_path.display()
        );
    }
    let dir = project_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("project path has no parent directory"))?;
    let stem = project_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("project path has no file stem"))?
        .to_string();
    Ok((dir, stem))
}

/// Where a New Project chosen at `chosen` lands: its folder and marker file.
/// Reads the disk, so it runs on the blocking worker. Refuses a folder that is
/// not empty or already holds a project; never creates anything.
pub(super) fn resolve_new_project(chosen: &Path) -> Result<(PathBuf, PathBuf)> {
    let (parent, stem) = new_project_parts(chosen)?;
    let marker = format!("{stem}.snxprj");
    let parent_is_the_project_folder = parent.file_name().is_some_and(|n| n == stem.as_str());
    // The user already made `<stem>/` and picked inside it: use it as is
    // rather than nesting `<stem>/<stem>/`.
    if parent_is_the_project_folder && holds_a_project(parent)? {
        anyhow::bail!(
            "a project already exists in {} — open it instead of creating a new one over it",
            parent.display()
        );
    }
    let folder = if parent_is_the_project_folder && is_unused(parent)? {
        parent.to_path_buf()
    } else {
        parent.join(&stem)
    };
    if folder.exists() {
        if !folder.is_dir() {
            anyhow::bail!("{} exists and is not a folder", folder.display());
        }
        if holds_a_project(&folder)? {
            anyhow::bail!(
                "a project already exists in {} — open it instead of creating a new one over it",
                folder.display()
            );
        }
        if !is_unused(&folder)? {
            anyhow::bail!(
                "the folder {} is not empty — pick another name or location",
                folder.display()
            );
        }
    }
    let project_path = folder.join(marker);
    Ok((folder, project_path))
}

fn entries_of(dir: &Path) -> Result<Vec<std::ffi::OsString>> {
    std::fs::read_dir(dir)
        .with_context(|| format!("read folder {}", dir.display()))?
        .map(|e| {
            e.map(|e| e.file_name())
                .with_context(|| format!("read folder {}", dir.display()))
        })
        .collect()
}

/// No entries other than OS clutter.
fn is_unused(dir: &Path) -> Result<bool> {
    Ok(entries_of(dir)?
        .iter()
        .all(|name| OS_CLUTTER.iter().any(|c| name.as_os_str() == *c)))
}

fn holds_a_project(dir: &Path) -> Result<bool> {
    Ok(entries_of(dir)?.iter().any(|name| {
        Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("snxprj"))
    }))
}

/// Create the project folder, the empty `.snxprj` marker (`parse_project` is
/// directory-driven and ignores its content) and the blank companion
/// `<stem>.snxsch`, then the managed `.gitignore` block (#1117). Returns the
/// `.snxprj` actually written. Runs on a blocking worker, never in `update`.
pub(super) fn write_new_project_files(chosen: &Path) -> Result<PathBuf> {
    let (_, stem) = new_project_parts(chosen)?;
    let (dir, project_path) = resolve_new_project(chosen)?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("create project directory {}", dir.display()))?;

    // `create_new` never truncates: a racing writer makes this fail rather
    // than clobber its file.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&project_path)
        .with_context(|| format!("write project file {}", project_path.display()))?;

    let sch_path = dir.join(format!("{stem}.snxsch"));
    let sheet = super::blank_schematic_sheet();
    let serialised = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .context("serialise blank schematic")?;
    // No-clobber publish; an existing schematic is kept as is (#933).
    if let Err(e) =
        super::super::dock::project_navigation::add_io::create_noclobber(&sch_path, |f| {
            std::io::Write::write_all(f, serialised.as_bytes())
        })
        && e.kind() != std::io::ErrorKind::AlreadyExists
    {
        anyhow::bail!("write blank schematic {}: {e}", sch_path.display());
    }
    // The project exists now; a `.gitignore` problem is reported but never
    // fails the creation.
    if let Err(e) = signex_types::project_vcs_files::ensure_project_gitignore(&dir) {
        crate::diagnostics::log_warning(format!("[git] .gitignore not updated: {e}"));
    }
    Ok(project_path)
}
