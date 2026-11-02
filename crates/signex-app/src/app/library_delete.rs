//! Deleting a library from disk without touching anything that is not one.
//!
//! A library is `<dir>/<name>.snxlib` plus known sub-folders of primitives
//! (`signex-library`, `adapters/local_git`). Delete removes the manifest and
//! the files of known library kinds inside the known folders, then each
//! emptied folder with a non-recursive `remove_dir`. Everything else (user
//! files, `.git`, `.gitattributes`) stays and is reported back.

use std::fs;
use std::path::{Path, PathBuf};

/// Extensions of the files that may be removed from a library sub-folder.
fn folder_extensions(folder: &str) -> &'static [&'static str] {
    match folder {
        "symbols" => &["snxsym"],
        "footprints" => &["snxfpt"],
        "sims" => &["snxsim"],
        "3dmodels" => &["step", "stp", "wrl", "iges"],
        "classes" => &["snxcls", "toml"],
        _ => &[],
    }
}

/// Known sub-folders (relative to the library folder) and the extensions of
/// the files that may be removed from them: the library crate's own folder
/// list plus the datasheet assets. A project's own `models` is not here.
pub(crate) fn known_folders() -> Vec<(&'static str, &'static [&'static str])> {
    let mut folders: Vec<_> = signex_library::adapters::local_git::LIBRARY_SUBFOLDERS
        .iter()
        .map(|f| (*f, folder_extensions(f)))
        .collect();
    folders.push(("assets/datasheets", &["pdf"]));
    folders
}

/// Parent folders emptied last, innermost first.
const KNOWN_PARENTS: &[&str] = &["assets"];

/// Delete the library whose manifest is `manifest`. `library_dir` is the
/// folder cleaned up after it: `None` skips the clean-up (the folder is the
/// project's, or another library shares it) and lists the known folders that
/// were left untouched. Returns the paths that were left on disk.
pub(crate) fn delete_library(
    manifest: &Path,
    library_dir: Option<&Path>,
) -> Result<Vec<PathBuf>, String> {
    // Named after this library, so safe to remove even beside a sibling.
    let sidecars = lock_sidecars(manifest);
    fs::remove_file(manifest).map_err(|e| e.to_string())?;
    for sidecar in sidecars {
        // Usually absent (a git library keeps its lock elsewhere).
        fs::remove_file(sidecar).ok();
    }
    let Some(dir) = library_dir else {
        return Ok(untouched_known_folders(manifest.parent()));
    };
    Ok(remove_known_artifacts(dir))
}

/// The no-VCS write lock and its queue beside `manifest`. A lock kept under
/// `.git` is left to the repository.
fn lock_sidecars(manifest: &Path) -> Vec<PathBuf> {
    let lock = signex_library::write_lock_path(manifest);
    if lock.parent() != manifest.parent() {
        return Vec::new();
    }
    let mut queue = lock.clone().into_os_string();
    queue.push(".queue");
    vec![lock, PathBuf::from(queue)]
}

/// Does `dir` hold a project or design document (`.snxprj`, `.snxsch`,
/// `.snxpcb`)? Its sub-folders are the project's, never a library's to clean.
pub(crate) fn holds_project_files(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            let path = e.path();
            ["snxprj", "snxsch", "snxpcb"]
                .iter()
                .any(|ext| crate::app::file_extension::has_extension(&path, ext))
        })
    })
}

/// Does `dir` hold a library manifest other than `manifest`? Such a folder's
/// sub-folders are shared, so none of them is this library's to clean up.
pub(crate) fn holds_other_library(dir: &Path, manifest: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            let path = e.path();
            path != manifest && crate::app::file_extension::has_extension(&path, "snxlib")
        })
    })
}

fn untouched_known_folders(dir: Option<&Path>) -> Vec<PathBuf> {
    let Some(dir) = dir else {
        return Vec::new();
    };
    known_folders()
        .iter()
        .map(|(folder, _)| dir.join(folder))
        .filter(|p| p.is_dir())
        .collect()
}

/// A real directory: not a symlink or junction leading somewhere else.
fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_dir())
}

/// Remove the known artifacts under `dir`, then `dir` itself if that left it
/// empty. Best effort: a failure leaves the entry and lists it.
pub(crate) fn remove_known_artifacts(dir: &Path) -> Vec<PathBuf> {
    let mut leftovers = Vec::new();
    for (folder, extensions) in known_folders() {
        let path = dir.join(folder);
        let top = dir.join(folder.split('/').next().unwrap_or(folder));
        if !is_real_dir(&top) || !is_real_dir(&path) {
            // A link may lead outside the project: never followed.
            if path.exists() {
                leftovers.push(top);
            }
            continue;
        }
        remove_known_files(&path, extensions, &mut leftovers);
        remove_if_empty(&path, &mut leftovers);
    }
    for folder in KNOWN_PARENTS {
        remove_if_empty(&dir.join(folder), &mut leftovers);
    }
    // `.git` and `.gitattributes` are never removed here.
    let mut remaining = false;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            remaining = true;
            let path = entry.path();
            if !leftovers.iter().any(|p| p.starts_with(&path)) {
                leftovers.push(path);
            }
        }
    }
    if !remaining && fs::remove_dir(dir).is_err() {
        leftovers.push(dir.to_path_buf());
    }
    leftovers.sort();
    leftovers
}

