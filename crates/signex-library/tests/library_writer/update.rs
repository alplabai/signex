//! `LibraryWriter::write_update`: overwrite a converter's earlier output
//! (contract §5).

use chrono::{TimeZone, Utc};

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::library_file::LibraryFile;
use signex_library::lifecycle::LifecycleState;
use signex_library::writer::LibraryWriter;

use crate::read_tree;
use crate::synthetic::{self, row_id};

fn row_of(adapter: &LocalGitAdapter, n: usize) -> Option<signex_library::component::ComponentRow> {
    adapter
        .iter_rows()
        .unwrap()
        .into_iter()
        .map(|(_, r)| r)
        .find(|r| r.row_id == row_id(n))
}

#[test]
fn an_update_with_the_same_input_changes_no_byte() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    synthetic::library(120).write(&root).unwrap();
    let before = read_tree(&root);
    let summary = synthetic::library(120).write_update(&root).unwrap();
    assert_eq!(read_tree(&root), before);
    assert_eq!(summary.rows_unchanged, 120);
    assert_eq!(summary.files_written + summary.files_deleted, 0);
}

#[test]
fn an_update_bumps_changed_rows_keeps_state_and_drops_removed_items() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    let snxlib = synthetic::library(100).write(&root).unwrap();
    // The library's own edit of a lifecycle state, which an update keeps.
    let text = std::fs::read_to_string(&snxlib).unwrap();
    let id = row_id(7).to_string();
    let edited: String = text
        .lines()
        .map(|l| {
            if l.starts_with(&id) {
                l.replacen("\tReleased\t", "\tObsolete\t", 1)
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(&snxlib, &edited).unwrap();
    let stored_r3 = text
        .lines()
        .find(|l| l.starts_with(&row_id(3).to_string()))
        .unwrap()
        .to_string();

    // Re-convert: 90 rows and 9 footprints (10 rows and footprint 9 gone),
    // row 7 changed upstream.
    let mut w = LibraryWriter::new("Synthetic", synthetic::manifest("Synthetic"));
    let later = Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap();
    w.set_update_time(later);
    for n in 0..90 {
        let mut r = synthetic::row(n, 2, 10);
        if n == 7 {
            r.description = "Changed upstream".into();
        }
        w.add_row(synthetic::table_of(n), r);
    }
    w.add_symbol_file(synthetic::envelope(
        0,
        (0..2).map(synthetic::symbol).collect(),
    ));
    for n in 0..9 {
        w.add_footprint_file(synthetic::footprint(n)).unwrap();
    }
    let summary = w.write_update(&root).unwrap();
    assert_eq!(summary.rows_removed, 10);
    assert_eq!(summary.rows_changed, 1);
    assert_eq!(summary.rows_unchanged, 89);
    assert_eq!(summary.files_deleted, 1, "footprint 9 is gone");

    let adapter = LocalGitAdapter::open(&snxlib).unwrap();
    assert!(row_of(&adapter, 95).is_none());
    let r7 = row_of(&adapter, 7).unwrap();
    assert_eq!(r7.description, "Changed upstream");
    assert_eq!(r7.state, LifecycleState::Obsolete, "lifecycle state kept");
    assert_eq!(r7.version, "1.0.1");
    assert_eq!(r7.created, synthetic::time());
    assert_eq!(r7.updated, later);
    assert_eq!(
        r7.content_hash,
        signex_library::hash_row_content(&r7).unwrap()
    );
    let after = std::fs::read_to_string(&snxlib).unwrap();
    assert!(
        after.contains(&format!("\n{stored_r3}\n")),
        "an unchanged row keeps its bytes"
    );
    assert!(
        adapter
            .get_footprint(synthetic::footprint(9).footprints[0].uuid)
            .is_err()
    );
    LibraryFile::parse(&after).unwrap();
}

#[test]
fn an_update_of_another_library_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    synthetic::library(10).write(&root).unwrap();
    let before = read_tree(&root);
    let mut other = synthetic::manifest("Synthetic");
    other.library_id = uuid::Uuid::from_u128(42);
    assert!(
        LibraryWriter::new("Synthetic", other)
            .write_update(&root)
            .is_err()
    );
    assert_eq!(read_tree(&root), before);
}
