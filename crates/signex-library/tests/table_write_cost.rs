//! Measurement for #416: wall-clock cost of a one-row edit on a large table.
//!
//! Ignored by default (timing, not correctness). Run with
//! `cargo test -p signex-library --test table_write_cost --release -- --ignored --nocapture`.

use std::time::Instant;

use chrono::Utc;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::*;
use uuid::Uuid;

fn row(n: usize, lib: Uuid) -> ComponentRow {
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(format!("R0805_{n}")),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
        footprint_ref: Some(PrimitiveRef::new(lib, Uuid::new_v4())),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Yageo", format!("RC0805FR-{n}")),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: Utc::now(),
        updated: Utc::now(),
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

#[test]
#[ignore = "timing measurement for #416"]
fn one_row_edit_cost_by_table_size() {
    let lib = Uuid::new_v4();
    for size in [1_000usize, 10_000, 50_000] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.tsv");
        let rows: Vec<_> = (0..size).map(|n| row(n, lib)).collect();
        write_table(&path, &rows).unwrap();
        let bytes = std::fs::metadata(&path).unwrap().len();

        let mut edited = rows[size / 2].clone();
        edited.version = "0.0.2".into();
        let t = Instant::now();
        update_row(&path, &edited).unwrap();
        let upd = t.elapsed();
        let t = Instant::now();
        append_row(&path, &row(size, lib)).unwrap();
        let app = t.elapsed();
        println!("rows={size} bytes={bytes} update_row={upd:?} append_row={app:?}");
    }
}

/// The path the app actually takes: `LocalGitAdapter` edits rewrite the whole
/// `.snxlib` (clone, encode, atomic write, commit).
#[test]
#[ignore = "timing measurement for #416"]
fn adapter_one_row_edit_cost_by_library_size() {
    signex_library::git_identity::test_identity::install();
    for size in [1_000usize, 10_000, 50_000] {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("lib").join("lib.snxlib");
        let manifest = SnxlibManifest {
            format: FORMAT_TOKEN.into(),
            library_id: Uuid::now_v7(),
            library: LibrarySection {
                name: "lib".into(),
                description: None,
            },
            mode: LibraryMode::default(),
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            classes: Vec::new(),
        };
        let lib = manifest.library_id;
        let opts = LibraryInitOptions {
            enable_git: true,
            use_lfs: false,
        };
        let adapter = LocalGitAdapter::init(&file, manifest, opts).unwrap();
        let seed = row(0, lib);
        adapter
            .insert_row("resistors", seed.clone(), "seed")
            .unwrap();

        // Bulk-seed by cloning the stored row with fresh ids; one insert per
        // row would be quadratic.
        let mut lf = LibraryFile::parse(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let table = lf.tables.get_mut("resistors").unwrap();
        let stored = table.rows[0].clone();
        for _ in 1..size {
            let mut r = stored.clone();
            r.cells.insert("row_id".into(), Uuid::now_v7().to_string());
            table.rows.push(r);
        }
        std::fs::write(&file, lf.write().unwrap()).unwrap();
        let bytes = std::fs::metadata(&file).unwrap().len();
        let adapter = LocalGitAdapter::open(&file).unwrap();

        let mut edited = seed.clone();
        edited.version = "0.0.2".into();
        let t = Instant::now();
        adapter.update_row("resistors", edited, "edit").unwrap();
        let upd = t.elapsed();
        let t = Instant::now();
        adapter
            .insert_row("resistors", row(size, lib), "add")
            .unwrap();
        let ins = t.elapsed();
        let t = Instant::now();
        adapter
            .delete_row("resistors", RowId::from_uuid(seed.row_id), None, "del")
            .unwrap();
        let del = t.elapsed();
        println!(
            "adapter rows={size} bytes={bytes} update_row={upd:?} insert_row={ins:?} delete_row={del:?}"
        );
    }
}
