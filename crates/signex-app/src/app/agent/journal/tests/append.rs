//! A line is appended whole, in order, and a crash's partial line is never
//! fused with the next one.

use std::io::Write as _;

use super::{CAP, Journal, access, edit, journal_path, read_edits};
use crate::app::agent::journal::{AccessEntry, append_line, read_journal};

#[test]
fn lines_are_appended_in_order_and_read_back_whole() {
    let dir = tempfile::tempdir().unwrap();
    for revision in 0..5 {
        append_line(
            dir.path(),
            Journal::Edits,
            &edit("color_net", revision).to_line(),
            CAP,
        )
        .unwrap();
    }

    let report = read_edits(dir.path());

    let revisions: Vec<_> = report.entries.iter().map(|e| e.revision_before).collect();
    assert_eq!(revisions, (0..5).map(Some).collect::<Vec<_>>());
    assert!(!report.partial_tail && report.newer == 0 && report.damaged == 0);
    assert!(report.entries.iter().all(|e| e.v == 1 && e.replayable));
}

#[test]
fn the_two_journals_are_separate_files_under_one_lock() {
    let dir = tempfile::tempdir().unwrap();
    append_line(
        dir.path(),
        Journal::Edits,
        &edit("color_net", 1).to_line(),
        CAP,
    )
    .unwrap();
    append_line(
        dir.path(),
        Journal::Access,
        &access("accepted").to_line(),
        CAP,
    )
    .unwrap();

    let access: crate::app::agent::journal::Report<AccessEntry> =
        read_journal(&journal_path(dir.path(), Journal::Access)).unwrap();
    assert_eq!(access.entries.len(), 1);
    assert_eq!(read_edits(dir.path()).entries.len(), 1);
    assert!(dir.path().join(".signex/agent-journal.lock").is_file());
}

/// A crash mid-append leaves a last line with no newline: the reader skips it
/// and says so, and the next append starts a line of its own.
#[test]
fn a_crash_truncated_last_line_is_skipped_reported_and_not_fused_with_the_next() {
    let dir = tempfile::tempdir().unwrap();
    append_line(
        dir.path(),
        Journal::Edits,
        &edit("color_net", 1).to_line(),
        CAP,
    )
    .unwrap();
    let path = journal_path(dir.path(), Journal::Edits);
    let whole = edit("color_net", 2).to_line();
    let cut = &whole[..whole.len() / 2];
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(cut.as_bytes())
        .unwrap();

    let before = read_edits(dir.path());
    assert!(before.partial_tail, "the cut line is reported");
    assert_eq!(before.entries.len(), 1, "and skipped");
    assert!(
        before.problems(&path)[0].contains("partial line"),
        "{:?}",
        before.problems(&path)
    );

    append_line(
        dir.path(),
        Journal::Edits,
        &edit("color_net", 3).to_line(),
        CAP,
    )
    .unwrap();

    let after = read_edits(dir.path());
    let revisions: Vec<_> = after.entries.iter().map(|e| e.revision_before).collect();
    assert_eq!(revisions, [Some(1), Some(3)], "the new line is whole");
    assert!(!after.partial_tail);
    assert_eq!(
        after.damaged, 1,
        "the cut piece is one damaged line, skipped"
    );
}

#[test]
fn a_line_of_a_newer_version_is_skipped_reported_and_left_in_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = journal_path(dir.path(), Journal::Edits);
    append_line(dir.path(), Journal::Edits, &edit("a", 1).to_line(), CAP).unwrap();
    let newer = r#"{"v":2,"ts":"2027-01-01T00:00:00.000Z","something":"new"}"#;
    append_line(dir.path(), Journal::Edits, newer, CAP).unwrap();
    append_line(dir.path(), Journal::Edits, &edit("b", 2).to_line(), CAP).unwrap();

    let report = read_edits(dir.path());

    assert_eq!(report.entries.len(), 2);
    assert_eq!(report.newer, 1);
    assert!(report.problems(&path)[0].contains("newer Signex"));
    assert!(std::fs::read_to_string(&path).unwrap().contains(newer));
}

#[test]
fn a_missing_journal_reads_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    let report = read_edits(dir.path());
    assert!(report.entries.is_empty() && !report.partial_tail);
}

#[cfg(unix)]
#[test]
fn the_files_are_created_private() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    append_line(dir.path(), Journal::Edits, &edit("a", 1).to_line(), CAP).unwrap();
    for name in ["agent-edits.jsonl", "agent-journal.lock"] {
        let mode = std::fs::metadata(dir.path().join(".signex").join(name))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "{name}");
    }
}
