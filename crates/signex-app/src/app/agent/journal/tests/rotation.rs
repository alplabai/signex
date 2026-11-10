//! At the line cap the newest lines stay (down to 90 % of the cap), by a
//! streamed copy and a rename, so a reader sees the old file or the new one;
//! an append racing a rotation loses no line.

use super::{Journal, edit, journal_path, read_edits};
use crate::app::agent::journal::{MIN_LINE_BYTES, append_line};

const CAP: u32 = 100;
/// What a rotation of `CAP` leaves.
const KEPT: u64 = 90;

fn put(dir: &std::path::Path, revision: u64, cap: u32) {
    append_line(
        dir,
        Journal::Edits,
        &edit("color_net", revision).to_line(),
        cap,
    )
    .unwrap();
}

fn revisions(dir: &std::path::Path) -> Vec<u64> {
    read_edits(dir)
        .entries
        .iter()
        .filter_map(|e| e.revision_before)
        .collect()
}

#[test]
fn rotating_down_to_ninety_percent_keeps_the_newest_lines_in_order() {
    let dir = tempfile::tempdir().unwrap();
    for revision in 0..(u64::from(CAP) + 40) {
        put(dir.path(), revision, CAP);
        let kept = revisions(dir.path());
        // Never more than the cap; the journal grows to it, is cut to 90 % when
        // it passes it, and never loses the line just written.
        assert!(kept.len() as u64 <= u64::from(CAP), "after {revision}");
        assert_eq!(kept.last(), Some(&revision), "after {revision}");
        if revision < u64::from(CAP) {
            assert_eq!(kept.len() as u64, revision + 1);
        }
        if revision == u64::from(CAP) {
            assert_eq!(kept.len() as u64, KEPT, "cut to 90 % at the first excess");
            assert_eq!(kept[0], u64::from(CAP) + 1 - KEPT, "the oldest went");
        }
    }

    let kept = revisions(dir.path());
    assert!(
        kept.windows(2).all(|w| w[1] == w[0] + 1),
        "no gap: {kept:?}"
    );
    let report = read_edits(dir.path());
    assert!(!report.partial_tail && report.damaged == 0);
}

/// A journal that has just been rotated is not rewritten by the next append:
/// it is rewritten again only once it is past the cap again.
#[cfg(unix)]
#[test]
fn a_rotated_journal_is_not_rewritten_until_it_is_past_the_cap_again() {
    use std::os::unix::fs::MetadataExt as _;
    let dir = tempfile::tempdir().unwrap();
    let file = journal_path(dir.path(), Journal::Edits);
    for revision in 0..=u64::from(CAP) {
        put(dir.path(), revision, CAP);
    }
    let rotated = std::fs::metadata(&file).unwrap().ino();

    // Ten more lines refill it to the cap: no rewrite on the way.
    for revision in 0..10 {
        put(dir.path(), 1_000 + revision, CAP);
        assert_eq!(std::fs::metadata(&file).unwrap().ino(), rotated);
    }
    assert_eq!(revisions(dir.path()).len() as u64, u64::from(CAP));

    // The next one is past the cap: rewritten, down to 90 %.
    put(dir.path(), 2_000, CAP);
    assert_ne!(std::fs::metadata(&file).unwrap().ino(), rotated);
    assert_eq!(revisions(dir.path()).len() as u64, KEPT);
}

/// The copy streams in chunks: a journal many times the chunk size rotates to
/// exactly its newest lines, none cut in two at a chunk boundary.
#[test]
fn a_journal_far_larger_than_one_chunk_rotates_to_exactly_its_newest_lines() {
    let dir = tempfile::tempdir().unwrap();
    let cap = 1_000_u32;
    let mut text = String::new();
    for revision in 0..3_000_u64 {
        let mut entry = edit("color_net", revision);
        entry.operands.insert("note".into(), "n".repeat(300));
        text.push_str(&entry.to_line());
        text.push('\n');
    }
    assert!(text.len() > 10 * 64 * 1024, "{} bytes", text.len());
    std::fs::create_dir(dir.path().join(".signex")).unwrap();
    std::fs::write(journal_path(dir.path(), Journal::Edits), text).unwrap();

    put(dir.path(), 9_999, cap);

    let kept = revisions(dir.path());
    assert_eq!(kept.len(), 900);
    assert_eq!(kept.last(), Some(&9_999));
    assert_eq!(kept[0], 3_000 + 1 - 900, "the 900 newest of 3001 lines");
    assert!(kept[..899].windows(2).all(|w| w[1] == w[0] + 1));
    assert_eq!(read_edits(dir.path()).damaged, 0);
}

/// There is no size beyond which a rotation gives up and the journal grows.
#[test]
fn a_journal_is_rotated_whatever_its_size() {
    let dir = tempfile::tempdir().unwrap();
    let mut text = String::new();
    for revision in 0..400_u64 {
        let mut entry = edit("color_net", revision);
        entry.operands.insert("note".into(), "n".repeat(20_000));
        text.push_str(&entry.to_line());
        text.push('\n');
    }
    std::fs::create_dir(dir.path().join(".signex")).unwrap();
    std::fs::write(journal_path(dir.path(), Journal::Edits), text).unwrap();

    put(dir.path(), 9_999, 200);

    assert_eq!(revisions(dir.path()).len(), 180);
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
/// cap, every line is there at most once and whole, each writer's own lines in
/// its order, and the last line of each survives.
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

    let kept = report.entries.len() as u64;
    assert!((KEPT..=u64::from(CAP)).contains(&kept), "{kept}");
    assert_eq!(report.damaged, 0);
    let mut seen = std::collections::BTreeSet::new();
    for entry in &report.entries {
        assert!(seen.insert(entry.revision_before), "a line twice");
    }
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
