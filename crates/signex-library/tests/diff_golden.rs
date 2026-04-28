//! Integration tests for the v0.9-refactor-2 row-diff engine.
//!
//! Per `v0.9-refactor-2-plan.md` §6 step 1.7, the diff is pure-ref +
//! binding-field comparison over [`ComponentRow`] pairs. Geometry-level
//! diffs live with the primitive editors and are out of scope here.
//!
//! These tests build three synthetic rows of the same internal_pn and
//! verify:
//! - mpn-only swap is a Minor bump,
//! - symbol_ref swap is a Major bump,
//! - the diff is symmetric (added/removed swap on reversal),
//! - lifecycle transitions surface in `lifecycle_detail`.

use signex_library::*;
use uuid::Uuid;

fn fixed_uuid(seed: u8) -> Uuid {
    let mut bytes = [0u8; 16];
    bytes[0] = seed;
    Uuid::from_bytes(bytes)
}

fn row_with(symbol: PrimitiveRef, footprint: Option<PrimitiveRef>, mpn: &str) -> ComponentRow {
    let t = chrono::DateTime::from_timestamp(1_745_625_600, 0).unwrap();
    let mut r = ComponentRow {
        row_id: fixed_uuid(1),
        internal_pn: InternalPn::new("R0805_10k"),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
        state: LifecycleState::Released,
        symbol_ref: symbol,
        footprint_ref: footprint,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Yageo", mpn),
        alternates: Vec::new(),
        supply: vec![DistributorListing::new("DigiKey", "311-10.0KCRCT-ND")],
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: t,
        updated: t,
        content_hash: [0u8; 32],
    };
    r.refresh_content_hash().unwrap();
    r
}

#[test]
fn mpn_only_swap_is_minor_bump() {
    let lib = fixed_uuid(2);
    let sym = PrimitiveRef::new(lib, fixed_uuid(3));
    let fp = PrimitiveRef::new(lib, fixed_uuid(4));

    let a = row_with(sym, Some(fp), "RC0805FR-0710KL");
    let b = row_with(sym, Some(fp), "RC0805FR-0710KL_REISSUE");

    let d = diff_rows(&a, &b);
    assert!(d.mpn_changed);
    assert!(!d.symbol_changed);
    assert!(!d.footprint_changed);
    assert_eq!(auto_bump_kind(&d), BumpKind::Minor);
}

#[test]
fn symbol_ref_swap_is_major_bump() {
    let lib = fixed_uuid(2);
    let sym_old = PrimitiveRef::new(lib, fixed_uuid(3));
    let sym_new = PrimitiveRef::new(lib, fixed_uuid(5));
    let fp = PrimitiveRef::new(lib, fixed_uuid(4));

    let a = row_with(sym_old, Some(fp), "X");
    let b = row_with(sym_new, Some(fp), "X");

    let d = diff_rows(&a, &b);
    assert!(d.symbol_changed);
    assert_eq!(auto_bump_kind(&d), BumpKind::Major);
}

#[test]
fn diff_is_symmetric_on_added_supply() {
    let lib = fixed_uuid(2);
    let sym = PrimitiveRef::new(lib, fixed_uuid(3));
    let fp = PrimitiveRef::new(lib, fixed_uuid(4));

    let mut a = row_with(sym, Some(fp), "X");
    a.supply.clear();
    let mut b = row_with(sym, Some(fp), "X");
    b.supply = vec![DistributorListing::new("Mouser", "M-1")];

    let forward = diff_rows(&a, &b);
    let reverse = diff_rows(&b, &a);

    assert_eq!(forward.supply_detail.added, reverse.supply_detail.removed);
    assert_eq!(forward.supply_detail.removed, reverse.supply_detail.added);
}

#[test]
fn lifecycle_diff_records_state_change_when_present() {
    let lib = fixed_uuid(2);
    let sym = PrimitiveRef::new(lib, fixed_uuid(3));
    let fp = PrimitiveRef::new(lib, fixed_uuid(4));

    let mut a = row_with(sym, Some(fp), "X");
    let mut b = row_with(sym, Some(fp), "X");

    // Same state: no lifecycle delta.
    let d = diff_rows(&a, &b);
    assert!(!d.state_changed);

    a.state = LifecycleState::Released;
    b.state = LifecycleState::Deprecated;
    let d = diff_rows(&a, &b);
    assert!(d.state_changed);
    assert_eq!(d.lifecycle_detail.from, Some(LifecycleState::Released));
    assert_eq!(d.lifecycle_detail.to, Some(LifecycleState::Deprecated));
}
