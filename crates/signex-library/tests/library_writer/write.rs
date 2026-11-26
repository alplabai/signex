//! `LibraryWriter::write`: a new library directory in one pass.

use std::time::Instant;

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::Footprint;
use signex_library::writer::LibraryWriter;

use crate::read_tree;
use crate::synthetic::{self, LIBRARY_ID};

#[test]
fn the_written_library_opens_and_every_reference_resolves() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    let snxlib = synthetic::library(200).write(&root).unwrap();
    let adapter = LocalGitAdapter::open(&snxlib).unwrap();
    assert_eq!(adapter.library_id(), LIBRARY_ID);
    let mut seen = 0;
    for (_, row) in adapter.iter_rows().unwrap() {
        seen += 1;
        adapter.get_symbol(row.symbol_ref.uuid).unwrap();
        let fp = adapter
            .get_footprint(row.footprint_ref.unwrap().uuid)
            .unwrap();
        assert_eq!(Some(fp.uuid), row.footprint_ref.map(|r| r.uuid));
        assert_eq!(
            row.content_hash,
            signex_library::hash_row_content(&row).unwrap()
        );
    }
    assert_eq!(seen, 200);
    assert!(!root.join(".git").exists(), "the writer runs no git");
}

#[test]
fn writing_twice_gives_identical_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a"), dir.path().join("b"));
    synthetic::library(300).write(&a).unwrap();
    synthetic::library(300).write(&b).unwrap();
    let (a, b) = (read_tree(&a), read_tree(&b));
    assert!(a.len() > 30);
    assert_eq!(a, b);
}

#[test]
fn a_non_empty_target_is_refused_and_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("keep.txt"), b"mine").unwrap();
    assert!(synthetic::library(10).write(&root).is_err());
    assert_eq!(read_tree(&root).len(), 1);
    let leftovers: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(leftovers.len(), 1, "no staging folder is left behind");
}

#[test]
fn an_empty_target_folder_is_filled() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    std::fs::create_dir(&root).unwrap();
    synthetic::library(10).write(&root).unwrap();
    assert!(root.join("Synthetic.snxlib").is_file());
}

#[test]
fn a_footprint_envelope_with_two_footprints_is_refused() {
    let mut file = synthetic::footprint(1);
    file.footprints.push(Footprint::empty("second"));
    let mut w = LibraryWriter::new("X", synthetic::manifest("X"));
    assert!(w.add_footprint_file(file).is_err());
}

#[test]
fn duplicate_part_numbers_and_row_ids_are_refused_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let mut w = synthetic::library(10);
    let mut dup = synthetic::row(11, 1, 1);
    dup.internal_pn = synthetic::row(3, 1, 1).internal_pn;
    w.add_row("resistors", dup);
    assert!(w.write(&dir.path().join("a")).is_err());
    assert!(!dir.path().join("a").exists());

    let mut w = synthetic::library(10);
    let mut dup = synthetic::row(3, 1, 1);
    dup.internal_pn = signex_library::identity::InternalPn::new("OTHER");
    w.add_row("ics", dup);
    assert!(w.write(&dir.path().join("b")).is_err());
}

#[test]
fn symbol_envelopes_are_named_like_the_editor_names_them() {
    let mut w = LibraryWriter::new("X", synthetic::manifest("X"));
    let file = |uuid: u128, name: &str| signex_library::primitive::SymbolFile {
        format: String::new(),
        file_uuid: uuid::Uuid::from_u128(uuid),
        display_name: name.into(),
        copied_from: None,
        symbols: Vec::new(),
        created: synthetic::time(),
        updated: synthetic::time(),
    };
    assert_eq!(
        w.add_symbol_file(file(1, "Discrete Passives")),
        "discrete-passives.snxsym"
    );
    assert_eq!(
        w.add_symbol_file(file(2, "Discrete  passives!")),
        "00000000-0000-0000-0000-000000000002.snxsym"
    );
}

/// Done-when of #3110: 25 000 rows in under 60 s on one pool core. Run with
/// `cargo test -p signex-library --release --test library_writer -- --ignored --nocapture`.
#[test]
#[ignore = "timing run, see the doc comment"]
fn writes_25000_rows_in_under_a_minute() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let writer = synthetic::library(25_000);
    let built = started.elapsed();
    let snxlib = writer.write(&dir.path().join("Synthetic")).unwrap();
    let total = started.elapsed();
    let files = read_tree(&dir.path().join("Synthetic")).len();
    eprintln!(
        "25000 rows, {files} files: built {built:?}, written {:?}",
        total - built
    );
    assert!(total.as_secs() < 60, "took {total:?}");
    let adapter = LocalGitAdapter::open(snxlib).unwrap();
    assert_eq!(adapter.iter_rows().unwrap().into_iter().count(), 25_000);
}
