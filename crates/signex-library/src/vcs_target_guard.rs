//! Which directories Enable Version Control must never `git init` (#1205).
//!
//! `git init` plus the initial commit stages every file under the directory,
//! so initialising a user's home, Desktop or a folder holding several projects
//! would put unrelated personal files into a repository. The check is pure
//! apart from reading the disk (the known folders are injected), and runs on a
//! blocking worker.

use std::path::{Path, PathBuf};

/// Folders under the home directory assumed shared when the platform does not
/// name them.
const WELL_KNOWN_USER_FOLDERS: [&str; 3] = ["Desktop", "Documents", "Downloads"];

/// How many directory levels below the target a foreign `.snxprj` is looked for.
const NESTED_PROJECT_SCAN_DEPTH: usize = 2;

/// Upper bound on entries read per directory. A directory with more is
/// refused rather than guessed at.
const MAX_ENTRIES_PER_DIR: usize = 5_000;

/// How many ancestors of a new project folder are checked for a project.
const MAX_ANCESTORS_CHECKED: usize = 64;

/// The user's home directory and the shared folders that must never become a
/// repository.
#[derive(Debug, Clone, Default)]
pub struct KnownFolders {
    /// The home directory, `None` when the platform cannot tell.
    pub home: Option<PathBuf>,
    /// Desktop, Documents, Downloads (as the platform names them, so a
    /// OneDrive-redirected Desktop or a localised XDG name is found).
    pub shared: Vec<PathBuf>,
}

impl KnownFolders {
    /// Ask the platform. The home directory comes from `dirs::home_dir`
    /// alone (the Windows known-folder API there, `$HOME` on Unix): there is
    /// no environment fallback, so a Unix-style `HOME` inherited from an MSYS
    /// shell can never shadow the real profile. A folder the platform does not name falls back to
    /// `<home>/Desktop` etc.
    pub fn from_system() -> Self {
        let home = dirs::home_dir();
        let mut shared: Vec<PathBuf> = [
            dirs::desktop_dir(),
            dirs::document_dir(),
            dirs::download_dir(),
        ]
        .into_iter()
        .flatten()
        .collect();
        if let Some(home) = &home {
            shared.extend(WELL_KNOWN_USER_FOLDERS.iter().map(|n| home.join(n)));
        }
        Self { home, shared }
    }
}

/// [`version_control_refusal`] against the platform's known folders. The
/// root, nested-project and sibling checks run even when the home directory is
/// unknown; that gap is logged.
pub fn system_version_control_refusal(dir: &Path, project_stem: Option<&str>) -> Option<String> {
    let known = KnownFolders::from_system();
    if known.home.is_none() {
        tracing::warn!(
            "[git] the home folder could not be determined: the home/Desktop/Documents/\
             Downloads check was skipped for {}",
            dir.display()
        );
    }
    version_control_refusal(dir, &known, project_stem)
}

/// The refusal message when `dir` must not be turned into a repository, or
/// `None` when it is fine. `project_stem` is the stem of the project that
/// lives in `dir`, when known: the only `.snxprj` allowed directly in `dir`.
pub fn version_control_refusal(
    dir: &Path,
    known: &KnownFolders,
    project_stem: Option<&str>,
) -> Option<String> {
    let target = canonical_or_given(dir);
    if is_filesystem_root(&target) {
        return Some(format!(
            "{} is a filesystem root; version control is not enabled there. \
             Move the project into its own folder first.",
            dir.display()
        ));
    }
    if let Some(home) = known.home.as_deref().map(canonical_or_given)
        && target == home
    {
        return Some(format!(
            "{} is your home folder; version control would stage your personal files. \
             Move the project into its own folder first.",
            dir.display()
        ));
    }
    if known
        .shared
        .iter()
        .any(|folder| canonical_or_given(folder) == target)
    {
        return Some(format!(
            "{} is a shared user folder; version control would stage unrelated files. \
             Move the project into its own folder first.",
            dir.display()
        ));
    }
    scan_for_other_projects(dir, &target, project_stem).err()
}

fn canonical_or_given(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// A true filesystem root: no parent (`/`, a drive root or a UNC share root).
fn is_filesystem_root(target: &Path) -> bool {
    target.parent().is_none()
}

fn is_project_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("snxprj"))
}

