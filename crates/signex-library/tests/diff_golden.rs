//! Integration tests for the v0.9 refactored revision-diff engine.
//!
//! The pre-refactor diff parsed pin/pad geometry from embedded S-expression
//! strings; the new diff (per `v0.9-library-refactor-plan.md` §7 step B5) is
//! pure-ref + binding-field comparison. Geometry-level diffs live with the
//! primitive editors and are out of scope here.
//!
//! These tests build three synthetic revisions of the same component and
//! verify:
//! - v1.0 → v1.1 (MPN-only swap) is a Minor bump,
//! - v1.0 → v2.0 (symbol_ref swap) is a Major bump,
//! - the diff is symmetric (added/removed swap on reversal),
//! - lifecycle transitions surface in `lifecycle_detail`.

use std::path::PathBuf;

use signex_library::*;
use uuid::Uuid;

fn fixed_uuid(seed: u8) -> Uuid {
    let mut bytes = [0u8; 16];
    bytes[0] = seed;
    Uuid::from_bytes(bytes)
}

fn rev_with(symbol: PrimitiveRef, footprint: Option<PrimitiveRef>, mpn: &str) -> Revision {
    let mut r = Revision {
        version: Version::new(1, 0),
        state: LifecycleState::Released,
        created: chrono::DateTime::from_timestamp(1_745_625_600, 0).unwrap(),
        author: "fixture@signex".into(),
        message: "initial release".into(),
        symbol_ref: symbol,
        footprint_ref: footprint,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Yageo", mpn),
        alternates: Vec::new(),
        supply: vec![DistributorListing::new("DigiKey", "311-10.0KCRCT-ND")],
        datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        content_hash: [0u8; 32],
    };
    r.refresh_content_hash();
    r
}

fn fixture_component(symbol: PrimitiveRef, footprint: PrimitiveRef) -> Component {
    Component {
        uuid: fixed_uuid(1),
        internal_pn: InternalPn::new("R0805_10k"),
        class: ComponentClass::new("resistor"),
        category: PathBuf::from("Passives/Resistors/0805"),
        family: None,
        head: Version::new(1, 0),
        revisions: vec![rev_with(symbol, Some(footprint), "RC0805FR-0710KL")],
    }
}

#[test]
fn v1_0_to_v1_1_mpn_swap_is_minor_bump() {
    let lib = fixed_uuid(2);
    let sym = PrimitiveRef::new(lib, fixed_uuid(3));
    let fp = PrimitiveRef::new(lib, fixed_uuid(4));

    let v1_0 = rev_with(sym, Some(fp), "RC0805FR-0710KL");
    let mut v1_1 = rev_with(sym, Some(fp), "RC0805FR-0710KL_REISSUE");
    v1_1.message = "supplier MPN reissued".into();

    let d = diff_revisions(&v1_0, &v1_1);
    assert!(d.mpn_changed);
    assert!(!d.symbol_changed);
    assert!(!d.footprint_changed);
    assert_eq!(auto_bump_kind(&d), BumpKind::Minor);
}

#[test]
fn v1_0_to_v2_0_symbol_ref_swap_is_major_bump() {
    let lib = fixed_uuid(2);
    let sym_old = PrimitiveRef::new(lib, fixed_uuid(3));
    let sym_new = PrimitiveRef::new(lib, fixed_uuid(5));
    let fp = PrimitiveRef::new(lib, fixed_uuid(4));

    let v1_0 = rev_with(sym_old, Some(fp), "X");
    let v2_0 = rev_with(sym_new, Some(fp), "X");

    let d = diff_revisions(&v1_0, &v2_0);
    assert!(d.symbol_changed);
    assert_eq!(auto_bump_kind(&d), BumpKind::Major);
}

#[test]
fn diff_is_symmetric_on_added_supply() {
    let lib = fixed_uuid(2);
    let sym = PrimitiveRef::new(lib, fixed_uuid(3));
    let fp = PrimitiveRef::new(lib, fixed_uuid(4));

    let mut a = rev_with(sym, Some(fp), "X");
    a.supply.clear();
    let mut b = rev_with(sym, Some(fp), "X");
    b.supply = vec![DistributorListing::new("Mouser", "M-1")];

    let forward = diff_revisions(&a, &b);
    let reverse = diff_revisions(&b, &a);

    assert_eq!(forward.supply_detail.added, reverse.supply_detail.removed);
    assert_eq!(forward.supply_detail.removed, reverse.supply_detail.added);
}

#[test]
fn lifecycle_diff_records_state_change_when_present() {
    let lib = fixed_uuid(2);
    let sym = PrimitiveRef::new(lib, fixed_uuid(3));
    let fp = PrimitiveRef::new(lib, fixed_uuid(4));

    let mut a = rev_with(sym, Some(fp), "X");
    let mut b = rev_with(sym, Some(fp), "X");

    // Same state: no lifecycle delta.
    let d = diff_revisions(&a, &b);
    assert!(!d.lifecycle_changed);

    a.state = LifecycleState::Released;
    b.state = LifecycleState::Deprecated;
    let d = diff_revisions(&a, &b);
    assert!(d.lifecycle_changed);
    assert_eq!(d.lifecycle_detail.from, Some(LifecycleState::Released));
    assert_eq!(d.lifecycle_detail.to, Some(LifecycleState::Deprecated));
}

#[test]
fn fixture_component_is_round_trippable_through_snxprt() {
    let lib = fixed_uuid(2);
    let sym = PrimitiveRef::new(lib, fixed_uuid(3));
    let fp = PrimitiveRef::new(lib, fixed_uuid(4));
    let comp = fixture_component(sym, fp);

    let file = SnxPartFile {
        schema_version: 2,
        component: comp.clone(),
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(snxpart_filename(comp.uuid));
    write_snxpart(&path, &file).unwrap();
    let back = read_snxpart(&path).unwrap();
    assert_eq!(file, back);
}
