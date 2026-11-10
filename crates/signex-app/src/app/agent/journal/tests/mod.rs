//! #2253: the journal writer and reader, on real folders.

mod append;
mod concurrent;
mod entries;
mod refusals;
mod rotation;
mod seam;
mod summary;

use std::collections::BTreeMap;
use std::path::Path;

use super::{AccessEntry, EditEntry};
use super::{JOURNAL_VERSION, Journal, read_journal};

/// An edit entry whose line is long enough to be a real one.
pub(super) fn edit(command: &str, revision: u64) -> EditEntry {
    EditEntry {
        v: JOURNAL_VERSION,
        ts: "2026-11-10T12:00:00.000Z".into(),
        session: SESSION.into(),
        pid: Some(4242),
        client: Some("signex-cli".into()),
        document: Some("0193a2f0-7b1c-7cde-8123-456789abcdef".into()),
        path: Some("/work/P/b.snxsch".into()),
        command: command.into(),
        operands: BTreeMap::from([("at".to_string(), "5,0".to_string())]),
        revision_before: Some(revision),
        revision_after: Some(revision + 1),
        group: Some(revision),
        outcome: "ran".into(),
        reason: None,
        truncated: false,
        replayable: true,
    }
}

pub(super) fn access(event: &str) -> AccessEntry {
    AccessEntry::new("2026-11-10T12:00:00.000Z".into(), event, "socket")
}

pub(super) fn journal_path(dir: &Path, journal: Journal) -> std::path::PathBuf {
    dir.join(".signex").join(journal.file_name())
}

pub(super) fn read_edits(dir: &Path) -> super::Report<EditEntry> {
    read_journal(&journal_path(dir, Journal::Edits)).unwrap()
}

pub(super) const CAP: u32 = 10_000;

/// The session of the entries `edit` makes.
pub(super) const SESSION: &str = "0193a2f0-aaaa-7bbb-8ccc-000000000001";
