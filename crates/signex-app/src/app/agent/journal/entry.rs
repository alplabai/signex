//! One line of a journal as data (#2253): the edit entry, the access entry,
//! their encoding with the per-entry size cap, and the replay check. Every
//! line carries `"v": 1`. See `docs/AGENT_JOURNALS.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The line format this build writes and reads.
pub const JOURNAL_VERSION: u64 = 1;

/// The most bytes one line may take, newline included. A longer entry keeps
/// its summary and drops its operands (`truncated`, not `replayable`).
pub const MAX_ENTRY_BYTES: usize = 64 * 1024;

/// No entry is shorter than this, so a journal of `n` bytes holds at most
/// `n / MIN_LINE_BYTES` lines; the line count is only taken above that.
pub const MIN_LINE_BYTES: usize = 64;

/// The longest path, reason or client name an entry keeps, in characters.
const FIELD_CHARS: usize = 1024;

/// The most rights an access line lists.
const MAX_CAPABILITIES: usize = 64;

/// An agent command that ran, with what a replay needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditEntry {
    pub v: u64,
    /// When the command ran, UTC, to the millisecond.
    pub ts: String,
    /// The session (one run of the app, or of a headless run) the revisions
    /// and the undo group belong to: an engine's revision counter starts again
    /// in every process, so a revision means something only within its
    /// session. A line without one (an older build) is never replayable.
    #[serde(default)]
    pub session: String,
    pub pid: Option<u32>,
    pub client: Option<String>,
    /// The document's persistent id; `None` when the command names none.
    pub document: Option<String>,
    /// The document's path at the time of the event.
    pub path: Option<String>,
    pub command: String,
    /// The operands as the wire carries them, by name.
    pub operands: BTreeMap<String, String>,
    pub revision_before: Option<u64>,
    pub revision_after: Option<u64>,
    /// The undo group the command made, when it made one.
    pub group: Option<u64>,
    /// `ran`, `refused` or `failed`.
    pub outcome: String,
    pub reason: Option<String>,
    pub truncated: bool,
    pub replayable: bool,
}

/// A connection or a grant of the agent channel, or a headless run's start
/// and end. Read queries are not entries: this record covers them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessEntry {
    pub v: u64,
    pub ts: String,
    /// `accepted`, `refused`, `closed`, `grants`, `started` or `ended`.
    pub event: String,
    /// `socket` (the running app) or `headless`.
    pub mode: String,
    pub pid: Option<u32>,
    pub client: Option<String>,
    pub reason: Option<String>,
    /// What the client may do: capabilities (`read`, `edit`, `save`, ...) and
    /// `allow-path` roots, as it reported or as the run was given them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    /// Some of the text was shortened or dropped to keep the line within
    /// [`MAX_ENTRY_BYTES`].
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
}

fn is_false(flag: &bool) -> bool {
    !*flag
}

/// A command id and its `name`, `text` operands, as a request sends them.
pub type ReplayRequest<'a> = (&'a str, Vec<(String, String)>);

impl EditEntry {
    /// Does the entry carry what a replay needs: it ran, it knows its session
    /// and revision, its operands were not dropped, and it is not a `revert`
    /// (which names an undo group that exists only in its session)?
    fn replay_ready(&self) -> bool {
        self.outcome == "ran"
            && self.revision_before.is_some()
            && !self.session.is_empty()
            && !self.truncated
            && self.command != "revert"
    }

    /// The command and operands to send again, if the entry is of `session`
    /// (the one that is running now) and the document is still at the
    /// revision the entry ran at. A replay from another session is refused,
    /// whatever the revision says, and so is one against another revision, as
    /// `expect_revision` refuses (#2178). Nothing about the document's content
    /// is compared (a later change).
    pub fn replay_request(
        &self,
        session: &str,
        current_revision: u64,
    ) -> Result<ReplayRequest<'_>, String> {
        if !self.replayable || !self.replay_ready() {
            return Err(format!(
                "the entry for `{}` at {} cannot be replayed (it did not run, it has no session, it is a revert, or its operands were dropped)",
                self.command, self.ts
            ));
        }
        if self.session != session {
            return Err(format!(
                "the entry for `{}` at {} was recorded in another session; its revision means nothing in this one. Nothing was applied",
                self.command, self.ts
            ));
        }
        let before = self.revision_before.unwrap_or_default();
        if current_revision != before {
            return Err(format!(
                "the document is at revision {current_revision}, the entry ran at {before}; read it again. Nothing was applied"
            ));
        }
        let operands = self
            .operands
            .iter()
            .map(|(name, text)| (name.clone(), text.clone()))
            .collect();
        Ok((self.command.as_str(), operands))
    }

    /// Fix the version, the derived flag and the length of the free fields.
    fn settle(&mut self) {
        self.v = JOURNAL_VERSION;
        self.path = self.path.take().map(|text| shorten(&text));
        self.client = self.client.take().map(|text| shorten(&text));
        self.reason = self.reason.take().map(|text| shorten(&text));
        self.replayable = self.replay_ready();
    }

    /// The line, without its newline: within [`MAX_ENTRY_BYTES`] including
    /// the newline, by dropping the operands (but not `summary`) when it
    /// would be longer.
    pub fn to_line(mut self) -> String {
        self.settle();
        let full = encode(&self);
        if full.len() < MAX_ENTRY_BYTES {
            return full;
        }
        self.operands.retain(|name, _| name == "summary");
        self.truncated = true;
        self.replayable = false;
        for text in self.operands.values_mut() {
            *text = shorten(text);
        }
        encode(&self)
    }
}

impl AccessEntry {
    /// An event of `mode` at `ts`; the rest is added with the `with_` methods.
    pub fn new(ts: String, event: &str, mode: &str) -> Self {
        Self {
            v: JOURNAL_VERSION,
            ts,
            event: event.to_string(),
            mode: mode.to_string(),
            pid: None,
            client: None,
            reason: None,
            capabilities: Vec::new(),
            truncated: false,
        }
    }

    #[must_use]
    pub fn with_peer(mut self, pid: Option<u32>, client: Option<String>) -> Self {
        self.pid = pid;
        self.client = client;
        self
    }

    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    #[must_use]
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// The line, without its newline: within [`MAX_ENTRY_BYTES`] including the
    /// newline, as an edit line is. A long list of rights is cut to its first
    /// entries, each cut to a bounded length, and dropped altogether if the
    /// line is still too long; `truncated` says so.
    pub fn to_line(mut self) -> String {
        self.v = JOURNAL_VERSION;
        self.client = self.client.take().map(|text| shorten(&text));
        self.reason = self.reason.take().map(|text| shorten(&text));
        let before = self.capabilities.clone();
        self.capabilities.truncate(MAX_CAPABILITIES);
        for capability in &mut self.capabilities {
            *capability = shorten(capability);
        }
        self.truncated = self.capabilities != before;
        let line = encode(&self);
        if line.len() < MAX_ENTRY_BYTES {
            return line;
        }
        self.capabilities.clear();
        self.truncated = true;
        encode(&self)
    }
}

fn shorten(text: &str) -> String {
    text.chars().take(FIELD_CHARS).collect()
}

fn encode<T: Serialize>(entry: &T) -> String {
    // A struct of strings, numbers and string maps always encodes.
    serde_json::to_string(entry).unwrap_or_else(|_| String::from("{}"))
}
