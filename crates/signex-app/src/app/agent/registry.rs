//! The registry: one small file per open project in the runtime directory,
//! naming the process that holds it and where to reach it (#1924). A client
//! (`client`) reads the entry of its project, and connects to the endpoint
//! its boot id names with the token in `<boot_id>.token` beside it. The
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
/// 2 added `kind`; an entry without one is a format 1 entry of a GUI. 3: a
/// `gui` entry with an empty `socket` is an app that has the project open and
/// does not listen (agent connections off); a reader of format 2 would take it
/// for a stale one, so it must not read it at all.
pub(super) const FORMAT: u32 = 3;

/// The largest entry this app reads back while cleaning up.
const MAX_ENTRY_BYTES: u64 = 64 * 1024;

/// Who holds a project: a running app that listens, or a headless
/// `signex-cli` run, which has no endpoint.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(super) enum Kind {
    #[default]
    Gui,
    Headless,
}

/// One entry, as written to `<registry>/<digest of the project path>.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct Entry {
    pub(super) format: u32,
    #[serde(default)]
    pub(super) kind: Kind,
    /// The project file's canonical path.
    pub(super) project: String,
    pub(super) pid: u32,
    /// The Unix socket path, or the Windows pipe name; empty for a headless
    /// run.
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

/// [`sync`] for the write number `seq`, under `applied`, the number of the last
/// write that ran: writes run on worker threads and can reach this lock out of
/// order, and an older list must never be written after a newer one. A write
/// older than the last applied is dropped.
pub(super) fn sync_in_order(
    dir: &Path,
    owner: &Owner,
    projects: &[PathBuf],
    seq: u64,
    applied: &std::sync::Mutex<u64>,
) -> Result<(), String> {
    let mut last = applied
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if seq < *last {
        return Ok(());
    }
    *last = seq;
    sync(dir, owner, projects)
}

