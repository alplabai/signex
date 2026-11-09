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
use super::token::is_boot_id;
use super::transport::endpoint_name;

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
        // The other instance of this project that is alive keeps its entry.
        let existing = read_entry(&registry.join(&name));
        let entry = Entry {
            format: FORMAT,
            project: canonical.to_string_lossy().into_owned(),
            pid: owner.pid,
            socket: owner.endpoint.clone(),
            boot_id: owner.boot_id.clone(),
        };
        if existing.as_ref() == Some(&entry) {
            // Already written: a retry does not rewrite it.
            wanted.insert(name);
            continue;
        }
        if !may_take_over(existing.as_ref(), owner, |entry| entry_is_live(dir, entry)) {
            let held = existing.map_or(0, |entry| entry.pid);
            problems.push(format!(
                "{} is also open in process {held}, which keeps its entry",
                canonical.display()
            ));
            continue;
        }
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

/// Whether this run may write over `existing`, the entry already at its
/// file: there is none, it is ours (this process, or this boot id), or it is
/// stale. Another live instance keeps its entry. `live` says whether the
/// entry's instance is still there.
fn may_take_over(existing: Option<&Entry>, owner: &Owner, live: impl Fn(&Entry) -> bool) -> bool {
    existing.is_none_or(|entry| {
        entry.pid == owner.pid || entry.boot_id == owner.boot_id || !live(entry)
    })
}

/// Is the instance an entry names still there: its process runs (a reused
/// process id of an unrelated process is not enough) and its socket answers a
/// connection, which is closed at once without sending a byte. A dead
/// instance's entry is stale.
///
/// A file in a shared folder is not trusted for where to connect: the entry
/// must carry a boot id of the right form and name exactly the endpoint that
/// boot id gives in `dir`, and only that endpoint is probed. An entry that
/// names anything else (a link, another folder, a network path) is stale.
fn entry_is_live(dir: &Path, entry: &Entry) -> bool {
    is_boot_id(&entry.boot_id)
        && entry.socket == endpoint_name(dir, &entry.boot_id)
        && pid_alive(entry.pid)
        && endpoint_answers(&entry.socket)
}

/// Does a socket answer a connection? The connect does not block: a peer that
/// is too busy to accept (a full backlog, a stopped process) answers `EAGAIN`
/// or `EINPROGRESS` and counts as there. The connection is closed at once.
#[cfg(unix)]
fn endpoint_answers(socket: &str) -> bool {
    let path = socket.as_bytes();
    // SAFETY: plain socket calls on a descriptor made and closed here, and a
    // zeroed `sockaddr_un` filled within its bounds.
    unsafe {
        let mut address: libc::sockaddr_un = std::mem::zeroed();
        if path.len() >= address.sun_path.len() || path.contains(&0) {
            return false;
        }
        address.sun_family = libc::AF_UNIX as libc::sa_family_t;
        for (slot, byte) in address.sun_path.iter_mut().zip(path) {
            *slot = *byte as libc::c_char;
        }
        let fd = libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0);
        if fd < 0 {
            return false;
        }
        let flags = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        let size = std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
        let connected = libc::connect(fd, (&raw const address).cast(), size) == 0;
        let error = std::io::Error::last_os_error().raw_os_error();
        libc::close(fd);
        connected || matches!(error, Some(libc::EINPROGRESS | libc::EAGAIN))
    }
}

#[cfg(windows)]
fn endpoint_answers(pipe: &str) -> bool {
    /// `ERROR_PIPE_BUSY`: the pipe exists and all its instances are in use.
    const PIPE_BUSY: i32 = 231;
    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe)
    {
        Ok(_connected) => true,
        Err(error) => error.raw_os_error() == Some(PIPE_BUSY),
    }
}

