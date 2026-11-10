//! At the line cap the newest lines stay, by a rename, so a reader sees the old
//! file or the new one; an append racing a rotation loses no line.

use super::{Journal, edit, journal_path, read_edits};
use crate::app::agent::journal::{MIN_LINE_BYTES, append_line};

const CAP: u32 = 100;

fn put(dir: &std::path::Path, revision: u64, cap: u32) {
    append_line(
        dir,
        Journal::Edits,
        &edit("color_net", revision).to_line(),
        cap,
    )
    .unwrap();
}

#[test]
fn the_newest_lines_stay_and_the_oldest_go() {
    let dir = tempfile::tempdir().unwrap();
    for revision in 0..(u64::from(CAP) + 40) {
        put(dir.path(), revision, CAP);
        // Never more than the cap, and never fewer than it holds so far.
        let kept = read_edits(dir.path()).entries.len() as u64;
        assert_eq!(kept, (revision + 1).min(u64::from(CAP)), "after {revision}");
    }

    let report = read_edits(dir.path());

    assert_eq!(report.entries.len(), CAP as usize);
    assert_eq!(report.entries[0].revision_before, Some(40));
    assert_eq!(
        report.entries.last().and_then(|e| e.revision_before),
        Some(u64::from(CAP) + 39)
    );
    assert!(!report.partial_tail && report.damaged == 0);
}

#[test]
fn a_rotation_leaves_no_temp_file_and_keeps_the_mode() {
    let dir = tempfile::tempdir().unwrap();
    for revision in 0..(u64::from(CAP) + 5) {
        put(dir.path(), revision, CAP);
    }
    let names: Vec<String> = std::fs::read_dir(dir.path().join(".signex"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().all(|name| !name.ends_with(".tmp")),
        "{names:?}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(journal_path(dir.path(), Journal::Edits))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn a_journal_at_or_under_the_cap_is_not_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    for revision in 0..u64::from(CAP) {
        put(dir.path(), revision, CAP);
    }
    assert_eq!(read_edits(dir.path()).entries.len(), CAP as usize);
}

/// A line shorter than the floor would let a journal hide more lines than the
/// cap from the size check.
#[test]
fn the_smallest_entries_are_longer_than_the_size_floor() {
    let mut bare = edit("x", 0);
    bare.document = None;
    bare.path = None;
    bare.client = None;
    bare.pid = None;
    bare.operands.clear();
    bare.group = None;
    bare.revision_before = None;
    bare.revision_after = None;
    assert!(bare.to_line().len() + 1 >= MIN_LINE_BYTES);
    let access = super::access("closed").to_line();
    assert!(access.len() + 1 >= MIN_LINE_BYTES);
}

/// Rotation holds the same lock as an append: with writers racing across the
/// cap, every line of the last stretch is there, once and whole.
#[test]
fn appends_racing_a_rotation_lose_no_line() {
    let dir = tempfile::tempdir().unwrap();
    for revision in 0..u64::from(CAP) {
        put(dir.path(), revision, CAP);
    }
    let per_writer = 60_u64;
    std::thread::scope(|scope| {
        for writer in 1..=4_u64 {
            let path = dir.path();
            scope.spawn(move || {
                for n in 0..per_writer {
                    put(path, writer * 10_000 + n, CAP);
                }
            });
        }
    });

    let report = read_edits(dir.path());

    assert_eq!(report.entries.len(), CAP as usize);
    assert_eq!(report.damaged, 0);
    let mut seen = std::collections::BTreeSet::new();
    for entry in &report.entries {
        assert!(seen.insert(entry.revision_before), "a line twice");
    }
    // Each writer's own lines stay in its order, and the newest are kept:
    // the last line of each writer survived.
    for writer in 1..=4_u64 {
        let mine: Vec<u64> = report
            .entries
            .iter()
            .filter_map(|e| e.revision_before)
            .filter(|r| r / 10_000 == writer)
            .collect();
        assert!(
            mine.windows(2).all(|w| w[0] < w[1]),
            "writer {writer} order"
        );
        assert_eq!(mine.last().copied(), Some(writer * 10_000 + per_writer - 1));
    }
}