/// `Err(refusal)` when `target` itself holds a project that is not this one,
/// or a subfolder (to [`NESTED_PROJECT_SCAN_DEPTH`]) holds any project, or the
/// scan could not be completed.
fn scan_for_other_projects(
    shown: &Path,
    target: &Path,
    project_stem: Option<&str>,
) -> Result<(), String> {
    let cannot = |why: String| {
        format!(
            "could not check {} for other projects: {why}",
            shown.display()
        )
    };
    let entries = read_entries(target).map_err(&cannot)?;
    let mut own_projects = Vec::new();
    for entry in &entries {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            return Err(cannot(format!("cannot stat {}", path.display())));
        };
        if kind.is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            if let Some(found) =
                nested_project(&path, NESTED_PROJECT_SCAN_DEPTH).map_err(&cannot)?
            {
                return Err(format!(
                    "{} contains another project ({}); version control would mix them. \
                     Enable it from a folder that holds only this project.",
                    shown.display(),
                    found.display()
                ));
            }
        } else if is_project_file(&path) {
            own_projects.push(path);
        }
    }
    let foreign = |p: &PathBuf| match project_stem {
        Some(stem) => p.file_stem().is_none_or(|s| s != stem),
        None => false,
    };
    if own_projects.len() > 1 || own_projects.iter().any(foreign) {
        return Err(format!(
            "{} holds more than this project ({}); version control would stage the others too. \
             Enable it from a folder that holds only this project.",
            shown.display(),
            own_projects
                .iter()
                .filter_map(|p| p.file_name())
                .map(|n| n.to_string_lossy())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(())
}

/// A `.snxprj` among the files of `dir` or of its subfolders, `levels` deep.
fn nested_project(dir: &Path, levels: usize) -> Result<Option<PathBuf>, String> {
    let mut subdirs = Vec::new();
    for entry in read_entries(dir)? {
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|e| format!("cannot stat {}: {e}", path.display()))?;
        if kind.is_dir() {
            subdirs.push(path);
        } else if is_project_file(&path) {
            return Ok(Some(path));
        }
    }
    if levels > 1 {
        for sub in subdirs {
            if let Some(found) = nested_project(&sub, levels - 1)? {
                return Ok(Some(found));
            }
        }
    }
    Ok(None)
}

/// The entries of `dir`, or why they could not all be read.
fn read_entries(dir: &Path) -> Result<Vec<std::fs::DirEntry>, String> {
    let mut entries = Vec::new();
    for entry in
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?
    {
        if entries.len() >= MAX_ENTRIES_PER_DIR {
            return Err(format!(
                "{} has more than {MAX_ENTRIES_PER_DIR} entries",
                dir.display()
            ));
        }
        entries.push(entry.map_err(|e| format!("cannot read {}: {e}", dir.display()))?);
    }
    Ok(entries)
}

/// The nearest ancestor of `folder` (itself excluded) that is a project
/// folder, for New Project: a project inside another would mix their files.
/// An ancestor is a project folder when it holds `<its own name>.snxprj`
/// (ASCII case-insensitive). A loose `.snxprj` in a shared folder (the old
/// layout) does not count, and the home folder, the shared folders and
/// filesystem roots are skipped. An ancestor that cannot be listed is skipped.
pub fn enclosing_project(folder: &Path, known: &KnownFolders) -> Option<PathBuf> {
    let protected: Vec<PathBuf> = known
        .home
        .iter()
        .chain(known.shared.iter())
        .map(|p| canonical_or_given(p))
        .collect();
    folder
        .ancestors()
        .skip(1)
        .take(MAX_ANCESTORS_CHECKED)
        .filter(|a| a.parent().is_some())
        .filter(|a| !protected.contains(&canonical_or_given(a)))
        .find(|ancestor| holds_own_named_project(ancestor))
        .map(Path::to_path_buf)
}

fn holds_own_named_project(folder: &Path) -> bool {
    let Some(name) = folder.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Ok(entries) = std::fs::read_dir(folder) else {
        return false;
    };
    entries.flatten().map(|e| e.path()).any(|p| {
        is_project_file(&p)
            && p.file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|stem| stem.eq_ignore_ascii_case(name))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }

    fn known(home: &Path) -> KnownFolders {
        KnownFolders {
            home: Some(home.to_path_buf()),
            shared: WELL_KNOWN_USER_FOLDERS
                .iter()
                .map(|n| home.join(n))
                .collect(),
        }
    }

    #[test]
    fn refuses_a_filesystem_root() {
        let root = Path::new(if cfg!(windows) { "C:\\" } else { "/" });
        assert!(version_control_refusal(root, &KnownFolders::default(), None).is_some());
    }

    #[test]
    fn refuses_the_home_folder_and_the_shared_folders() {
        let home = tempfile::tempdir().unwrap();
        let known = known(home.path());
        for name in WELL_KNOWN_USER_FOLDERS {
            std::fs::create_dir_all(home.path().join(name)).unwrap();
            assert!(version_control_refusal(&home.path().join(name), &known, None).is_some());
        }
        assert!(version_control_refusal(home.path(), &known, None).is_some());
    }

    #[test]
    fn refuses_a_platform_named_shared_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let redirected = tmp.path().join("OneDrive").join("Desktop");
        std::fs::create_dir_all(&redirected).unwrap();
        let known = KnownFolders {
            home: Some(tmp.path().to_path_buf()),
            shared: vec![redirected.clone()],
        };
        assert!(version_control_refusal(&redirected, &known, None).is_some());
    }

    #[test]
    fn an_unknown_home_still_runs_the_other_checks() {
        let parent = tempfile::tempdir().unwrap();
        touch(&parent.path().join("A").join("A.snxprj"));
        let none = KnownFolders::default();
        assert!(version_control_refusal(parent.path(), &none, None).is_some());
    }

    #[test]
    fn allows_a_project_folder_under_the_home_folder() {
        let home = tempfile::tempdir().unwrap();
        let project = home.path().join("Desktop").join("Board");
        touch(&project.join("Board.snxprj"));
        let known = known(home.path());
        assert_eq!(
            version_control_refusal(&project, &known, Some("Board")),
            None
        );
    }

    #[test]
    fn refuses_a_folder_that_holds_other_projects() {
        let parent = tempfile::tempdir().unwrap();
        touch(&parent.path().join("A").join("A.snxprj"));
        touch(&parent.path().join("B").join("deep").join("B.snxprj"));
        assert!(version_control_refusal(parent.path(), &KnownFolders::default(), None).is_some());
    }

    #[test]
    fn refuses_sibling_projects_in_the_old_shared_layout() {
        let work = tempfile::tempdir().unwrap();
        touch(&work.path().join("A.snxprj"));
        touch(&work.path().join("B.snxprj"));
        touch(&work.path().join("diary.txt"));
        let none = KnownFolders::default();
        assert!(version_control_refusal(work.path(), &none, None).is_some());
        assert!(version_control_refusal(work.path(), &none, Some("A")).is_some());
    }

    #[test]
    fn refuses_a_single_project_that_is_not_this_one() {
        let work = tempfile::tempdir().unwrap();
        touch(&work.path().join("B.snxprj"));
        let none = KnownFolders::default();
        assert!(version_control_refusal(work.path(), &none, Some("A")).is_some());
        assert_eq!(version_control_refusal(work.path(), &none, Some("B")), None);
    }

    #[test]
    fn a_project_with_its_own_marker_and_plain_subfolders_is_fine() {
        let project = tempfile::tempdir().unwrap();
        touch(&project.path().join("P.snxprj"));
        touch(&project.path().join("assets").join("a.png"));
        let none = KnownFolders::default();
        assert_eq!(
            version_control_refusal(project.path(), &none, Some("P")),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_subfolder_refuses_instead_of_allowing() {
        use std::os::unix::fs::PermissionsExt;
        let project = tempfile::tempdir().unwrap();
        touch(&project.path().join("P.snxprj"));
        let locked = project.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let refusal = version_control_refusal(project.path(), &KnownFolders::default(), None);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        if std::fs::read_dir(&locked).is_ok() {
            return; // running as root: permissions are not enforced
        }
        assert!(refusal.is_some_and(|m| m.contains("could not check")));
    }

    #[test]
    fn finds_the_project_folder_that_encloses_a_folder() {
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("Alpha").join("alpha.snxprj"));
        let inner = tmp.path().join("Alpha").join("Beta");
        let none = KnownFolders::default();
        assert_eq!(
            enclosing_project(&inner, &none),
            Some(tmp.path().join("Alpha"))
        );
        assert_eq!(enclosing_project(&tmp.path().join("Other"), &none), None);
    }

    #[test]
    fn a_loose_project_in_a_shared_folder_does_not_enclose() {
        let home = tempfile::tempdir().unwrap();
        touch(&home.path().join("Desktop").join("Untitled.snxprj"));
        touch(&home.path().join("Stray.snxprj"));
        let known = known(home.path());
        let deep = home.path().join("Desktop").join("work").join("X");
        assert_eq!(enclosing_project(&deep, &known), None);
        assert_eq!(
            enclosing_project(&home.path().join("Desktop").join("Foo"), &known),
            None
        );
    }

    #[test]
    fn a_loose_project_in_an_ordinary_folder_does_not_enclose() {
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("Work").join("Untitled.snxprj"));
        let none = KnownFolders::default();
        assert_eq!(
            enclosing_project(&tmp.path().join("Work").join("X"), &none),
            None
        );
    }
}
