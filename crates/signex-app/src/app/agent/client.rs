//! The client side of the agent channel (#1924), for `signex-cli`: find who
//! holds a project, attach to a running app, or hold the project for a
//! headless run. The registry, token and endpoint rules are the ones the app
//! writes (`registry`, `files`, `transport`), so there is one format; this file
//! only reads them with the checks a client owes. See `docs/AGENT_SOCKET.md`,
//! "Client".

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Map, Value};

use super::files::{self, REGISTRY_DIR};
use super::registry::{self, Entry, Kind, Owner};
use super::reply::{MessageLine, Status};
use super::token::boot_id;
use super::transport::endpoint_name;

/// How long a reply may take before the client gives up. The app answers
/// inside its update loop, so a long edit list can take a while.
const REPLY_TIMEOUT: Duration = Duration::from_secs(120);

/// How long a connect may take: a socket whose backlog is full blocks it.
#[cfg(unix)]
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// The longest reply line read, in bytes.
const MAX_REPLY_BYTES: u64 = 64 * 1024 * 1024;

/// The token is 64 hex characters; a file larger than this is not one.
const MAX_TOKEN_BYTES: u64 = 256;

/// Where the runtime directory is (see `files::runtime_dir`).
pub fn runtime_dir() -> Result<PathBuf, String> {
    files::runtime_dir()
}

/// Who holds a project, as its registry entry says.
#[derive(Debug)]
pub enum Lookup {
    /// No entry.
    Free,
    /// An entry nobody answers for: its process is gone, its boot id or
    /// endpoint is not the form this app writes, or it names another project.
    Stale(StaleEntry),
    /// A running app listens and has the project open.
    Gui(GuiPeer),
    /// A live headless run holds the project.
    Headless { pid: u32 },
    /// A running app has the project open and does not listen (agent
    /// connections are off): nothing to attach to, and not to be edited behind.
    GuiNotListening { pid: u32 },
}

/// A stale entry that may be removed with [`StaleEntry::remove`].
#[derive(Debug)]
pub struct StaleEntry {
    path: PathBuf,
    entry: Entry,
}

impl StaleEntry {
    /// Remove the entry, if it is still the one found stale and is this
    /// user's own plain file (never a link or another user's file). An entry
    /// rewritten since is left alone.
    pub fn remove(self) -> Result<(), String> {
        if registry::read_entry(&self.path).as_ref() != Some(&self.entry) {
            return Ok(());
        }
        files::remove_if_ours(&self.path)
    }
}

/// A running app that holds a project.
#[derive(Debug)]
pub struct GuiPeer {
    pub pid: u32,
    dir: PathBuf,
    boot_id: String,
}

/// The project's canonical path, which names its entry and its holders.
fn canonical(project: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(project).map_err(|error| format!("{}: {error}", project.display()))
}

/// Where the entry of `project` is in `dir`: its name is the digest of the
/// project's canonical path, the same rule the app writes it by.
pub fn entry_path(dir: &Path, project: &Path) -> Result<PathBuf, String> {
    let name = registry::entry_file_name(&canonical(project)?);
    Ok(dir.join(REGISTRY_DIR).join(name))
}

/// Read the entry of `project` in `dir`. The entry is found by the digest of
/// the project's canonical path, and what it names is never trusted: the
/// endpoint to connect to is the one its boot id gives, never its `socket`
/// text.
pub fn lookup(dir: &Path, project: &Path) -> Result<Lookup, String> {
    let canonical = canonical(project)?;
    match std::fs::symlink_metadata(dir) {
        Ok(_) => files::check_dir(dir)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Lookup::Free),
        Err(error) => return Err(format!("cannot inspect {}: {error}", dir.display())),
    }
    let path = entry_path(dir, &canonical)?;
    let Some(entry) = registry::read_entry(&path) else {
        return Ok(Lookup::Free);
    };
    // Not ours to judge: neither live nor stale, and never removed or replaced.
    if !registry::is_known(&entry) {
        return Err(format!(
            "{} is in format {}, newer than this signex-cli reads; not touching it",
            path.display(),
            entry.format
        ));
    }
    let names_this_project = entry.project == canonical.to_string_lossy();
    if !names_this_project || !registry::entry_is_live(dir, &entry) {
        return Ok(Lookup::Stale(StaleEntry { path, entry }));
    }
    Ok(match entry.kind {
        Kind::Headless => Lookup::Headless { pid: entry.pid },
        Kind::Gui if entry.socket.is_empty() => Lookup::GuiNotListening { pid: entry.pid },
        Kind::Gui => Lookup::Gui(GuiPeer {
            pid: entry.pid,
            dir: dir.to_path_buf(),
            boot_id: entry.boot_id,
        }),
    })
}

