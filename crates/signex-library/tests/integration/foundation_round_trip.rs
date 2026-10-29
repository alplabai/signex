//! Round-trip integration smoke tests for the row model: a row stored in a
//! `.snxlib` table reads back unchanged, and the parameter-template
//! validation pipeline works on the row payload.

#![cfg(feature = "local-git")]

use chrono::Utc;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::*;
use uuid::Uuid;

#[test]
fn full_row_round_trip_via_snxlib() {
    let lib = Uuid::new_v4();
    let row = ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("R0805_10k"),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
        footprint_ref: Some(PrimitiveRef::new(lib, Uuid::new_v4())),
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Yageo", "RC0805FR-0710KL"),
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
    };

    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("L").join("L.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: lib,
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
    adapter
        .insert_row("resistors", row.clone(), "seed")
        .unwrap();
    drop(adapter);

    // A fresh open reads what is on disk, not the writer's memory.
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let back = adapter.read_table("resistors").unwrap();
    assert_eq!(back.len(), 1);
    let back = &back[0];
    assert_eq!(back.row_id, row.row_id);
    assert_eq!(back.internal_pn, row.internal_pn);
    assert_eq!(back.class, row.class);
    assert_eq!(back.datasheet, row.datasheet);
    assert_eq!(back.state, row.state);
    assert_eq!(back.symbol_ref, row.symbol_ref);
    assert_eq!(back.footprint_ref, row.footprint_ref);
    assert_eq!(back.primary_mpn, row.primary_mpn);
    assert_eq!(back.version, row.version);
}

#[test]
fn template_registry_validates_round_trip() {
    let r = TemplateRegistry::new_with_builtins();
    let mut params = ParamMap::new();
    params.insert(
        "value".into(),
        ParamValue::Measurement {
            value: 10_000.0,
            unit: "ohm".into(),
        },
    );
    params.insert(
        "tolerance".into(),
        ParamValue::Measurement {
            value: 1.0,
            unit: "%".into(),
        },
    );
    params.insert(
        "power".into(),
        ParamValue::Measurement {
            value: 0.125,
            unit: "W".into(),
        },
    );
    let v = r.validate_params(Uuid::nil(), "resistor", &params);
    assert!(v.is_empty(), "valid resistor params: got {:?}", v);
}
