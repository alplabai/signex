//! What a cell may do (mockup row 200): read and write files inside its
//! project, and nothing else of the machine. Starting a program, opening a
//! network connection and opening a file outside the project are refused
//! with a `PermissionError` and one plain sentence; only that cell fails.
//!
//! The runtime reports each attempt as a Python audit event
//! ([`SandboxPolicy::check_event`]); this module decides. It is a guard
//! against a cell doing these by accident or by habit, not a wall against
//! code written to escape: `docs/NOTEBOOK_CELLS.md`, "Sandbox".

use serde::{Deserialize, Serialize};

/// Something a cell tried that the sandbox rules on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "access", rename_all = "snake_case")]
pub enum Access {
    /// Start a program: `subprocess.run`, `os.system`, `os.exec*`, ...
    StartProgram { call: String },
    /// Open a network connection or look up a host.
    Network { call: String },
    /// Open, list, create, change or remove a file or folder.
    File {
        call: String,
        path: String,
        write: bool,
    },
}

/// Why an access was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalKind {
    StartProgram,
    Network,
    OutsideProject,
}

/// A refused access: what the cell called and why it was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    pub kind: RefusalKind,
    /// The Python call, `subprocess.run`, or the event that named it.
    pub call: String,
}

impl Refusal {
    /// The `PermissionError` text the cell raises.
    pub fn error_text(&self) -> String {
        let why = match self.kind {
            RefusalKind::StartProgram => "cells cannot start programs",
            RefusalKind::Network => "cells cannot open network connections",
            RefusalKind::OutsideProject => "cells cannot open files outside this project",
        };
        format!("PermissionError: {why}.")
    }

    /// The sentence that names the call: `subprocess.run is blocked in
    /// notebook cells.`
    pub fn blocked_sentence(&self) -> String {
        format!("{} is blocked in notebook cells.", self.call)
    }
}

/// The sentence every refusal is explained with.
pub const SANDBOX_RULE: &str = "Notebook cells can read and write files inside this project \
     only. They cannot start programs (subprocess, os.system), open network connections, or \
     open files outside the project folder.";

/// The audit events that start a program, by prefix.
const PROGRAM_EVENTS: [&str; 9] = [
    "subprocess.",
    "os.system",
    "os.exec",
    "os.posix_spawn",
    "os.spawn",
    "os.fork",
    "os.startfile",
    "pty.spawn",
    "_winapi.CreateProcess",
];

/// The audit events that reach the network, by prefix.
const NETWORK_EVENTS: [&str; 7] = [
    "socket.",
    "urllib.Request",
    "http.client.",
    "ftplib.",
    "smtplib.",
    "poplib.",
    "imaplib.",
];

/// The audit events on a path, its first argument, and whether each
/// writes. `open` writes when its mode does.
const FILE_EVENTS: [(&str, bool); 14] = [
    ("open", false),
    ("os.listdir", false),
    ("os.scandir", false),
    ("glob.glob", false),
    ("os.chdir", false),
    ("os.mkdir", true),
    ("os.remove", true),
    ("os.rmdir", true),
    ("os.rename", true),
    ("os.truncate", true),
    ("os.chmod", true),
    ("os.symlink", true),
    ("os.link", true),
    ("shutil.rmtree", true),
];

/// The rules for one cell run: the project folder it may use and the
/// folders it may only read (the runtime's own library).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPolicy {
    project: Vec<String>,
    read_only: Vec<Vec<String>>,
}

impl SandboxPolicy {
    /// The policy of a cell in the project at `project_root`, an absolute
    /// path. The runtime hands over paths with links resolved; a relative
    /// path is taken from the project folder, where a cell runs.
    pub fn new(project_root: &str) -> Self {
        Self {
            project: components(project_root, &[]),
            read_only: Vec::new(),
        }
    }

    /// The policy with `folder` (absolute) open for reading too.
    pub fn with_read_only(mut self, folder: &str) -> Self {
        self.read_only.push(components(folder, &[]));
        self
    }

    /// Allow or refuse `access`.
    pub fn check(&self, access: &Access) -> Result<(), Refusal> {
        match access {
            Access::StartProgram { call } => Err(Refusal {
                kind: RefusalKind::StartProgram,
                call: call.clone(),
            }),
            Access::Network { call } => Err(Refusal {
                kind: RefusalKind::Network,
                call: call.clone(),
            }),
            Access::File { call, path, write } => {
                let path = components(path, &self.project);
                let inside = |root: &Vec<String>| path.starts_with(root);
                let allowed =
                    inside(&self.project) || (!write && self.read_only.iter().any(inside));
                if allowed {
                    Ok(())
                } else {
                    Err(Refusal {
                        kind: RefusalKind::OutsideProject,
                        call: call.clone(),
                    })
                }
            }
        }
    }

    /// Allow or refuse the audit event `event` with its arguments as text;
    /// an event the sandbox has no rule for is allowed.
    pub fn check_event(&self, event: &str, args: &[String]) -> Result<(), Refusal> {
        match classify(event, args) {
            Some(access) => self.check(&access),
            None => Ok(()),
        }
    }
}

/// The access a Python audit event `event` with `args` stands for, if the
/// sandbox rules on it.
pub fn classify(event: &str, args: &[String]) -> Option<Access> {
    let call = event.to_string();
    let named = |prefixes: &[&str]| prefixes.iter().any(|prefix| event.starts_with(prefix));
    if named(&PROGRAM_EVENTS) {
        return Some(Access::StartProgram { call });
    }
    if named(&NETWORK_EVENTS) {
        return Some(Access::Network { call });
    }
    let &(_, writes) = FILE_EVENTS.iter().find(|(name, _)| *name == event)?;
    let path = args.first()?.clone();
    // `open(3)`: a file already open, by number.
    if path.parse::<i64>().is_ok() {
        return None;
    }
    let write = writes || (event == "open" && args.get(1).is_some_and(|mode| writes_mode(mode)));
    Some(Access::File { call, path, write })
}

/// Whether an `open` mode writes: `w`, `a`, `x` or `+`.
fn writes_mode(mode: &str) -> bool {
    mode.contains(['w', 'a', 'x', '+'])
}

/// Whether the absolute path `path` is `folder` or lies inside it, by the
/// rules the sandbox reads paths with.
pub fn is_inside(path: &str, folder: &str) -> bool {
    let folder = components(folder, &[]);
    !folder.is_empty() && components(path, &[]).starts_with(&folder)
}

/// `path` as its folder names, with `.` and `..` applied, a Windows drive
/// letter kept in lower case and `\` read as `/`. A relative path is taken
/// from `base`.
fn components(path: &str, base: &[String]) -> Vec<String> {
    let path = path.replace('\\', "/");
    let absolute = path.starts_with('/') || has_drive(&path);
    let mut parts: Vec<String> = if absolute { Vec::new() } else { base.to_vec() };
    for (index, part) in path.split('/').enumerate() {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            drive if index == 0 && has_drive(drive) => parts.push(drive.to_ascii_lowercase()),
            other => parts.push(other.to_string()),
        }
    }
    parts
}

fn has_drive(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}