fn remove_known_files(folder: &Path, extensions: &[&str], leftovers: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Only plain files: a symlink or sub-directory is not ours to follow.
        let is_file = entry.file_type().is_ok_and(|t| t.is_file());
        let known = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| extensions.iter().any(|k| k.eq_ignore_ascii_case(e)));
        if is_file && known && fs::remove_file(&path).is_ok() {
            continue;
        }
        leftovers.push(path);
    }
}

/// Non-recursive: refuses (and the folder stays) when anything is inside.
fn remove_if_empty(folder: &Path, leftovers: &mut Vec<PathBuf>) {
    if is_real_dir(folder)
        && fs::remove_dir(folder).is_err()
        && !leftovers.iter().any(|p| p.starts_with(folder))
    {
        leftovers.push(folder.to_path_buf());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_files_survive_and_known_ones_go() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("lib");
        fs::create_dir_all(dir.join("symbols")).unwrap();
        fs::create_dir_all(dir.join("assets/datasheets")).unwrap();
        fs::write(dir.join("lib.snxlib"), b"m").unwrap();
        fs::write(dir.join("symbols/a.snxsym"), b"s").unwrap();
        fs::write(dir.join("symbols/notes.txt"), b"mine").unwrap();
        fs::write(dir.join("assets/datasheets/h.pdf"), b"p").unwrap();
        fs::write(dir.join("README.md"), b"mine").unwrap();

        let left = delete_library(&dir.join("lib.snxlib"), Some(&dir)).unwrap();

        assert!(!dir.join("lib.snxlib").exists());
        assert!(!dir.join("symbols/a.snxsym").exists());
        assert!(!dir.join("assets").exists());
        assert_eq!(fs::read(dir.join("symbols/notes.txt")).unwrap(), b"mine");
        assert_eq!(fs::read(dir.join("README.md")).unwrap(), b"mine");
        assert!(left.contains(&dir.join("README.md")));
    }

    #[test]
    fn project_models_beside_the_library_survive() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("hw");
        fs::create_dir_all(dir.join("models")).unwrap();
        fs::create_dir_all(dir.join("3dmodels")).unwrap();
        fs::write(dir.join("lib.snxlib"), b"m").unwrap();
        fs::write(dir.join("models/x.step"), b"pcb").unwrap();
        fs::write(dir.join("3dmodels/y.step"), b"lib").unwrap();

        delete_library(&dir.join("lib.snxlib"), Some(&dir)).unwrap();

        assert!(dir.join("models/x.step").exists());
        assert!(!dir.join("3dmodels").exists());
    }

    #[test]
    fn a_project_file_is_detected_case_insensitively() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(!holds_project_files(tmp.path()));
        fs::write(tmp.path().join("P.SNXPRJ"), b"p").unwrap();
        assert!(holds_project_files(tmp.path()));
    }

    #[test]
    fn an_emptied_library_folder_is_removed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("lib");
        fs::create_dir_all(dir.join("footprints")).unwrap();
        fs::write(dir.join("lib.snxlib"), b"m").unwrap();
        fs::write(dir.join("footprints/f.snxfpt"), b"f").unwrap();

        let left = delete_library(&dir.join("lib.snxlib"), Some(&dir)).unwrap();

        assert!(left.is_empty());
        assert!(!dir.exists());
    }

    #[test]
    fn a_sibling_library_keeps_its_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("libs");
        fs::create_dir_all(dir.join("symbols")).unwrap();
        fs::write(dir.join("A.snxlib"), b"a").unwrap();
        fs::write(dir.join("B.snxlib"), b"b").unwrap();
        fs::write(dir.join("symbols/b.snxsym"), b"s").unwrap();
        assert!(holds_other_library(&dir, &dir.join("A.snxlib")));

        let left = delete_library(&dir.join("A.snxlib"), None).unwrap();

        assert!(!dir.join("A.snxlib").exists());
        assert!(dir.join("B.snxlib").exists());
        assert!(dir.join("symbols/b.snxsym").exists());
        assert!(left.contains(&dir.join("symbols")));
    }

    #[test]
    fn the_libraries_own_lock_files_go() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("lib");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("lib.snxlib"), b"m").unwrap();
        fs::write(dir.join(".lib.snxlib.signex-writelock"), b"").unwrap();
        fs::write(dir.join(".lib.snxlib.signex-writelock.queue"), b"").unwrap();

        let left = delete_library(&dir.join("lib.snxlib"), Some(&dir)).unwrap();

        assert!(left.is_empty(), "{left:?}");
        assert!(!dir.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_folder_is_not_followed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let shared = tmp.path().join("shared");
        let dir = tmp.path().join("lib");
        fs::create_dir_all(&shared).unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(shared.join("x.snxsym"), b"s").unwrap();
        fs::write(dir.join("lib.snxlib"), b"m").unwrap();
        std::os::unix::fs::symlink(&shared, dir.join("symbols")).unwrap();

        let left = delete_library(&dir.join("lib.snxlib"), Some(&dir)).unwrap();

        assert!(shared.join("x.snxsym").exists());
        assert!(left.contains(&dir.join("symbols")));
    }
}