/// A headless run's hold on a project: an entry of kind `headless` that a GUI
/// opening the project sees, and an exclusive lock on the project's lock file
/// for the run's whole life. The entry is live while the lock is held, and the
/// kernel drops the lock with the process, so a run killed by a signal and a
/// reused process id are both found stale. Dropping the hold removes the entry;
/// the (empty) lock file stays.
#[derive(Debug)]
pub struct Hold {
    dir: PathBuf,
    owner: Owner,
    _lock: std::fs::File,
}

/// Hold `project` for this process. Fails if another headless run holds it,
/// a running app has it registered, or the entry cannot be written.
pub fn hold(dir: &Path, project: &Path) -> Result<Hold, String> {
    let canonical = canonical(project)?;
    files::prepare_dir(dir)?;
    let registry = dir.join(REGISTRY_DIR);
    files::prepare_dir(&registry)?;
    // The lock first: with it no other run holds the project, so an entry of a
    // headless run found here is a dead run's and is replaced.
    let lock_path = registry::lock_path(dir, &canonical);
    let entry_file = registry.join(registry::entry_file_name(&canonical));
    let lock = match registry::acquire_lock(&lock_path) {
        Ok(lock) => lock,
        // An app has the project open, whether or not it listens: it holds the
        // lock too.
        Err(why) => {
            let app = registry::read_entry(&entry_file)
                .filter(|entry| registry::is_known(entry) && entry.kind == Kind::Gui);
            return Err(match app {
                Some(entry) => format!(
                    "{} is also open in process {}, which keeps its entry",
                    canonical.display(),
                    entry.pid
                ),
                None => why,
            });
        }
    };
    if let Some(existing) = registry::read_entry(&entry_file) {
        if !registry::is_known(&existing) {
            return Err(format!(
                "{} is in a newer format; not replacing it",
                entry_file.display()
            ));
        }
        if existing.kind == Kind::Gui && registry::entry_is_live(dir, &existing) {
            return Err(format!(
                "{} is also open in process {}, which keeps its entry",
                canonical.display(),
                existing.pid
            ));
        }
    }
    let owner = Owner {
        pid: std::process::id(),
        boot_id: boot_id().ok_or("the system has no randomness")?,
        endpoint: String::new(),
    };
    let entry = Entry {
        format: registry::FORMAT,
        kind: Kind::Headless,
        project: canonical.to_string_lossy().into_owned(),
        pid: owner.pid,
        socket: String::new(),
        boot_id: owner.boot_id.clone(),
    };
    let text = serde_json::to_string_pretty(&entry).map_err(|error| error.to_string())?;
    files::write_private(&entry_file, text.as_bytes())?;
    Ok(Hold {
        dir: dir.to_path_buf(),
        owner,
        _lock: lock,
    })
}

impl Drop for Hold {
    /// Removes the entry. The lock file stays: deleting a locked file lets a
    /// second run lock the unlinked one (see `registry::acquire_lock`).
    fn drop(&mut self) {
        if let Err(why) = registry::remove_all(&self.dir, &self.owner, &mut registry::Locks::new())
        {
            crate::diagnostics::log_warning(format!("could not remove the project hold: {why}"));
        }
    }
}

impl GuiPeer {
    /// Connect to the app's endpoint and take its session token from the file
    /// beside it. The token file must be a plain file of this user that nobody
    /// else can read; the endpoint must be the socket (or pipe) the boot id
    /// names.
    pub fn connect(&self) -> Result<Connection, String> {
        let token = read_token(&self.dir.join(format!("{}.token", self.boot_id)))?;
        let stream = open_stream(&endpoint_name(&self.dir, &self.boot_id))?;
        Connection::new(stream, token)
    }
}

