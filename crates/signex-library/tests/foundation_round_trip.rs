//! Round-trip integration smoke tests for the v0.9 refactored library shape.
//!
//! Per `v0.9-library-refactor-plan.md` §7, the on-disk format flips from
//! "one .snxpart per revision" to "one .snxprt per component (carries every
//! revision)" and the schema version bumps to `2`.

use std::path::PathBuf;

use signex_library::*;
use uuid::Uuid;

#[test]
fn full_component_round_trip_via_snxprt() {
    let lib = Uuid::new_v4();
    let mut rev = Revision {
        version: Version::new(1, 0),
        state: LifecycleState::Released,
        created: chrono::Utc::now(),
        author: "caner@alplab".into(),
        message: "initial release".into(),
        symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
        footprint_ref: Some(PrimitiveRef::new(lib, Uuid::new_v4())),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Yageo", "RC0805FR-0710KL"),
        alternates: Vec::new(),
        supply: Vec::new(),
        datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        content_hash: [0u8; 32],
    };
    rev.refresh_content_hash();

    let comp = Component {
        uuid: Uuid::now_v7(),
        internal_pn: InternalPn::new("R0805_10k"),
        class: ComponentClass::new("resistor"),
        category: PathBuf::from("Passives/Resistors/0805"),
        family: None,
        revisions: vec![rev],
        head: Version::new(1, 0),
    };

    let file = SnxPartFile {
        schema_version: 2,
        component: comp,
    };

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(snxpart_filename(file.component.uuid));
    write_snxpart(&path, &file).unwrap();
    let back = read_snxpart(&path).unwrap();
    assert_eq!(file, back);
    assert_ne!(
        back.component.head_revision().unwrap().content_hash,
        [0u8; 32]
    );
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
