//! `read_row` converts one row, `snapshot_table` converts only rows that
//! changed, and no read serves stale content (#1437).

use super::write_oracle_tests::{Rng, World, random_row, seeded};
use super::*;
use crate::identity::InternalPn;

const ROWS: usize = 40;

/// A random row whose text cells are plain, so it round-trips the file.
fn clean_row(rng: &mut Rng, w: &World, id: uuid::Uuid, pn: &str) -> ComponentRow {
    let mut row = random_row(rng, w, id);
    row.internal_pn = InternalPn::new(pn);
    row.primary_mpn = crate::manufacturer::ManufacturerPart::draft("Acme", format!("M-{pn}"));
    row
}

/// A library holding `ROWS` rows with unique part numbers, and their ids.
fn library() -> (tempfile::TempDir, LocalGitAdapter, World, Rng, Vec<RowId>) {
    let mut rng = Rng(11);
    let w = World {
        lib: rng.uuid(),
        symbols: (0..3).map(|_| rng.uuid()).collect(),
        footprints: (0..3).map(|_| rng.uuid()).collect(),
    };
    let dir = tempfile::tempdir().unwrap();
    let file = seeded(dir.path(), &mut rng, &w, 0, false);
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let mut ids = Vec::new();
    for n in 0..ROWS {
        let id = rng.uuid();
        let row = clean_row(&mut rng, &w, id, &format!("PN-{n}"));
        adapter.insert_row("t", row, "seed").unwrap();
        ids.push(RowId::from_uuid(id));
    }
    (dir, adapter, w, rng, ids)
}

fn by_id(rows: &[ComponentRow], id: uuid::Uuid) -> &ComponentRow {
    rows.iter().find(|r| r.row_id == id).unwrap()
}

fn converted_by<T>(adapter: &LocalGitAdapter, f: impl FnOnce() -> T) -> (T, usize) {
    let before = adapter.row_reads.conversions();
    let out = f();
    (out, adapter.row_reads.conversions() - before)
}

/// What a table read must equal: every row converted afresh.
fn fresh(adapter: &LocalGitAdapter) -> Vec<ComponentRow> {
    let guard = adapter.library_file.read().unwrap();
    guard.tables["t"]
        .rows
        .iter()
        .map(|r| adapter.row_reads.convert(r).unwrap())
        .collect()
}

#[test]
fn read_row_converts_only_the_requested_row() {
    let (_dir, adapter, _, _, ids) = library();
    let (row, n) = converted_by(&adapter, || adapter.read_row("t", ids[7]).unwrap());
    assert_eq!(row.row_id, ids[7].as_uuid());
    assert_eq!(n, 1);
    assert!(matches!(
        adapter.read_row("t", RowId::from_uuid(uuid::Uuid::now_v7())),
        Err(LibraryError::NotFound(_))
    ));
    assert!(matches!(
        adapter.read_row("nope", ids[0]),
        Err(LibraryError::NotFound(_))
    ));
}

#[test]
fn a_second_table_read_converts_nothing() {
    let (_dir, adapter, ..) = library();
    let (first, _) = converted_by(&adapter, || adapter.read_table("t").unwrap());
    let (second, n) = converted_by(&adapter, || adapter.read_table("t").unwrap());
    assert_eq!(n, 0);
    assert_eq!(first, second);
    assert_eq!(second, fresh(&adapter));
}

#[test]
fn an_edit_through_the_adapter_converts_one_row_and_shows_it() {
    let (_dir, adapter, ..) = library();
    let before = adapter.read_table("t").unwrap();
    let mut edited = before[5].clone();
    edited.description = "d-9.9.9".into();
    adapter.update_row("t", edited, "edit").unwrap();

    let (after, n) = converted_by(&adapter, || adapter.read_table("t").unwrap());
    assert_eq!(n, 1, "only the edited row is converted again");
    let id = before[5].row_id;
    assert_eq!(by_id(&after, id).description, "d-9.9.9");
    assert_eq!(after, fresh(&adapter));
    assert_eq!(
        &adapter.read_row("t", RowId::from_uuid(id)).unwrap(),
        by_id(&after, id)
    );
}

#[test]
fn insert_and_delete_show_in_the_next_read() {
    let (_dir, adapter, w, mut rng, ids) = library();
    adapter.read_table("t").unwrap();
    adapter.delete_row("t", ids[3], None, "del").unwrap();
    let id = rng.uuid();
    let row = clean_row(&mut rng, &w, id, "PN-NEW");
    adapter.insert_row("t", row, "add").unwrap();

    let (rows, n) = converted_by(&adapter, || adapter.read_table("t").unwrap());
    assert_eq!(n, 1, "only the inserted row is converted");
    assert_eq!(rows.len(), ROWS);
    assert!(rows.iter().all(|r| r.row_id != ids[3].as_uuid()));
    assert!(rows.iter().any(|r| r.row_id == id));
    assert_eq!(rows, fresh(&adapter));
}

