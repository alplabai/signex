//! `insert_row` refuses a duplicate `row_id` or `internal_pn` and a bad
//! table name instead of storing a second copy (#1363).

use super::write_oracle_tests::{Rng, World, random_row, seeded};
use super::*;
use crate::identity::InternalPn;

#[test]
fn insert_row_refuses_duplicates_and_bad_table_names() {
    let mut rng = Rng(3);
    let w = World {
        lib: rng.uuid(),
        symbols: (0..3).map(|_| rng.uuid()).collect(),
        footprints: (0..3).map(|_| rng.uuid()).collect(),
    };
    let dir = tempfile::tempdir().unwrap();
    let file = seeded(dir.path(), &mut rng, &w, 0, false);
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let mut first = {
        let id = rng.uuid();
        random_row(&mut rng, &w, id)
    };
    first.internal_pn = InternalPn::new("PN-1");
    adapter.insert_row("t", first.clone(), "a").unwrap();

    // Same row id, different part number.
    let mut same_id = random_row(&mut rng, &w, first.row_id);
    same_id.internal_pn = InternalPn::new("PN-2");
    assert!(matches!(
        adapter.insert_row("t", same_id, "b"),
        Err(LibraryError::Conflict(_))
    ));
    // Same part number, new row id, even in another table.
    let mut same_pn = {
        let id = rng.uuid();
        random_row(&mut rng, &w, id)
    };
    same_pn.internal_pn = InternalPn::new("PN-1");
    assert!(matches!(
        adapter.insert_row("other", same_pn, "c"),
        Err(LibraryError::Conflict(_))
    ));
    // Bad table names.
    let mut fresh = {
        let id = rng.uuid();
        random_row(&mut rng, &w, id)
    };
    fresh.internal_pn = InternalPn::new("PN-3");
    for bad in ["", "  ", "a/b", "a.b"] {
        assert!(
            adapter.insert_row(bad, fresh.clone(), "d").is_err(),
            "{bad:?}"
        );
    }
    // Nothing leaked in.
    assert_eq!(adapter.read_table("t").unwrap().len(), 1);
    assert!(
        !adapter
            .list_tables()
            .unwrap()
            .contains(&"other".to_string())
    );
}

fn world_and_adapter(seed: u64) -> (Rng, World, tempfile::TempDir, LocalGitAdapter) {
    let mut rng = Rng(seed);
    let w = World {
        lib: rng.uuid(),
        symbols: (0..3).map(|_| rng.uuid()).collect(),
        footprints: (0..3).map(|_| rng.uuid()).collect(),
    };
    let dir = tempfile::tempdir().unwrap();
    let file = seeded(dir.path(), &mut rng, &w, 0, false);
    let adapter = LocalGitAdapter::open(&file).unwrap();
    (rng, w, dir, adapter)
}

#[test]
fn blank_part_numbers_never_conflict() {
    let (mut rng, w, _dir, adapter) = world_and_adapter(5);
    for pn in ["", "", "  ", "  "] {
        let id = rng.uuid();
        let mut row = random_row(&mut rng, &w, id);
        row.internal_pn = InternalPn::new(pn);
        adapter.insert_row("t", row, "draft").unwrap();
    }
    assert_eq!(adapter.read_table("t").unwrap().len(), 4);
}

#[test]
fn update_row_refuses_another_rows_part_number() {
    let (mut rng, w, _dir, adapter) = world_and_adapter(6);
    let mut a = {
        let id = rng.uuid();
        random_row(&mut rng, &w, id)
    };
    a.internal_pn = InternalPn::new("PN-A");
    let mut b = {
        let id = rng.uuid();
        random_row(&mut rng, &w, id)
    };
    b.internal_pn = InternalPn::new("PN-B");
    adapter.insert_row("t", a.clone(), "a").unwrap();
    adapter.insert_row("t", b.clone(), "b").unwrap();

    // B takes A's PN: refused. Keeping its own PN is fine.
    let mut clash = b.clone();
    clash.internal_pn = InternalPn::new("PN-A");
    assert!(matches!(
        adapter.update_row("t", clash.clone(), "c"),
        Err(LibraryError::Conflict(_))
    ));
    assert!(matches!(
        adapter.update_row_with_assets("t", clash, "c", &[]),
        Err(LibraryError::Conflict(_))
    ));
    adapter.update_row("t", b, "same").unwrap();
    // Blanking a PN is allowed (draft).
    let mut blank = a;
    blank.internal_pn = InternalPn::new("");
    adapter.update_row("t", blank, "blank").unwrap();
}

