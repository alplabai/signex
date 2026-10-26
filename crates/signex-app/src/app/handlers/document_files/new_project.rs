//! File ▸ New Project: where the project lives and the off-thread writes.
//!
//! The Save-As dialog yields `<parent>/<stem>.snxprj`; the project itself is
//! the folder `<parent>/<stem>/` holding `<stem>.snxprj` and `<stem>.snxsch`.
//! Writing into `<parent>` directly would turn a shared folder (a Desktop) into
//! the project directory (#1205).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::ignore::IgnoreResult;

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

/// Where a New Project lands: its folder, its `.snxprj`, and whether the
/// folder is already there (an empty one the user made).
pub(super) struct NewProjectTarget {
    folder: PathBuf,
    project_path: PathBuf,
    folder_exists: bool,
}

/// Where a New Project chosen at `chosen` lands. Reads the disk, so it runs on
/// the blocking worker. Refuses a folder that is not empty, already holds a
/// project, or sits inside another project; never creates anything.
pub(super) fn resolve_new_project(chosen: &Path) -> Result<NewProjectTarget> {
    let (parent, stem) = new_project_parts(chosen)?;
    let marker = format!("{stem}.snxprj");
    // ASCII case-insensitive on every volume: on a case-sensitive one this
    // only reuses a `foo` folder for project `Foo`, which is harmless.
    let parent_is_the_project_folder = parent
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case(&stem));
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
    if let Some(outer) = signex_library::vcs_target_guard::enclosing_project(&folder) {
        anyhow::bail!(
            "{} would be inside the project in {} — pick a location outside it",
            folder.display(),
            outer.display()
        );
    }
    let folder_exists = folder.exists();
    if folder_exists {
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
    Ok(NewProjectTarget {
        folder,
        project_path,
        folder_exists,
    })
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

/// What one New Project call put on disk, so a failure can take it back.
#[derive(Default)]
struct Created {
    folder: bool,
    files: Vec<PathBuf>,
}

impl Created {
    /// Remove what this call created: the whole folder when it made it, else
    /// only its own files. Best effort; a leftover would make a retry refuse.
    fn roll_back(&self, dir: &Path) {
        if self.folder {
            std::fs::remove_dir_all(dir).ignore("rollback of a failed New Project");
        } else {
            for file in &self.files {
                std::fs::remove_file(file).ignore("rollback of a failed New Project");
            }
        }
    }
}

/// Create the project folder, the empty `.snxprj` marker (`parse_project` is
/// directory-driven and ignores its content) and the blank companion
/// `<stem>.snxsch`, then the managed `.gitignore` block (#1117). Returns the
/// `.snxprj` actually written. A failure removes what this call created.
/// Runs on a blocking worker, never in `update`.
pub(super) fn write_new_project_files(chosen: &Path) -> Result<PathBuf> {
    let (_, stem) = new_project_parts(chosen)?;
    let target = resolve_new_project(chosen)?;
    let mut created = Created::default();
    let result = write_into(&target, &stem, &mut created);
    if result.is_err() {
        created.roll_back(&target.folder);
    }
    result.map(|()| target.project_path)
}

fn write_into(target: &NewProjectTarget, stem: &str, created: &mut Created) -> Result<()> {
    let dir = &target.folder;
    if !target.folder_exists {
        // `create_dir`, not `_all`: fail if the folder appeared since the
        // check instead of writing into someone else's folder.
        std::fs::create_dir(dir)
            .with_context(|| format!("create project directory {}", dir.display()))?;
        created.folder = true;
    }
    // `create_new` never truncates: a racing writer makes this fail rather
    // than clobber its file.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target.project_path)
        .with_context(|| format!("write project file {}", target.project_path.display()))?;
    created.files.push(target.project_path.clone());

    let sch_path = dir.join(format!("{stem}.snxsch"));
    let sheet = super::blank_schematic_sheet();
    let serialised = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .context("serialise blank schematic")?;
    // No-clobber publish; a schematic that appeared meanwhile is not ours.
    super::super::dock::project_navigation::add_io::create_noclobber(&sch_path, |f| {
        std::io::Write::write_all(f, serialised.as_bytes())
    })
    .map_err(|e| anyhow::anyhow!("write blank schematic {}: {e}", sch_path.display()))?;
    created.files.push(sch_path);
    // The project exists now; a `.gitignore` problem is reported but never
    // fails the creation.
    if let Err(e) = signex_types::project_vcs_files::ensure_project_gitignore(dir) {
        crate::diagnostics::log_warning(format!("[git] .gitignore not updated: {e}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rollback_removes_the_folder_this_call_created() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("P");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("P.snxprj"), b"").unwrap();
        let created = Created {
            folder: true,
            files: Vec::new(),
        };
        created.roll_back(&dir);
        assert!(!dir.exists());
    }

    #[test]
    fn a_rollback_keeps_a_folder_the_user_made_and_removes_only_our_files() {
        let tmp = tempfile::tempdir().unwrap();
        let ours = tmp.path().join("P.snxprj");
        std::fs::write(&ours, b"").unwrap();
        std::fs::write(tmp.path().join(".DS_Store"), b"x").unwrap();
        let created = Created {
            folder: false,
            files: vec![ours.clone()],
        };
        created.roll_back(tmp.path());
        assert!(!ours.exists());
        assert!(tmp.path().join(".DS_Store").exists());
    }
}
