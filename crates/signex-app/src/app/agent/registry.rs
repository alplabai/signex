//! The registry: one small file per open project in the runtime directory,
//! naming the process that holds it and where to reach it (#1924). A client
//! (PR B of #1924) lists the folder, reads the entry of its project, and
//! connects to `socket` with the token in `<boot_id>.token` beside it. The
//! format is in `docs/AGENT_SOCKET.md`, "Registry".

use std::collections::BTreeSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::files::{self, REGISTRY_DIR};

/// The version of the entry format. A change that breaks a reader moves it.
pub(super) const FORMAT: u32 = 1;

/// The largest entry this app reads back while cleaning up.
const MAX_ENTRY_BYTES: u64 = 64 * 1024;

/// One entry, as written to `<registry>/<digest of the project path>.json`.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct Entry {
    pub(super) format: u32,
    /// The project file's canonical path.
    pub(super) project: String,
    pub(super) pid: u32,
    /// The Unix socket path, or the Windows pipe name.
    pub(super) socket: String,
    pub(super) boot_id: String,
}

/// This app run, as the entries name it.
#[derive(Debug, Clone)]
pub(super) struct Owner {
    pub(super) pid: u32,
    pub(super) boot_id: String,
    pub(super) endpoint: String,
}

/// The file name of the entry of the project at the canonical path: 32 hex
/// characters of its SHA-256, so the name is stable and holds no path text.
pub(super) fn entry_file_name(canonical: &Path) -> String {
    let digest = Sha256::digest(canonical.to_string_lossy().as_bytes());
    let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
    format!("{hex}.json")
}