#[test]
fn another_adapter_editing_the_file_shows_in_the_next_read() {
    let (_dir, adapter, ..) = library();
    let warm = adapter.read_table("t").unwrap();
    let other = LocalGitAdapter::open(adapter.file_path_buf()).unwrap();
    let mut edited = other
        .read_row("t", RowId::from_uuid(warm[9].row_id))
        .unwrap();
    edited.description = "d-7.7.7".into();
    other.update_row("t", edited, "external").unwrap();

    let rows = adapter.read_table("t").unwrap();
    assert_eq!(by_id(&rows, warm[9].row_id).description, "d-7.7.7");
    assert_eq!(rows, fresh(&adapter));
    let id = RowId::from_uuid(warm[9].row_id);
    assert_eq!(adapter.read_row("t", id).unwrap().description, "d-7.7.7");
}

/// A checkout or pull replaces the file under the adapter: rows change,
/// vanish and appear, and the length changes.
#[test]
fn a_file_replaced_on_disk_never_serves_old_rows() {
    let (_dir, adapter, ..) = library();
    let warm = adapter.read_table("t").unwrap();
    let path = adapter.file_path_buf().to_path_buf();
    let mut lf = LibraryFile::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let table = lf.tables.get_mut("t").unwrap();
    table.rows.truncate(10);
    let changed = std::sync::Arc::make_mut(&mut table.rows[2]);
    changed.cells.insert("description".into(), "d-3.3.3".into());
    std::fs::write(&path, lf.write().unwrap()).unwrap();

    let rows = adapter.read_table("t").unwrap();
    assert_eq!(rows.len(), 10);
    assert_eq!(rows[2].description, "d-3.3.3");
    assert_eq!(rows, fresh(&adapter));
    let gone = RowId::from_uuid(warm[20].row_id);
    assert!(matches!(
        adapter.read_row("t", gone),
        Err(LibraryError::NotFound(_))
    ));
}

#[test]
fn a_table_that_disappears_reads_empty() {
    let (_dir, adapter, ..) = library();
    adapter.read_table("t").unwrap();
    let path = adapter.file_path_buf().to_path_buf();
    let mut lf = LibraryFile::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    lf.tables.remove("t");
    std::fs::write(&path, lf.write().unwrap()).unwrap();
    assert!(adapter.read_table("t").unwrap().is_empty());
}

/// `read_row` is the first read after another writer changed the file: it
/// must look at the disk itself, not rely on an earlier `read_table`.
#[test]
fn read_row_first_after_an_external_edit_sees_the_edit() {
    let (_dir, adapter, _, _, ids) = library();
    let other = LocalGitAdapter::open(adapter.file_path_buf()).unwrap();
    let mut edited = other.read_row("t", ids[4]).unwrap();
    edited.description = "d-first".into();
    other.update_row("t", edited, "external").unwrap();

    assert_eq!(
        adapter.read_row("t", ids[4]).unwrap().description,
        "d-first"
    );
}

#[test]
fn read_row_finds_an_id_written_in_another_uuid_spelling() {
    let (_dir, adapter, ..) = library();
    let path = adapter.file_path_buf().to_path_buf();
    let mut lf = LibraryFile::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let table = lf.tables.get_mut("t").unwrap();
    let id = table.rows[3].cells["row_id"].clone();
    let spelled = id.replace('-', "").to_uppercase();
    std::sync::Arc::make_mut(&mut table.rows[3])
        .cells
        .insert("row_id".into(), spelled);
    std::fs::write(&path, lf.write().unwrap()).unwrap();

    let shown = adapter.read_table("t").unwrap();
    let wanted = uuid::Uuid::parse_str(&id).unwrap();
    assert!(shown.iter().any(|r| r.row_id == wanted));
    assert_eq!(
        adapter
            .read_row("t", RowId::from_uuid(wanted))
            .unwrap()
            .row_id,
        wanted
    );
}

#[test]
fn a_renamed_table_frees_its_old_entry() {
    let (_dir, adapter, ..) = library();
    adapter.read_table("t").unwrap();
    assert_eq!(adapter.row_reads.kept_tables(), vec!["t".to_string()]);
    adapter.rename_table("t", "u", "rename").unwrap();
    adapter.read_table("u").unwrap();
    assert_eq!(adapter.row_reads.kept_tables(), vec!["u".to_string()]);
}

#[test]
fn a_reload_from_disk_frees_the_rows_it_replaced() {
    let (_dir, adapter, ..) = library();
    adapter.read_table("t").unwrap();
    assert_eq!(adapter.row_reads.kept_tables(), vec!["t".to_string()]);
    let other = LocalGitAdapter::open(adapter.file_path_buf()).unwrap();
    let mut row = other.read_table("t").unwrap().remove(0);
    row.description = "d-reload".into();
    other.update_row("t", row, "external").unwrap();

    // Reload without a table read: nothing may stay behind.
    adapter.refresh_for_read();
    assert!(adapter.row_reads.kept_tables().is_empty());
}