#[test]
fn existing_table_with_refused_name_still_takes_rows() {
    let (mut rng, w, _dir, adapter) = world_and_adapter(7);
    let seed = {
        let id = rng.uuid();
        let mut r = random_row(&mut rng, &w, id);
        r.internal_pn = InternalPn::new("PN-SEED");
        r
    };
    adapter.insert_row("t", seed, "seed").unwrap();
    // Hand-edited file: a table whose name the create path now refuses.
    adapter
        .mutate_library_file(
            |lf| {
                let t = lf.tables["t"].clone();
                lf.tables.insert("a.b".to_string(), t);
                Ok(())
            },
            "legacy",
            "legacy",
        )
        .unwrap();
    let mut row = {
        let id = rng.uuid();
        random_row(&mut rng, &w, id)
    };
    row.internal_pn = InternalPn::new("PN-LEGACY");
    adapter.insert_row("a.b", row, "x").unwrap();
}

#[test]
fn history_subject_drops_a_trailing_carriage_return() {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let sig = git2::Signature::now("t", "t@example.com").unwrap();
    let tree = repo
        .find_tree(repo.treebuilder(None).unwrap().write().unwrap())
        .unwrap();
    let id = repo
        .commit(None, &sig, &sig, "x\r\n\r\nbody\r\n", &tree, &[])
        .unwrap();
    let entry = HistoryEntry::from_commit(&repo.find_commit(id).unwrap());
    assert_eq!(entry.subject, "x");
}

#[test]
fn legacy_duplicate_part_numbers_stay_editable_but_cannot_be_copied() {
    let (mut rng, w, _dir, adapter) = world_and_adapter(8);
    let mut rows = Vec::new();
    for pn in ["DUP", "PN-C"] {
        let id = rng.uuid();
        let mut r = random_row(&mut rng, &w, id);
        r.internal_pn = InternalPn::new(pn);
        adapter.insert_row("t", r.clone(), "seed").unwrap();
        rows.push(r);
    }
    // A second row with the same PN, written past `insert_row`.
    let dup_id = rng.uuid();
    adapter
        .mutate_library_file(
            |lf| {
                let table = lf.tables.get_mut("t").unwrap();
                let mut copy = table.rows[0].clone();
                Arc::make_mut(&mut copy)
                    .cells
                    .insert(LEGACY_ROW_ID_COL.into(), dup_id.to_string());
                table.rows.push(copy);
                Ok(())
            },
            "legacy",
            "legacy",
        )
        .unwrap();

    // Editing another field of a duplicated row works.
    let mut edit = adapter.read_row("t", RowId::from_uuid(dup_id)).unwrap();
    edit.description = "edited".into();
    adapter.update_row("t", edit, "edit").unwrap();
    // Moving a third row onto the duplicated PN is still refused.
    let mut clash = adapter
        .read_row("t", RowId::from_uuid(rows[1].row_id))
        .unwrap();
    clash.internal_pn = InternalPn::new("DUP");
    assert!(matches!(
        adapter.update_row("t", clash, "clash"),
        Err(LibraryError::Conflict(_))
    ));
}

#[test]
fn part_numbers_differing_only_in_whitespace_are_duplicates() {
    let (mut rng, w, _dir, adapter) = world_and_adapter(9);
    let mut rows = Vec::new();
    for pn in ["PN-A", "PN-B"] {
        let id = rng.uuid();
        let mut r = random_row(&mut rng, &w, id);
        r.internal_pn = InternalPn::new(pn);
        adapter.insert_row("t", r.clone(), "seed").unwrap();
        rows.push(r);
    }
    // Insert a padded copy of an existing PN.
    let mut padded = {
        let id = rng.uuid();
        random_row(&mut rng, &w, id)
    };
    padded.internal_pn = InternalPn::new("PN-A ");
    assert!(matches!(
        adapter.insert_row("t", padded, "pad"),
        Err(LibraryError::Conflict(_))
    ));
    // Move another row onto a padded copy.
    let mut clash = adapter
        .read_row("t", RowId::from_uuid(rows[1].row_id))
        .unwrap();
    clash.internal_pn = InternalPn::new(" PN-A\t");
    assert!(matches!(
        adapter.update_row("t", clash, "clash"),
        Err(LibraryError::Conflict(_))
    ));
    // Case stays significant.
    let mut lower = {
        let id = rng.uuid();
        random_row(&mut rng, &w, id)
    };
    lower.internal_pn = InternalPn::new("pn-a");
    adapter.insert_row("t", lower, "case").unwrap();
}