/// Write the entry of every project in `projects` and remove this run's other
/// entries (a closed project). Entries of other processes are never touched.
/// With an empty `owner.endpoint` the entry says the app has the project open
/// and does not listen. The runtime folder is made first, as `hold` does: with
/// agent connections off no listener ever makes it. A project that cannot be
/// resolved is named in the error, after the rest was done.
pub(super) fn sync(dir: &Path, owner: &Owner, projects: &[PathBuf]) -> Result<(), String> {
    files::prepare_dir(dir)?;
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
        // An entry a newer Signex wrote is not ours to judge or replace.
        if existing.as_ref().is_some_and(|entry| !is_known(entry)) {
            problems.push(format!(
                "{} has an entry in a newer format, which is left alone",
                canonical.display()
            ));
            continue;
        }
        let entry = Entry {
            format: FORMAT,
            kind: Kind::Gui,
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
        // The window holds the project's lock while it is open (`claim_shared`),
        // so no headless run has it: a headless entry here is a dead run's.
        // Another window's entry is judged by its process and its socket.
        let live = |entry: &Entry| entry.kind == Kind::Gui && entry_is_live(dir, entry);
        if !may_take_over(existing.as_ref(), owner, live) {
            problems.push(format!(
                "{} is also open in process {}, which keeps its entry",
                canonical.display(),
                existing.as_ref().map_or(0, |entry| entry.pid)
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
///
/// A headless run has no endpoint: its entry is live while its process holds
/// the project's lock file, a kernel lock that dies with the process, so a
/// killed run and a reused process id are both stale (see `lock_is_held`).
pub(super) fn entry_is_live(dir: &Path, entry: &Entry) -> bool {
    if !is_boot_id(&entry.boot_id) {
        return false;
    }
    match entry.kind {
        Kind::Headless => {
            entry.socket.is_empty() && lock_is_held(&lock_path(dir, Path::new(&entry.project)))
        }
        // An app that has the project open and does not listen (agent
        // connections off). From another window's side only its process can be
        // judged (a client judges by the lock, see `client::lookup`).
        Kind::Gui if entry.socket.is_empty() => pid_alive(entry.pid),
        Kind::Gui => attachable(dir, entry),
    }
}

/// Whether a client may connect to the app an entry names: its boot id has the
/// right form, `socket` is the endpoint that boot id gives, its process runs
/// and the endpoint answers.
pub(super) fn attachable(dir: &Path, entry: &Entry) -> bool {
    is_boot_id(&entry.boot_id)
        && !entry.socket.is_empty()
        && entry.socket == endpoint_name(dir, &entry.boot_id)
        && pid_alive(entry.pid)
        && endpoint_answers(&entry.socket)
}

/// The process of a live headless run that holds `canonical` (a project's
/// canonical path), and the entry file that says so, if one does. A runtime
/// folder that is not ours and private, or an entry that is not a plain file of
/// ours in a format this app knows, holds nothing.
pub(super) fn held_by_headless(dir: &Path, canonical: &Path) -> Option<(u32, PathBuf)> {
    if let Err(why) = files::check_dir(dir) {
        tracing::debug!("agent registry not read: {why}");
        return None;
    }
    let path = dir.join(REGISTRY_DIR).join(entry_file_name(canonical));
    let entry = read_entry(&path).filter(is_known)?;
    (entry.kind == Kind::Headless && entry_is_live(dir, &entry)).then_some((entry.pid, path))
}

/// The process and entry file for the notice of an open refused because a
/// headless run holds the project's lock: `(0, entry file)` when no entry names
/// the run yet (it writes its entry just after it takes the lock).
pub(super) fn headless_holder(dir: &Path, canonical: &Path) -> (u32, PathBuf) {
    held_by_headless(dir, canonical)
        .unwrap_or_else(|| (0, dir.join(REGISTRY_DIR).join(entry_file_name(canonical))))
}

/// Whether this app reads the format of `entry`: a newer one is data of a
/// Signex that knows more, never judged stale and never replaced.
pub(super) fn is_known(entry: &Entry) -> bool {
    entry.format <= FORMAT
}

/// The lock file of the project at `canonical` in `dir`, beside its entry.
pub(super) fn lock_path(dir: &Path, canonical: &Path) -> PathBuf {
    let name = entry_file_name(canonical).replace(".json", ".lock");
    dir.join(REGISTRY_DIR).join(name)
}

/// Open (making it if need be, 0600, never through a link) and exclusively
/// lock the lock file at `path`. The lock is the run's life: the kernel drops
/// it when the process ends, however it ends. Fails if another process holds
/// it. A lock file is never deleted (an empty 0600 file is harmless): deleting
/// one while locked lets a second holder lock the unlinked file.
///
/// A reader's momentary lock in [`lock_is_held`] can make one attempt fail, so
/// a few are made with short pauses before the project counts as held.
pub(super) fn acquire_lock(path: &Path) -> Result<std::fs::File, String> {
    acquire_lock_with(path, Lock::Exclusive, LOCK_ATTEMPTS, LOCK_PAUSE, || {}).map_err(|why| {
        match why {
            LockError::Held => HELD_MESSAGE.to_string(),
            LockError::Other(text) => text,
        }
    })
}

const LOCK_ATTEMPTS: u32 = 5;
const LOCK_PAUSE: std::time::Duration = std::time::Duration::from_millis(20);

/// What a project lock says when it is refused.
pub(super) const HELD_MESSAGE: &str =
    "the project is held by another signex-cli run or open in a Signex";

/// Who may hold a project at once: any number of windows (shared), or one
/// headless run (exclusive), never both. A reader asks for exclusive without
/// waiting, which fails while anyone holds it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Lock {
    Shared,
    Exclusive,
}

pub(super) enum LockError {
    /// The other kind of holder has it.
    Held,
    Other(String),
}

/// A window's claim on the project at `canonical`: the shared lock, taken
/// synchronously when the project opens and held (by the returned file) for as
/// long as it stays open. Fails with `Held` while a headless run holds it. The
/// runtime folder is made first, as for any hold.
pub(super) fn claim_shared(dir: &Path, canonical: &Path) -> Result<std::fs::File, LockError> {
    files::prepare_dir(dir).map_err(LockError::Other)?;
    files::prepare_dir(&dir.join(REGISTRY_DIR)).map_err(LockError::Other)?;
    // One attempt, no pause: this runs inside the update (a pause there stalls
    // the window), and it is retried at the registry tick. Only the CLI's
    // `acquire_lock` waits out a reader's momentary lock.
    acquire_lock_with(
        &lock_path(dir, canonical),
        Lock::Shared,
        1,
        std::time::Duration::ZERO,
        || {},
    )
}

/// [`acquire_lock`] for either kind, with its attempts and pause; `before_verify`
/// runs between the lock and the check that the path still names the file (a
/// test swaps the file there).
pub(super) fn acquire_lock_with(
    path: &Path,
    how: Lock,
    attempts: u32,
    pause: std::time::Duration,
    mut before_verify: impl FnMut(),
) -> Result<std::fs::File, LockError> {
    for attempt in 1..=attempts {
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options.open(path).map_err(|error| {
            LockError::Other(format!("cannot open {}: {error}", path.display()))
        })?;
        let meta = file
            .metadata()
            .map_err(|error| LockError::Other(error.to_string()))?;
        if !meta.is_file() || !owned_by_us(&meta) {
            return Err(LockError::Other(format!(
                "{} is not a plain file of this user",
                path.display()
            )));
        }
        let locked = match how {
            Lock::Exclusive => file.try_lock(),
            Lock::Shared => file.try_lock_shared(),
        };
        match locked {
            Ok(()) => {
                before_verify();
                // The path must still name the file that is locked: one that was
                // replaced since the open is a lock on nothing anyone else sees.
                if names_same_file(&file, path) {
                    return Ok(file);
                }
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                if attempt < attempts {
                    std::thread::sleep(pause);
                }
            }
            Err(std::fs::TryLockError::Error(error)) => {
                return Err(LockError::Other(format!(
                    "cannot lock {}: {error}",
                    path.display()
                )));
            }
        }
    }
    Err(LockError::Held)
}

/// Whether `path` still names the file `file` was opened as. (Where an open
/// file cannot be deleted or replaced, it always does.)
#[cfg(unix)]
fn names_same_file(file: &std::fs::File, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    match (file.metadata(), std::fs::symlink_metadata(path)) {
        (Ok(opened), Ok(named)) => opened.dev() == named.dev() && opened.ino() == named.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn names_same_file(_file: &std::fs::File, _path: &Path) -> bool {
    true
}

/// Is the lock file at `path` locked, shared or exclusive, by a living
/// process? An exclusive try fails while anyone holds it; taking it proves
/// every holder is gone (it is released at once); a file that is missing or not
/// a plain file of ours holds nothing.
pub(super) fn lock_is_held(path: &Path) -> bool {
    open_plain_lock_file(path)
        .is_some_and(|file| matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock)))
}

/// Is the lock file at `path` held EXCLUSIVELY (by a headless run)? A shared
/// holder (another window) does not count: a shared try fails only while an
/// exclusive lock is held.
pub(super) fn lock_is_held_exclusively(path: &Path) -> bool {
    open_plain_lock_file(path).is_some_and(|file| {
        matches!(
            file.try_lock_shared(),
            Err(std::fs::TryLockError::WouldBlock)
        )
    })
}

/// The lock file at `path`, opened without following a link or blocking, if it
/// is a plain file of ours.
fn open_plain_lock_file(path: &Path) -> Option<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).ok()?;
    file.metadata()
        .is_ok_and(|meta| meta.is_file() && owned_by_us(&meta))
        .then_some(file)
}

/// A headless run holding `canonical` exclusively in one of the user's other
/// runtime folders (`alternates`): its process (`0` while it has not written its
/// entry) and entry file. A folder that is not ours and private is not looked
/// at (the same rule as every reader).
pub(super) fn held_by_headless_in(
    alternates: &[PathBuf],
    canonical: &Path,
) -> Option<(u32, PathBuf)> {
    alternates
        .iter()
        .filter(|dir| files::check_dir(dir).is_ok())
        .find(|dir| lock_is_held_exclusively(&lock_path(dir, canonical)))
        .map(|dir| headless_holder(dir, canonical))
}

/// Whether `meta` is a file of this user (where the system says who owns it).
#[cfg(unix)]
fn owned_by_us(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    meta.uid() == files::own_uid()
}

/// Where the system does not say who owns a file, the user's profile folder is
/// private by its ACL (see `files::runtime_dir`).
#[cfg(not(unix))]
fn owned_by_us(_meta: &std::fs::Metadata) -> bool {
    true
}

/// Does a socket answer a connection? The connect does not block, so a peer
/// that does not accept cannot hang it. Where it would block, Linux answers
/// `EAGAIN` (or `EINPROGRESS`), which counts as there. macOS answers
/// `ECONNREFUSED` for a full backlog, so a stalled peer whose backlog has
/// filled may be taken for a dead one there; the only effect is that the
/// registry then points at this instance. The connection is closed at once and
/// the descriptor is close-on-exec.
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
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let kind = libc::SOCK_STREAM | libc::SOCK_CLOEXEC;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let kind = libc::SOCK_STREAM;
        let fd = libc::socket(libc::AF_UNIX, kind, 0);
        if fd < 0 {
            return false;
        }
        // Close-on-exec at once where `socket` could not set it, and
        // non-blocking; either failing ends the probe.
        let flags = libc::fcntl(fd, libc::F_GETFL);
        let prepared = flags >= 0
            && libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) >= 0
            && libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) >= 0;
        if !prepared {
            libc::close(fd);
            return false;
        }
        let size = std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
        let connected = libc::connect(fd, (&raw const address).cast(), size) == 0;
        let error = std::io::Error::last_os_error().raw_os_error();
        libc::close(fd);
        connect_answered(connected, error)
    }
}