/// One open connection to a running app. Replies are read by a thread of
/// their own, so a wait for one is bounded on every platform.
pub struct Connection {
    writer: Stream,
    replies: mpsc::Receiver<Result<String, String>>,
    token: String,
    timeout: Duration,
    /// A request failed: see [`Connection::ask`].
    broken: bool,
}

impl Connection {
    fn new(stream: Stream, token: String) -> Result<Self, String> {
        let reader = stream
            .try_clone()
            .map_err(|error| format!("cannot read from the running Signex: {error}"))?;
        let (sender, replies) = mpsc::channel();
        // ast-grep-ignore: no-raw-spawn (the client runs in signex-cli, not in update())
        std::thread::spawn(move || read_replies(reader, &sender));
        Ok(Self {
            writer: stream,
            replies,
            token,
            timeout: REPLY_TIMEOUT,
            broken: false,
        })
    }

    /// Wait at most `timeout` for a reply, instead of the default.
    #[must_use]
    pub fn with_reply_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Send one command and read its reply. Replies are matched to requests
    /// by order, so after any failure (a timeout above all: the reply may still
    /// arrive) the connection is broken for good: a late reply must never be
    /// taken for the answer to the next request. Every later `ask` fails.
    pub fn ask(&mut self, id: &str, operands: &[(String, String)]) -> Result<Reply, String> {
        if self.broken {
            return Err(
                "the connection to the running Signex failed earlier in this run, so nothing \
                 more is sent on it"
                    .into(),
            );
        }
        let line = request_line(&self.token, id, operands)?;
        let outcome = self.exchange(id, &line);
        self.broken = outcome.is_err();
        outcome
    }

    fn exchange(&mut self, id: &str, line: &str) -> Result<Reply, String> {
        self.writer
            .write_all(line.as_bytes())
            .and_then(|()| self.writer.write_all(b"\n"))
            .and_then(|()| self.writer.flush())
            .map_err(|error| format!("sending to the running Signex: {error}"))?;
        let reply = match self.replies.recv_timeout(self.timeout) {
            Ok(reply) => parse_reply(reply?.trim_end())?,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(format!(
                    "the running Signex did not answer within {} s",
                    self.timeout.as_secs_f32()
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("the connection to the running Signex ended".into());
            }
        };
        // The app names the command it answers; another one is not our reply.
        match reply.id.as_deref() {
            Some(answered) if answered != id => Err(format!(
                "the running Signex answered `{answered}` to a request for `{id}`"
            )),
            _ => Ok(reply),
        }
    }
}

/// Send each reply line of `stream` until it ends or the connection is dropped.
fn read_replies(stream: Stream, sender: &mpsc::Sender<Result<String, String>>) {
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        let read = (&mut reader).take(MAX_REPLY_BYTES).read_line(&mut line);
        let item = match read {
            Ok(0) => Err("the running Signex closed the connection without a reply".to_string()),
            Ok(_) if !line.ends_with('\n') => {
                Err("the running Signex sent a reply that is cut off".to_string())
            }
            Ok(_) => Ok(line),
            Err(error) => Err(format!("reading the running Signex's reply: {error}")),
        };
        let last = item.is_err();
        if sender.send(item).is_err() || last {
            return;
        }
    }
}

/// Run `work` on a thread of its own and wait at most `limit` for it. A blocked
/// call is left behind (the thread ends when it returns), not waited for.
#[cfg(any(unix, test))]
fn within<T: Send + 'static>(
    limit: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (sender, receiver) = mpsc::channel();
    // ast-grep-ignore: no-raw-spawn (the client runs in signex-cli, not in update())
    std::thread::spawn(move || sender.send(work()).ok());
    receiver
        .recv_timeout(limit)
        .map_err(|_| format!("timed out after {} s", limit.as_secs_f32()))
}

/// One request line, without its newline: the token, the id and the operands
/// as the wire reads them (text, except the edit list, which is JSON).
pub fn request_line(
    token: &str,
    id: &str,
    operands: &[(String, String)],
) -> Result<String, String> {
    let mut args = Map::new();
    for (name, text) in operands {
        let value = if name == "edits" {
            serde_json::from_str(text).map_err(|error| format!("`edits` is not JSON: {error}"))?
        } else {
            Value::String(text.clone())
        };
        args.insert(name.clone(), value);
    }
    Ok(serde_json::json!({"token": token, "id": id, "args": args}).to_string())
}

