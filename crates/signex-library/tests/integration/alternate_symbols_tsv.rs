//! `.snxlib` storage of `alternate_symbols`: an optional 22nd column that
//! appears only once a row carries an alternate.

#![cfg(feature = "local-git")]

use std::path::PathBuf;

use chrono::Utc;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::*;
use uuid::Uuid;

/// An empty git-less library at `<dir>/L/L.snxlib`.
fn new_library() -> (tempfile::TempDir, PathBuf, LocalGitAdapter) {
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("L").join("L.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "L".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let opts = LibraryInitOptions {
        enable_git: false,
        use_lfs: false,
    };
    let adapter = LocalGitAdapter::init(&file, manifest, opts).unwrap();
    (dir, file, adapter)
}

/// Column header of table `t` as stored in the file.
fn stored_columns(file: &PathBuf) -> Vec<String> {
    let text = std::fs::read_to_string(file).unwrap();
    LibraryFile::parse(&text).unwrap().tables["t"]
        .columns
        .clone()
}

fn row(pn: &str) -> ComponentRow {
    let lib = Uuid::now_v7();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::url(""),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", pn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: "0.0.1".into(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: Utc::now(),
        updated: Utc::now(),
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

fn with_alternate(mut r: ComponentRow) -> ComponentRow {
    r.alternate_symbols.push(AlternateSymbol::new(
        PrimitiveRef::new(r.symbol_ref.library_id, Uuid::now_v7()),
        "0.0.2",
        "IEC",
    ));
    r
}

#[test]
fn table_without_alternates_keeps_the_legacy_21_column_layout() {
    let (_dir, file, adapter) = new_library();
    adapter.insert_row("t", row("A"), "seed").unwrap();
    adapter.insert_row("t", row("B"), "seed").unwrap();
    assert_eq!(stored_columns(&file), TABLE_HEADER);
}

#[test]
fn legacy_21_column_table_reads_with_no_alternates() {
    let (_dir, _file, adapter) = new_library();
    let original = row("A");
    adapter.insert_row("t", original.clone(), "seed").unwrap();
    let back = adapter
        .read_row("t", RowId::from_uuid(original.row_id))
        .unwrap();
    assert_eq!(back.internal_pn, original.internal_pn);
    assert!(back.alternate_symbols.is_empty());
}

#[test]
fn alternates_round_trip_through_the_22_column_layout() {
    let (_dir, file, adapter) = new_library();
    let with = with_alternate(row("A"));
    adapter.insert_row("t", with.clone(), "seed").unwrap();
    adapter.insert_row("t", row("B"), "seed").unwrap();

    let columns = stored_columns(&file);
    assert_eq!(columns.last().unwrap(), ALTERNATE_SYMBOLS_COLUMN);
    assert_eq!(columns.len(), TABLE_HEADER.len() + 1);
    let back = adapter
        .read_row("t", RowId::from_uuid(with.row_id))
        .unwrap();
    assert_eq!(back.alternate_symbols, with.alternate_symbols);
}

#[test]
fn any_other_column_layout_still_errors() {
    for extra in ["\textra\tmore", "\tnot_alternates"] {
        let (_dir, file, adapter) = new_library();
        let r = row("A");
        adapter.insert_row("t", r.clone(), "seed").unwrap();
        adapter
            .delete_row("t", RowId::from_uuid(r.row_id), None, "empty the table")
            .unwrap();
        drop(adapter);
        let text = std::fs::read_to_string(&file).unwrap();
        let header = TABLE_HEADER.join("\t");
        assert!(text.contains(&header), "header line not found");
        std::fs::write(&file, text.replace(&header, &format!("{header}{extra}"))).unwrap();

        // Either open or the first row read must refuse the layout.
        let refused = match LocalGitAdapter::open(&file) {
            Err(_) => true,
            Ok(adapter) => adapter.iter_rows().is_err(),
        };
        assert!(refused, "layout with {extra:?} was accepted");
    }
}

#[test]
fn write_refuses_an_alternate_equal_to_the_default_symbol() {
    let (_dir, _file, adapter) = new_library();
    let mut r = row("A");
    r.alternate_symbols
        .push(AlternateSymbol::new(r.symbol_ref, "0.0.1", "same"));
    assert!(adapter.insert_row("t", r, "seed").is_err());
}