/// Is a process with this id running? A process of another user counts: it
/// exists, though it cannot be signalled.
#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid == 0 {
        return false;
    }
    // SAFETY: signal 0 sends nothing; it only reports whether the process exists.
    let found = unsafe { libc::kill(pid, 0) } == 0;
    found || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, GetLastError, STILL_ACTIVE,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: plain calls; the handle is closed before returning.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code = 0_u32;
        let read = GetExitCodeProcess(process, &mut code);
        CloseHandle(process);
        read != 0 && code == STILL_ACTIVE as u32
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
    fn a_closed_project_loses_its_entry() {
        let dir = scratch();
        let (a, b) = (
            project(dir.path(), "a.snxprj"),
            project(dir.path(), "b.snxprj"),
        );
        let ours = owner(41, "bb");
        sync(dir.path(), &ours, &[a.clone(), b.clone()]).unwrap();
        assert_eq!(entries(dir.path()).len(), 2);

        sync(dir.path(), &ours, std::slice::from_ref(&a)).unwrap();
        assert_eq!(entries(dir.path()).len(), 1, "b's entry is gone");

        sync(dir.path(), &ours, &[]).unwrap();
        assert!(entries(dir.path()).is_empty(), "no project, no entry");
    }

    #[cfg(unix)]
    fn entry_pid(dir: &Path, project: &Path) -> u32 {
        let name = entry_file_name(&std::fs::canonicalize(project).unwrap());
        read_entry(&dir.join(REGISTRY_DIR).join(name))
            .expect("an entry")
            .pid
    }

    /// Process 1 (init, launchd) is always running and is not this process.
    #[cfg(unix)]
    #[test]
    fn a_live_other_instance_keeps_its_entry_and_our_close_does_not_remove_it() {
        let dir = scratch();
        let a = project(dir.path(), "a.snxprj");
        // Its process runs and the socket its boot id names answers.
        let (theirs, _listening) = listening_owner(dir.path(), 1, CC);
        let ours = owner(41, "bb");
        sync(dir.path(), &theirs, std::slice::from_ref(&a)).unwrap();

        let why = sync(dir.path(), &ours, std::slice::from_ref(&a)).unwrap_err();
        assert!(why.contains("also open in process 1"), "{why}");
        assert_eq!(entry_pid(dir.path(), &a), 1, "theirs is kept");

        // Our project closes, and the exit: theirs is still there.
        sync(dir.path(), &ours, &[]).unwrap();
        remove_all(dir.path(), &ours).unwrap();
        assert_eq!(entry_pid(dir.path(), &a), 1);
    }

    /// The process id of an entry can belong to an unrelated process by now
    /// (here, process 1); a socket that refuses a connection shows it.
    #[cfg(unix)]
    #[test]
    fn an_entry_whose_process_id_was_reused_and_whose_socket_is_dead_is_taken_over() {
        let dir = scratch();
        let a = project(dir.path(), "a.snxprj");
        // A socket file whose listener is gone refuses a connection.
        let (reused, listener) = listening_owner(dir.path(), 1, EE);
        drop(listener);
        sync(dir.path(), &reused, std::slice::from_ref(&a)).unwrap();
        assert!(pid_alive(1));
        assert!(!endpoint_answers(&reused.endpoint));

        sync(dir.path(), &owner(41, "bb"), std::slice::from_ref(&a)).expect("taken over");
        assert_eq!(entry_pid(dir.path(), &a), 41);
    }

    /// An entry file is data another process wrote: what it names is not
    /// probed unless it is exactly the endpoint its boot id gives here.
    #[cfg(unix)]
    #[test]
    fn an_entry_that_names_any_other_socket_or_a_malformed_boot_id_is_stale() {
        let dir = scratch();
        let a = project(dir.path(), "a.snxprj");
        // A live process, and a socket that answers, but not the boot id's.
        let (answering, _listener) = listening_owner(dir.path(), 1, CC);
        let planted = Owner {
            boot_id: EE.into(),
            ..answering.clone()
        };
        let planted_entry = Entry {
            format: FORMAT,
            project: "p".into(),
            pid: 1,
            socket: planted.endpoint.clone(),
            boot_id: planted.boot_id.clone(),
        };
        assert!(!entry_is_live(dir.path(), &planted_entry), "another socket");
        let live_entry = Entry {
            boot_id: CC.into(),
            ..planted_entry
        };
        assert!(entry_is_live(dir.path(), &live_entry), "its own socket");
        // The same socket under a boot id of the wrong form.
        let (odd, _odd_listener) = listening_owner(dir.path(), 1, "cc");
        let odd_entry = Entry {
            socket: odd.endpoint.clone(),
            boot_id: "cc".into(),
            ..live_entry
        };
        assert!(
            !entry_is_live(dir.path(), &odd_entry),
            "a malformed boot id"
        );

        sync(dir.path(), &planted, std::slice::from_ref(&a)).unwrap();
        sync(dir.path(), &owner(41, "bb"), std::slice::from_ref(&a)).expect("taken over");
        assert_eq!(entry_pid(dir.path(), &a), 41);
    }

    #[cfg(unix)]
    #[test]
    fn a_socket_answers_only_while_something_listens() {
        let dir = scratch();
        let (live, listener) = listening_owner(dir.path(), 1, CC);
        assert!(endpoint_answers(&live.endpoint));
        drop(listener);
        assert!(!endpoint_answers(&live.endpoint), "a dead socket file");
        assert!(!endpoint_answers(
            &dir.path().join("none").to_string_lossy()
        ));
        assert!(
            !endpoint_answers(&"x".repeat(500)),
            "too long for a socket path"
        );
    }

    /// Boot ids of the right form.
    #[cfg(unix)]
    const CC: &str = "cccccccccccccccc";
    #[cfg(unix)]
    const EE: &str = "eeeeeeeeeeeeeeee";

    /// An instance `pid` / `boot` listening on the socket its boot id names
    /// in `dir`.
    #[cfg(unix)]
    fn listening_owner(
        dir: &Path,
        pid: u32,
        boot: &str,
    ) -> (Owner, std::os::unix::net::UnixListener) {
        let socket = endpoint_name(dir, boot);
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let owner = Owner {
            pid,
            boot_id: boot.into(),
            endpoint: socket,
        };
        (owner, listener)
    }

    #[test]
    fn a_boot_id_has_one_form() {
        assert!(is_boot_id("0123456789abcdef"));
        assert!(!is_boot_id("0123456789ABCDEF"));
        assert!(!is_boot_id("0123456789abcde"));
        assert!(!is_boot_id("0123456789abcdef0"));
        assert!(!is_boot_id("../../etc/passwd"));
        assert!(!is_boot_id(""));
    }

    #[cfg(unix)]
    #[test]
    fn the_entry_of_a_process_that_is_gone_is_taken_over() {
        let dir = scratch();
        let a = project(dir.path(), "a.snxprj");
        // Above every system's process id limit, so no process has it.
        let dead = 2_000_000_000;
        assert!(!pid_alive(dead));
        assert!(pid_alive(std::process::id()));
        sync(dir.path(), &owner(dead, "dd"), std::slice::from_ref(&a)).unwrap();

        sync(dir.path(), &owner(41, "bb"), std::slice::from_ref(&a)).expect("taken over");
        assert_eq!(entry_pid(dir.path(), &a), 41);
    }

    #[test]
    fn only_a_live_process_that_is_not_us_keeps_an_entry() {
        let ours = owner(41, "bb");
        let entry = |pid: u32, boot: &str| Entry {
            format: FORMAT,
            project: "p".into(),
            pid,
            socket: "s".into(),
            boot_id: boot.into(),
        };
        let live = |_: &Entry| true;
        let gone = |_: &Entry| false;
        assert!(may_take_over(None, &ours, live), "no entry");
        assert!(
            may_take_over(Some(&entry(41, "zz")), &ours, live),
            "our process"
        );
        assert!(
            may_take_over(Some(&entry(7, "bb")), &ours, live),
            "our boot id"
        );
        assert!(
            may_take_over(Some(&entry(7, "cc")), &ours, gone),
            "a dead process"
        );
        assert!(
            !may_take_over(Some(&entry(7, "cc")), &ours, live),
            "a live one"
        );
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