/// What the app answered, in the headless runner's terms.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub id: Option<String>,
    pub status: Status,
    pub reason: Option<String>,
    pub data: Option<Value>,
    pub messages: Vec<MessageLine>,
    pub changed: Vec<String>,
}

/// One reply line of the app. Anything that is not the documented shape is an
/// error: a status is never guessed.
pub fn parse_reply(line: &str) -> Result<Reply, String> {
    let bad = |why: &str| format!("the running Signex sent a reply that is not understood: {why}");
    let Value::Object(mut object) = serde_json::from_str(line).map_err(|e| bad(&e.to_string()))?
    else {
        return Err(bad("not an object"));
    };
    let status = match object.get("status").and_then(Value::as_str) {
        Some("ran") => Status::Ran,
        Some("answered") => Status::Answered,
        Some("refused") => Status::Refused,
        Some("failed") => Status::Failed,
        _ => return Err(bad("no known status")),
    };
    let text = |value: Option<&Value>| value.and_then(Value::as_str).map(str::to_string);
    let messages = object
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("no messages"))?
        .iter()
        .map(|item| {
            let level = match item.get("level").and_then(Value::as_str) {
                Some("ERROR") => "ERROR",
                Some("WARN") => "WARN",
                Some("INFO") => "INFO",
                Some("DEBUG") => "DEBUG",
                Some("TRACE") => "TRACE",
                _ => return Err(bad("a message with no known level")),
            };
            let message = text(item.get("message")).ok_or_else(|| bad("a message with no text"))?;
            Ok(MessageLine { level, message })
        })
        .collect::<Result<_, String>>()?;
    let changed = object
        .get("changed")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("no changed list"))?
        .iter()
        .map(|item| text(Some(item)).ok_or_else(|| bad("a changed entry that is not text")))
        .collect::<Result<_, String>>()?;
    Ok(Reply {
        id: text(object.get("id")),
        status,
        reason: text(object.get("reason")),
        data: object.remove("data"),
        messages,
        changed,
    })
}

/// Read the session token at `path`. It must be a plain file (never a link)
/// of this user that no other user can read, holding 64 hex characters.
fn read_token(path: &Path) -> Result<String, String> {
    let why = |what: &dyn std::fmt::Display| {
        format!("not attaching: the session token {} {what}", path.display())
    };
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        // Never through a link, and never blocking on a pipe planted there.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(not(unix))]
    {
        if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(why(&"is a link"));
        }
    }
    let file = options
        .open(path)
        .map_err(|error| why(&format!("cannot be opened ({error})")))?;
    let meta = file.metadata().map_err(|error| why(&error))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        token_file_verdict(meta.is_file(), meta.uid(), meta.mode(), files::own_uid())
            .map_err(|reason| why(&reason))?;
    }
    #[cfg(not(unix))]
    {
        if !meta.is_file() {
            return Err(why(&"is not a plain file"));
        }
    }
    let mut text = String::new();
    file.take(MAX_TOKEN_BYTES)
        .read_to_string(&mut text)
        .map_err(|error| why(&error))?;
    let is_token = text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit());
    if is_token {
        Ok(text)
    } else {
        Err(why(&"is not a session token"))
    }
}

/// Whether a token file may be read: a plain file of this user with no
/// group or other permission.
#[cfg(unix)]
fn token_file_verdict(
    is_file: bool,
    owner: u32,
    mode: u32,
    own_uid: u32,
) -> Result<(), &'static str> {
    if !is_file {
        return Err("is not a plain file");
    }
    if owner != own_uid {
        return Err("is owned by another user");
    }
    if mode & 0o077 != 0 {
        return Err("can be read by other users (it must be mode 0600)");
    }
    Ok(())
}

#[cfg(unix)]
type Stream = std::os::unix::net::UnixStream;

