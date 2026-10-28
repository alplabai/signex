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