/// Write the entry of every project in `projects` and remove this run's other
/// entries (a closed project). Entries of other processes are never touched.
/// A project that cannot be resolved is named in the error, after the rest
/// was done.
pub(super) fn sync(dir: &Path, owner: &Owner, projects: &[PathBuf]) -> Result<(), String> {
    let registry = dir.join(REGISTRY_DIR);
    files::prepare_dir(&registry)?;
    let mut wanted = BTreeSet::new();
    let mut problems = Vec::new();
    for project in projects {
        let canonical = match std::fs::canonicalize(project) {
            Ok(canonical) => canonical,
            Err(error) => {
                problems.push(format!("{}: {error}", project.display()));
                continue;
            }
        };
        let name = entry_file_name(&canonical);
        let entry = Entry {
            format: FORMAT,
            project: canonical.to_string_lossy().into_owned(),
            pid: owner.pid,
            socket: owner.endpoint.clone(),
            boot_id: owner.boot_id.clone(),
        };
        let text = serde_json::to_string_pretty(&entry).map_err(|error| error.to_string())?;
        match files::write_private(&registry.join(&name), text.as_bytes()) {
            Ok(()) => {
                wanted.insert(name);
            }
            Err(why) => problems.push(why),
        }
    }
    problems.extend(remove_own_except(&registry, owner, &wanted));
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// Remove every entry of this run (the listener closed, or the app is
/// quitting).
pub(super) fn remove_all(dir: &Path, owner: &Owner) -> Result<(), String> {
    let registry = dir.join(REGISTRY_DIR);
    if !registry.is_dir() {
        return Ok(());
    }
    let problems = remove_own_except(&registry, owner, &BTreeSet::new());
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// Remove the entries in `registry` this run wrote and that are not named in
/// `keep`; what could not be removed comes back as reasons.
fn remove_own_except(registry: &Path, owner: &Owner, keep: &BTreeSet<String>) -> Vec<String> {
    let listing = match std::fs::read_dir(registry) {
        Ok(listing) => listing,
        Err(error) => return vec![format!("cannot list {}: {error}", registry.display())],
    };
    let mut problems = Vec::new();
    for item in listing.flatten() {
        let name = item.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".json") || keep.contains(&name) {
            continue;
        }
        let path = item.path();
        if read_entry(&path)
            .is_some_and(|entry| entry.pid == owner.pid && entry.boot_id == owner.boot_id)
            && let Err(why) = files::remove_if_ours(&path)
        {
            problems.push(why);
        }
    }
    problems
}

/// The entry in the file at `path`, if it is a small regular file holding one.
fn read_entry(path: &Path) -> Option<Entry> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }
    let mut text = String::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_ENTRY_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(pid: u32, boot: &str) -> Owner {
        Owner {
            pid,
            boot_id: boot.into(),
            endpoint: format!("/run/{boot}.sock"),
        }
    }

    fn scratch() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("snx-1924-registry")
            .tempdir()
            .expect("temp dir")
    }

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir.join(REGISTRY_DIR))
            .unwrap()
            .flatten()
            .map(|item| item.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn project(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, "{}").unwrap();
        path
    }

    #[test]
    fn an_entry_is_named_from_the_canonical_path_and_holds_pid_socket_and_boot_id() {
        let dir = scratch();
        let a = project(dir.path(), "a.snxprj");
        sync(dir.path(), &owner(41, "bb"), std::slice::from_ref(&a)).expect("written");

        let canonical = std::fs::canonicalize(&a).unwrap();
        let name = entry_file_name(&canonical);
        assert_eq!(name.len(), 32 + ".json".len());
        assert_eq!(entries(dir.path()), std::slice::from_ref(&name));
        let text = std::fs::read_to_string(dir.path().join(REGISTRY_DIR).join(&name)).unwrap();
        let entry: Entry = serde_json::from_str(&text).unwrap();
        assert_eq!(
            entry,
            Entry {
                format: FORMAT,
                project: canonical.to_string_lossy().into_owned(),
                pid: 41,
                socket: "/run/bb.sock".into(),
                boot_id: "bb".into(),
            }
        );
        // The name holds a digest, not the path.
        assert!(!name.contains("snxprj"));
        // The same project reached through a link has the same entry.
        #[cfg(unix)]
        {
            let link = dir.path().join("link.snxprj");
            std::os::unix::fs::symlink(&a, &link).unwrap();
            let linked = std::fs::canonicalize(&link).unwrap();
            assert_eq!(entry_file_name(&linked), name);
        }
    }

    #[cfg(unix)]
    #[test]
    fn an_entry_file_is_private() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = scratch();
        let a = project(dir.path(), "a.snxprj");
        sync(dir.path(), &owner(41, "bb"), &[a]).unwrap();
        let registry = dir.path().join(REGISTRY_DIR);
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&registry), 0o700);
        let entry = registry.join(&entries(dir.path())[0]);
        assert_eq!(mode(&entry), 0o600);
    }

    #[test]
    fn a_closed_project_loses_its_entry_and_other_runs_keep_theirs() {
        let dir = scratch();
        let (a, b) = (
            project(dir.path(), "a.snxprj"),
            project(dir.path(), "b.snxprj"),
        );
        let (ours, theirs) = (owner(41, "bb"), owner(42, "cc"));
        sync(dir.path(), &theirs, std::slice::from_ref(&a)).unwrap();
        sync(dir.path(), &ours, &[a.clone(), b.clone()]).unwrap();
        // One file name per project: the later write is the entry of `a`.
        assert_eq!(entries(dir.path()).len(), 2);

        sync(dir.path(), &ours, std::slice::from_ref(&a)).unwrap();
        assert_eq!(entries(dir.path()).len(), 1, "b's entry is gone");

        sync(dir.path(), &ours, &[]).unwrap();
        assert!(entries(dir.path()).is_empty(), "no project, no entry");
    }

    #[test]
    fn exit_removes_every_entry_of_this_run_and_none_of_another() {
        let dir = scratch();
        let (a, b) = (
            project(dir.path(), "a.snxprj"),
            project(dir.path(), "b.snxprj"),
        );
        let (ours, theirs) = (owner(41, "bb"), owner(42, "cc"));
        sync(dir.path(), &theirs, std::slice::from_ref(&b)).unwrap();
        sync(dir.path(), &ours, std::slice::from_ref(&a)).unwrap();
        assert_eq!(entries(dir.path()).len(), 2);

        remove_all(dir.path(), &ours).unwrap();
        let left = entries(dir.path());
        assert_eq!(left, [entry_file_name(&std::fs::canonicalize(&b).unwrap())]);
        // Nothing to remove where no registry was ever made.
        remove_all(&dir.path().join("none"), &ours).unwrap();
    }

    #[test]
    fn a_project_that_cannot_be_resolved_is_reported_and_the_rest_is_written() {
        let dir = scratch();
        let a = project(dir.path(), "a.snxprj");
        let missing = dir.path().join("gone.snxprj");
        let why = sync(dir.path(), &owner(41, "bb"), &[missing, a]).unwrap_err();
        assert!(why.contains("gone.snxprj"), "{why}");
        assert_eq!(entries(dir.path()).len(), 1);
    }
}