/// What a non-blocking connect's result says: it connected, or it would
/// block (the peer exists but is busy). Anything else (no file, refused) is
/// no peer.
#[cfg(unix)]
fn connect_answered(connected: bool, error: Option<i32>) -> bool {
    connected || matches!(error, Some(libc::EINPROGRESS | libc::EAGAIN))
}

/// Does a pipe answer a connection? The pipe is opened for identification
/// only: with the default (impersonation) a server that squats a stale
/// entry's name could act as the probing user.
#[cfg(windows)]
fn endpoint_answers(pipe: &str) -> bool {
    use std::os::windows::fs::OpenOptionsExt as _;
    /// `ERROR_PIPE_BUSY`: the pipe exists and all its instances are in use.
    const PIPE_BUSY: i32 = 231;
    /// `SECURITY_IDENTIFICATION` (`SecurityIdentification << 16`); `std` adds
    /// `SECURITY_SQOS_PRESENT` itself.
    const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;
    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .security_qos_flags(SECURITY_IDENTIFICATION)
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

/// Remove every entry of this run (the app is quitting, or a headless run
/// ends). The lock files stay.
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
        // By process: an entry this process wrote under an earlier boot id (the
        // setting went off and on) is still its own.
        if read_entry(&path).is_some_and(|entry| is_known(&entry) && entry.pid == owner.pid)
            && let Err(why) = files::remove_if_ours(&path)
        {
            problems.push(why);
        }
    }
    problems
}

