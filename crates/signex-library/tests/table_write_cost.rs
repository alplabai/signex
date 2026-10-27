//! Measurement for #416: wall-clock cost of a one-row edit on a large table.
//!
//! Ignored by default (timing, not correctness). Run with
//! `cargo test -p signex-library --test table_write_cost --release -- --ignored --nocapture`.

use std::time::Instant;

use chrono::Utc;
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