/// Connect to the socket of the boot id, which must be a socket of this user
/// (not a link or a file put in its place).
#[cfg(unix)]
fn open_stream(endpoint: &str) -> Result<Stream, String> {
    use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};
    let meta = std::fs::symlink_metadata(endpoint)
        .map_err(|error| format!("not attaching: {endpoint}: {error}"))?;
    if !meta.file_type().is_socket() || meta.uid() != files::own_uid() {
        return Err(format!(
            "not attaching: {endpoint} is not a socket of this user"
        ));
    }
    let path = endpoint.to_string();
    let stream = within(CONNECT_TIMEOUT, move || Stream::connect(path))
        .map_err(|why| format!("cannot connect to {endpoint}: {why}"))?
        .map_err(|error| format!("cannot connect to {endpoint}: {error}"))?;
    stream
        .set_write_timeout(Some(REPLY_TIMEOUT))
        .map_err(|error| format!("cannot set a timeout on {endpoint}: {error}"))?;
    Ok(stream)
}

#[cfg(windows)]
type Stream = std::fs::File;

/// Open the pipe of the boot id for identification only, so a server that
/// squats the name cannot act as this user.
#[cfg(windows)]
fn open_stream(pipe: &str) -> Result<Stream, String> {
    use std::os::windows::fs::OpenOptionsExt as _;
    /// `SECURITY_IDENTIFICATION` (`SecurityIdentification << 16`).
    const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .security_qos_flags(SECURITY_IDENTIFICATION)
        .open(pipe)
        .map_err(|error| format!("cannot connect to {pipe}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_that_blocks_is_given_up_on_after_its_limit() {
        let slow = within(Duration::from_millis(50), || {
            std::thread::sleep(Duration::from_secs(5));
        });
        assert!(slow.unwrap_err().contains("timed out"));
        let fast = within(Duration::from_secs(5), || 7);
        assert_eq!(fast, Ok(7));
    }

    #[test]
    fn a_request_line_carries_the_token_the_id_texts_and_the_edit_list_as_json() {
        let edits = r#"[{"id":"place_junction","args":{"at":"1,2"}}]"#;
        let line = request_line(
            "tok",
            "apply_edits",
            &[
                ("document".into(), "doc".into()),
                ("edits".into(), edits.into()),
            ],
        )
        .unwrap();
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["token"], "tok");
        assert_eq!(value["id"], "apply_edits");
        assert_eq!(value["args"]["document"], "doc");
        assert_eq!(value["args"]["edits"][0]["id"], "place_junction");
        let bad = request_line("t", "apply_edits", &[("edits".into(), "nope".into())]);
        assert!(bad.unwrap_err().contains("`edits` is not JSON"));
    }

    #[test]
    fn a_reply_is_read_as_the_headless_result_and_anything_else_is_an_error() {
        let reply = parse_reply(
            r#"{"id":"x","status":"refused","reason":"why","data":{"n":1},
                "messages":[{"level":"WARN","message":"m"}],"changed":["a"]}"#,
        )
        .unwrap();
        assert_eq!(reply.id.as_deref(), Some("x"));
        assert_eq!(reply.status, Status::Refused);
        assert_eq!(reply.reason.as_deref(), Some("why"));
        assert_eq!(reply.data, Some(serde_json::json!({"n": 1})));
        assert_eq!(
            reply.messages,
            [MessageLine {
                level: "WARN",
                message: "m".into()
            }]
        );
        assert_eq!(reply.changed, ["a"]);
        for bad in [
            "not json",
            "[]",
            r#"{"status":"ok","messages":[],"changed":[]}"#,
            r#"{"status":"ran","changed":[]}"#,
            r#"{"status":"ran","messages":[{"level":"LOUD","message":"m"}],"changed":[]}"#,
            r#"{"status":"ran","messages":[],"changed":[1]}"#,
        ] {
            let why = parse_reply(bad).unwrap_err();
            assert!(why.contains("not understood"), "{bad}: {why}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn only_an_own_plain_private_file_is_a_token() {
        assert!(token_file_verdict(true, 501, 0o100600, 501).is_ok());
        assert!(token_file_verdict(true, 501, 0o100400, 501).is_ok());
        let foreign = token_file_verdict(true, 0, 0o100600, 501).unwrap_err();
        assert!(foreign.contains("another user"), "{foreign}");
        for wide in [0o100644, 0o100640, 0o100604, 0o100666] {
            let why = token_file_verdict(true, 501, wide, 501).unwrap_err();
            assert!(why.contains("0600"), "{wide:o}: {why}");
        }
        assert!(token_file_verdict(false, 501, 0o40700, 501).is_err());
    }
}
