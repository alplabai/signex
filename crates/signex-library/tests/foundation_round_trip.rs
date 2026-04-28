//! Round-trip integration smoke tests for the v0.9-refactor-2 row model.
//!
//! Per `v0.9-refactor-2-plan.md` §3, the on-disk format flips from
//! "one .snxprt per component" to "one row inside `tables/<name>.tsv`".
//! This test verifies the table TSV round-trip and the parameter-template
//! validation pipeline still works on the new row payload.

use chrono::Utc;
use signex_library::*;
use uuid::Uuid;

#[test]
fn full_row_round_trip_via_tsv() {
    let lib = Uuid::new_v4();
    let row = ComponentRow {
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("R0805_10k"),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
        footprint_ref: Some(PrimitiveRef::new(lib, Uuid::new_v4())),
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
    };

    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_table(tmp.path(), std::slice::from_ref(&row)).unwrap();
    let back = read_table(tmp.path()).unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0], row);
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