/// The entry in the file at `path`, if it is a small plain file of this user
/// holding one. The file is opened without following a link and without
/// blocking (a FIFO put in its place is not opened for reading), and checked
/// on the descriptor it was opened as, so a swap after the open changes nothing.
pub(super) fn read_entry(path: &Path) -> Option<Entry> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(not(unix))]
    {
        if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return None;
        }
    }
    let file = options.open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > MAX_ENTRY_BYTES || !owned_by_us(&meta) {
        return None;
    }
    let mut text = String::new();
    file.take(MAX_ENTRY_BYTES).read_to_string(&mut text).ok()?;
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

    /// A scratch folder only this user can enter, as the runtime folder is
    /// (the registry write refuses one others can enter).
    fn scratch() -> tempfile::TempDir {
        let dir = tempfile::Builder::new()
            .prefix("snx-1924-registry")
            .tempdir()
            .expect("temp dir");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        dir
    }

    #[cfg(unix)]
    fn private_scratch() -> tempfile::TempDir {
        scratch()
    }

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir.join(REGISTRY_DIR))
            .unwrap()
            .flatten()
            .map(|item| item.file_name().to_string_lossy().into_owned())
            // The entries; a project's (empty) lock file stays once made.
            .filter(|name| name.ends_with(".json"))
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
                kind: Kind::Gui,
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
            kind: Kind::Gui,
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

    /// A full backlog cannot be driven the same way on every system (macOS
    /// answers `ECONNREFUSED`, Linux `EAGAIN`), so the reading of the connect's
    /// result is tested alone.
    #[cfg(unix)]
    #[test]
    fn a_connect_that_would_block_means_a_peer_is_there_and_a_refusal_does_not() {
        assert!(connect_answered(true, None));
        assert!(connect_answered(false, Some(libc::EAGAIN)));
        assert!(connect_answered(false, Some(libc::EINPROGRESS)));
        assert!(!connect_answered(false, Some(libc::ECONNREFUSED)));
        assert!(!connect_answered(false, Some(libc::ENOENT)));
        assert!(!connect_answered(false, None));
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
    fn an_entry_written_before_kinds_existed_reads_as_a_gui_holder() {
        let old = r#"{"format":1,"project":"/p","pid":7,"socket":"/s","boot_id":"bb"}"#;
        let entry: Entry = serde_json::from_str(old).unwrap();
        assert_eq!(entry.kind, Kind::Gui);
        let written = serde_json::to_value(&entry).unwrap();
        assert_eq!(written["kind"], "gui");
        let headless = Entry {
            kind: Kind::Headless,
            ..entry
        };
        assert_eq!(serde_json::to_value(&headless).unwrap()["kind"], "headless");
    }

    /// Write the entry of a headless run `pid` of `format` for the project `a`.
    #[cfg(unix)]
    fn plant_headless(dir: &Path, a: &Path, pid: u32, format: u32) -> PathBuf {
        let registry = dir.join(REGISTRY_DIR);
        files::prepare_dir(&registry).unwrap();
        let canonical = std::fs::canonicalize(a).unwrap();
        let entry = Entry {
            format,
            kind: Kind::Headless,
            project: canonical.to_string_lossy().into_owned(),
            pid,
            socket: String::new(),
            boot_id: CC.into(),
        };
        let path = registry.join(entry_file_name(&canonical));
        std::fs::write(&path, serde_json::to_string(&entry).unwrap()).unwrap();
        path
    }

    /// The headless run's lock on the project `a`, held by this test.
    #[cfg(unix)]
    fn hold_lock(dir: &Path, a: &Path) -> std::fs::File {
        let canonical = std::fs::canonicalize(a).unwrap();
        acquire_lock(&lock_path(dir, &canonical)).expect("a lock")
    }

    /// A headless entry is live while its lock is held, whatever its pid
    /// says: process 1 is always running, and holds nothing here.
    #[cfg(unix)]
    #[test]
    fn a_headless_hold_is_live_while_its_lock_is_held_and_stale_once_it_is_not() {
        let dir = private_scratch();
        files::prepare_dir(&dir.path().join(REGISTRY_DIR)).unwrap();
        let a = project(dir.path(), "a.snxprj");
        let canonical = std::fs::canonicalize(&a).unwrap();
        let entry = plant_headless(dir.path(), &a, 1, FORMAT);
        let lock = hold_lock(dir.path(), &a);
        assert_eq!(held_by_headless(dir.path(), &canonical), Some((1, entry)));
        // A window cannot open what a run holds.
        assert!(matches!(
            claim_shared(dir.path(), &canonical),
            Err(LockError::Held)
        ));
        assert_eq!(headless_holder(dir.path(), &canonical).0, 1);

        // The holder is gone: the kernel dropped its lock. Its pid is reused
        // (process 1 runs), which makes no difference.
        drop(lock);
        assert!(pid_alive(1));
        assert_eq!(held_by_headless(dir.path(), &canonical), None);
        sync(dir.path(), &owner(41, "bb"), std::slice::from_ref(&a)).expect("taken over");
        assert_eq!(entry_pid(dir.path(), &a), 41);
        // An app's entry is never a headless hold.
        assert_eq!(held_by_headless(dir.path(), &canonical), None);
    }

    /// Any number of windows share a project; a headless run shares it with
    /// none, in either order.
    #[cfg(unix)]
    #[test]
    fn windows_share_a_project_and_a_headless_run_shares_it_with_none() {
        let dir = private_scratch();
        let a = project(dir.path(), "a.snxprj");
        let canonical = std::fs::canonicalize(&a).unwrap();
        let window = || match claim_shared(dir.path(), &canonical) {
            Ok(lock) => lock,
            Err(_) => panic!("a window could not claim the project"),
        };
        let exclusive = || {
            acquire_lock_with(
                &lock_path(dir.path(), &canonical),
                Lock::Exclusive,
                1,
                std::time::Duration::ZERO,
                || {},
            )
        };
        let (first, second) = (window(), window());
        assert!(lock_is_held(&lock_path(dir.path(), &canonical)));
        assert!(
            matches!(exclusive(), Err(LockError::Held)),
            "a run took an open project"
        );
        // One window closes: the project is still open in the other.
        drop(first);
        assert!(lock_is_held(&lock_path(dir.path(), &canonical)));
        assert!(
            matches!(exclusive(), Err(LockError::Held)),
            "a gap while one window is open"
        );
        drop(second);
        let run = exclusive().ok().expect("the run, with no window");
        // And a window cannot open what a run holds.
        assert!(matches!(
            claim_shared(dir.path(), &canonical),
            Err(LockError::Held)
        ));
        drop(run);
    }

    /// The same-file check is part of acquiring: a lock taken on a file that
    /// was replaced meanwhile is not returned.
    #[cfg(unix)]
    #[test]
    fn a_lock_on_a_file_replaced_meanwhile_is_not_returned() {
        let dir = private_scratch();
        files::prepare_dir(&dir.path().join(REGISTRY_DIR)).unwrap();
        let a = project(dir.path(), "a.snxprj");
        let path = lock_path(dir.path(), &std::fs::canonicalize(&a).unwrap());
        let swapped = std::cell::Cell::new(false);
        let file = acquire_lock_with(&path, Lock::Exclusive, 3, std::time::Duration::ZERO, || {
            if !swapped.replace(true) {
                std::fs::remove_file(&path).unwrap();
                std::fs::write(&path, "").unwrap();
            }
        })
        .ok()
        .expect("the second attempt locks the new file");
        assert!(names_same_file(&file, &path), "a lock on an unlinked file");
    }

    /// Writes reach the worker out of order: an older list never follows a
    /// newer one.
    #[test]
    fn an_older_registry_write_never_follows_a_newer_one() {
        let dir = scratch();
        let (a, b) = (
            project(dir.path(), "a.snxprj"),
            project(dir.path(), "b.snxprj"),
        );
        let ours = owner(41, "bb");
        let applied = std::sync::Mutex::new(0_u64);
        sync_in_order(dir.path(), &ours, &[a.clone(), b], 2, &applied).unwrap();
        assert_eq!(entries(dir.path()).len(), 2);
        // The older write (one project fewer) arrives late and is dropped.
        sync_in_order(dir.path(), &ours, &[a], 1, &applied).unwrap();
        assert_eq!(
            entries(dir.path()).len(),
            2,
            "an older list removed an entry"
        );
    }

    /// With agent connections off no listener makes the runtime folder: the
    /// listing makes it, or nothing is listed on a fresh boot.
    #[test]
    fn the_registry_write_makes_the_runtime_folder() {
        let root = scratch();
        let dir = root.path().join("not-made-yet");
        let a = project(root.path(), "a.snxprj");
        sync(&dir, &owner(41, "bb"), std::slice::from_ref(&a)).expect("written");
        assert_eq!(entries(&dir).len(), 1);
    }

    /// A second holder cannot take the lock, and the file is not a lock when
    /// it is a link.
    #[cfg(unix)]
    #[test]
    fn the_lock_is_exclusive_and_never_taken_through_a_link() {
        let dir = scratch();
        files::prepare_dir(&dir.path().join(REGISTRY_DIR)).unwrap();
        let a = project(dir.path(), "a.snxprj");
        let first = hold_lock(dir.path(), &a);
        let canonical = std::fs::canonicalize(&a).unwrap();
        let second = acquire_lock(&lock_path(dir.path(), &canonical)).unwrap_err();
        assert!(
            second.contains("held by another signex-cli run"),
            "{second}"
        );
        drop(first);

        let path = lock_path(dir.path(), &canonical);
        std::fs::remove_file(&path).unwrap();
        let target = dir.path().join("elsewhere");
        std::fs::write(&target, "x").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(acquire_lock(&path).is_err(), "a link was locked");
        assert!(!lock_is_held(&path));
    }

    /// The window's claim runs inside the update: one attempt, no waiting (the
    /// CLI's `acquire_lock` is the one that waits out a reader's momentary lock).
    #[cfg(unix)]
    #[test]
    fn the_windows_claim_does_not_wait_inside_the_update() {
        let dir = scratch();
        files::prepare_dir(&dir.path().join(REGISTRY_DIR)).unwrap();
        let a = project(dir.path(), "a.snxprj");
        let _run = hold_lock(dir.path(), &a);
        let canonical = std::fs::canonicalize(&a).unwrap();
        // The retries would sleep 4 x 20 ms; the quickest of a few tries is
        // far under that unless the claim waits.
        let quickest = (0..3)
            .map(|_| {
                let started = std::time::Instant::now();
                assert!(matches!(
                    claim_shared(dir.path(), &canonical),
                    Err(LockError::Held)
                ));
                started.elapsed()
            })
            .min()
            .unwrap();
        assert!(quickest < LOCK_PAUSE * 3, "the claim waited: {quickest:?}");
        // A reader's momentary lock does not make a shared claim fail for good:
        // with nobody holding it, it is taken at once.
        drop(_run);
        assert!(claim_shared(dir.path(), &canonical).is_ok());
    }

    /// A lock on a file that was deleted or replaced is a lock on nothing: the
    /// path must still name the locked file, or the attempt is not a hold.
    #[cfg(unix)]
    #[test]
    fn a_lock_counts_only_while_the_path_still_names_the_locked_file() {
        let dir = private_scratch();
        files::prepare_dir(&dir.path().join(REGISTRY_DIR)).unwrap();
        let a = project(dir.path(), "a.snxprj");
        let path = lock_path(dir.path(), &std::fs::canonicalize(&a).unwrap());
        let lock = acquire_lock(&path).expect("a lock");
        assert!(names_same_file(&lock, &path));
        assert!(lock_is_held(&path));

        // What an earlier version did on drop: delete the file while locked.
        std::fs::remove_file(&path).unwrap();
        assert!(
            !names_same_file(&lock, &path),
            "a deleted file is still named"
        );
        std::fs::write(&path, "").unwrap();
        assert!(
            !names_same_file(&lock, &path),
            "a replaced file is still named"
        );
        assert!(
            !lock_is_held(&path),
            "a lock on the old file holds the new one"
        );
    }

    /// A reader's momentary lock must not make a starting run think the
    /// project is held: the attempt is repeated a few times.
    #[cfg(unix)]
    #[test]
    fn a_readers_momentary_lock_does_not_fail_a_starting_run() {
        let dir = private_scratch();
        files::prepare_dir(&dir.path().join(REGISTRY_DIR)).unwrap();
        let a = project(dir.path(), "a.snxprj");
        let path = lock_path(dir.path(), &std::fs::canonicalize(&a).unwrap());
        drop(acquire_lock(&path).unwrap());

        let reader = std::fs::File::open(&path).unwrap();
        reader.try_lock().expect("the reader's lock");
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(40));
            drop(reader);
        });
        // One attempt finds it held; the run's own few attempts outlast it.
        let once = acquire_lock_with(&path, Lock::Exclusive, 1, std::time::Duration::ZERO, || {});
        assert!(matches!(once.unwrap_err(), LockError::Held));
        let held = acquire_lock(&path);
        releaser.join().unwrap();
        assert!(held.is_ok(), "a reader's momentary lock failed the run");
    }

    /// An entry in a format newer than this app's is not its to judge: it is
    /// no hold, is not replaced and is not removed.
    #[cfg(unix)]
    #[test]
    fn an_entry_in_a_newer_format_is_left_alone_by_every_reader() {
        let dir = scratch();
        let a = project(dir.path(), "a.snxprj");
        let canonical = std::fs::canonicalize(&a).unwrap();
        let entry = plant_headless(dir.path(), &a, std::process::id(), FORMAT + 1);
        let before = std::fs::read(&entry).unwrap();
        let _lock = hold_lock(dir.path(), &a);

        assert_eq!(held_by_headless(dir.path(), &canonical), None);
        let why = sync(dir.path(), &owner(41, "bb"), std::slice::from_ref(&a)).unwrap_err();
        assert!(why.contains("newer format"), "{why}");
        // Its pid and boot id are the ones of this run: still not removed.
        let ours = Owner {
            pid: std::process::id(),
            boot_id: CC.into(),
            endpoint: String::new(),
        };
        remove_all(dir.path(), &ours).unwrap();
        assert_eq!(std::fs::read(&entry).unwrap(), before);
    }

    /// The GUI reads the registry on the update thread: a runtime folder that
    /// is not private, and an entry that is a link or a FIFO, hold nothing and
    /// block nothing.
    #[cfg(unix)]
    #[test]
    fn the_registry_is_read_only_from_a_private_folder_and_plain_entries() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = private_scratch();
        files::prepare_dir(&dir.path().join(REGISTRY_DIR)).unwrap();
        let a = project(dir.path(), "a.snxprj");
        let canonical = std::fs::canonicalize(&a).unwrap();
        let entry = plant_headless(dir.path(), &a, 1, FORMAT);
        let _lock = hold_lock(dir.path(), &a);
        assert!(held_by_headless(dir.path(), &canonical).is_some());

        // A folder others can enter is not trusted.
        let mode =
            |bits| std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(bits));
        mode(0o755).unwrap();
        assert_eq!(held_by_headless(dir.path(), &canonical), None);
        mode(0o700).unwrap();

        // A link in the entry's place is not followed.
        let plain = std::fs::read(&entry).unwrap();
        std::fs::remove_file(&entry).unwrap();
        let target = dir.path().join("target.json");
        std::fs::write(&target, &plain).unwrap();
        std::os::unix::fs::symlink(&target, &entry).unwrap();
        assert_eq!(held_by_headless(dir.path(), &canonical), None);
        assert!(read_entry(&entry).is_none());

        // A FIFO is not opened for reading: this returns instead of hanging.
        std::fs::remove_file(&entry).unwrap();
        let fifo = std::ffi::CString::new(entry.to_str().unwrap()).unwrap();
        // SAFETY: a valid NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert_eq!(held_by_headless(dir.path(), &canonical), None);
        assert!(read_entry(&entry).is_none());
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
            kind: Kind::Gui,
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
