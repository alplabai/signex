//! Golden tests for the WS-D revision diff engine.
//!
//! Three fixture revisions cover the headline cases:
//!
//! * `r0805_v1_0.snxpart` — baseline 0805 resistor (2 pins).
//! * `r0805_v1_1.snxpart` — same symbol/footprint, only `shared.mpn` changes
//!   vs v1.0 → minor-bump worthy.
//! * `r0805_v2_0.snxpart` — adds a third pin → major-bump worthy.
//!
//! Fixtures are committed JSON. To regenerate them after schema changes, run:
//!
//! ```bash
//! cargo test -p signex-library --test diff_golden -- --ignored regenerate_fixtures
//! ```

use std::path::{Path, PathBuf};

use signex_library::diff::{BumpKind, auto_bump_kind, diff_revisions};
use signex_library::*;
use uuid::Uuid;

// A fixed UUID so v1.0 / v1.1 / v2.0 share the same logical identity.
// (UUIDv7 timestamp is from 2026-04-26, the day this fixture was authored —
// not load-bearing, just deterministic.)
const FIXTURE_UUID: &str = "01964e00-0000-7000-8000-000000000000";
const FIXTURE_INTERNAL_PN: &str = "R0805_10k";

const SYMBOL_2_PINS: &str = r#"(symbol "R0805"
  (pin passive line (at -2.54 0 0) (length 1.27)
    (name "1") (number "1"))
  (pin passive line (at  2.54 0 180) (length 1.27)
    (name "2") (number "2")))"#;

const SYMBOL_3_PINS: &str = r#"(symbol "R0805_KELVIN"
  (pin passive line (at -2.54 0 0) (length 1.27)
    (name "1") (number "1"))
  (pin passive line (at  2.54 0 180) (length 1.27)
    (name "2") (number "2"))
  (pin passive line (at  0 2.54 270) (length 1.27)
    (name "SENSE") (number "3")))"#;

const FOOTPRINT_2_PADS: &str = r#"(footprint "R_0805_2012Metric"
  (pad "1" smd rect (at -0.9 0) (size 1.025 1.4) (layers F.Cu F.Mask))
  (pad "2" smd rect (at  0.9 0) (size 1.025 1.4) (layers F.Cu F.Mask)))"#;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn load(name: &str) -> SnxPartFile {
    let path = fixtures_dir().join(name);
    read_snxpart(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn fixtures_are_present_and_readable() {
    let v1_0 = load("r0805_v1_0.snxpart");
    let v1_1 = load("r0805_v1_1.snxpart");
    let v2_0 = load("r0805_v2_0.snxpart");

    assert_eq!(v1_0.revision.version, Version::new(1, 0));
    assert_eq!(v1_1.revision.version, Version::new(1, 1));
    assert_eq!(v2_0.revision.version, Version::new(2, 0));

    // Same logical component across revisions.
    assert_eq!(v1_0.uuid, v1_1.uuid);
    assert_eq!(v1_0.uuid, v2_0.uuid);
    assert_eq!(v1_0.internal_pn, v1_1.internal_pn);
}

#[test]
fn v1_0_to_v1_1_has_no_symbol_or_footprint_change() {
    let v1_0 = load("r0805_v1_0.snxpart");
    let v1_1 = load("r0805_v1_1.snxpart");

    let d = diff_revisions(&v1_0.revision, &v1_1.revision);

    assert!(
        d.symbol.added_pins.is_empty(),
        "v1.0→v1.1 should not add pins, got {:?}",
        d.symbol.added_pins
    );
    assert!(
        d.symbol.removed_pins.is_empty(),
        "v1.0→v1.1 should not remove pins, got {:?}",
        d.symbol.removed_pins
    );
    assert!(
        d.symbol.moved_pins.is_empty(),
        "v1.0→v1.1 should not move pins, got {:?}",
        d.symbol.moved_pins
    );
    assert!(d.footprint.added_pads.is_empty());
    assert!(d.footprint.removed_pads.is_empty());
    assert!(
        d.parameters.added.is_empty()
            && d.parameters.removed.is_empty()
            && d.parameters.changed.is_empty(),
        "v1.0→v1.1 only changes shared.mpn, not parameters"
    );
}

#[test]
fn v1_0_to_v1_1_is_minor_bump() {
    let v1_0 = load("r0805_v1_0.snxpart");
    let v1_1 = load("r0805_v1_1.snxpart");

    let d = diff_revisions(&v1_0.revision, &v1_1.revision);
    assert_eq!(auto_bump_kind(&d), BumpKind::Minor);
}

#[test]
fn v1_0_to_v2_0_adds_at_least_one_pin() {
    let v1_0 = load("r0805_v1_0.snxpart");
    let v2_0 = load("r0805_v2_0.snxpart");

    let d = diff_revisions(&v1_0.revision, &v2_0.revision);
    assert!(
        !d.symbol.added_pins.is_empty(),
        "v1.0→v2.0 should add pins, symbol diff was {:?}",
        d.symbol
    );
}

#[test]
fn v1_0_to_v2_0_is_major_bump() {
    let v1_0 = load("r0805_v1_0.snxpart");
    let v2_0 = load("r0805_v2_0.snxpart");

    let d = diff_revisions(&v1_0.revision, &v2_0.revision);
    assert_eq!(auto_bump_kind(&d), BumpKind::Major);
}

#[test]
fn diff_is_symmetric_swap_added_removed() {
    let v1_0 = load("r0805_v1_0.snxpart");
    let v2_0 = load("r0805_v2_0.snxpart");

    let forward = diff_revisions(&v1_0.revision, &v2_0.revision);
    let reverse = diff_revisions(&v2_0.revision, &v1_0.revision);

    // Sort copies because the orderings of intermediate sets may differ.
    fn sorted(v: &[String]) -> Vec<String> {
        let mut x = v.to_vec();
        x.sort();
        x
    }

    assert_eq!(
        sorted(&forward.symbol.added_pins),
        sorted(&reverse.symbol.removed_pins),
        "added pins forward must equal removed pins reverse"
    );
    assert_eq!(
        sorted(&forward.symbol.removed_pins),
        sorted(&reverse.symbol.added_pins),
        "removed pins forward must equal added pins reverse"
    );
    assert_eq!(
        sorted(&forward.footprint.added_pads),
        sorted(&reverse.footprint.removed_pads)
    );
    assert_eq!(
        sorted(&forward.footprint.removed_pads),
        sorted(&reverse.footprint.added_pads)
    );
    assert_eq!(
        sorted(&forward.parameters.added),
        sorted(&reverse.parameters.removed)
    );
    assert_eq!(
        sorted(&forward.parameters.removed),
        sorted(&reverse.parameters.added)
    );
    assert_eq!(
        sorted(&forward.suppliers.added),
        sorted(&reverse.suppliers.removed)
    );
    assert_eq!(
        sorted(&forward.suppliers.removed),
        sorted(&reverse.suppliers.added)
    );
}

#[test]
fn lifecycle_diff_records_state_change_when_present() {
    // v1.0 vs v1.1 in our fixtures both stay Released → lifecycle.from is None.
    let v1_0 = load("r0805_v1_0.snxpart");
    let v1_1 = load("r0805_v1_1.snxpart");

    let d = diff_revisions(&v1_0.revision, &v1_1.revision);
    assert!(d.lifecycle.from.is_none() && d.lifecycle.to.is_none());

    // Synthesise a state-change pair locally to exercise the populated path.
    let mut a = v1_0.revision.clone();
    let mut b = v1_0.revision.clone();
    a.state = LifecycleState::Released;
    b.state = LifecycleState::Deprecated;
    let d2 = diff_revisions(&a, &b);
    assert_eq!(d2.lifecycle.from, Some(LifecycleState::Released));
    assert_eq!(d2.lifecycle.to, Some(LifecycleState::Deprecated));
}

#[test]
fn supplier_diff_is_keyed_by_distributor_and_sku() {
    let v1_0 = load("r0805_v1_0.snxpart");
    let v1_1 = load("r0805_v1_1.snxpart");

    // v1.1 has the same suppliers as v1.0 in our fixture → no diff.
    let d = diff_revisions(&v1_0.revision, &v1_1.revision);
    assert!(d.suppliers.added.is_empty());
    assert!(d.suppliers.removed.is_empty());
}

// ---------------------------------------------------------------------------
// Fixture regeneration (run with `--ignored regenerate_fixtures`)
// ---------------------------------------------------------------------------

fn build_v1_0() -> SnxPartFile {
    let uuid = Uuid::parse_str(FIXTURE_UUID).unwrap();
    let mut shared = SharedSide {
        mpn: "RC0805FR-0710KL".into(),
        manufacturer: "Yageo".into(),
        description: "Resistor 10k 1% 0805".into(),
        suppliers: vec![SupplierLink {
            distributor: "DigiKey".into(),
            sku: "311-10.0KCRCT-ND".into(),
            url: None,
        }],
        ..Default::default()
    };
    shared
        .parameters
        .insert("value".into(), ParamValue::Text("10k".into()));
    shared
        .parameters
        .insert("tolerance".into(), ParamValue::Text("1%".into()));

    let mut rev = Revision {
        version: Version::new(1, 0),
        state: LifecycleState::Released,
        // Use a fixed timestamp so fixtures are byte-stable.
        created: chrono::DateTime::from_timestamp(1_745_625_600, 0).unwrap(),
        author: "fixture@signex".into(),
        message: "initial release".into(),
        schematic: SchematicSide {
            symbol: SymbolBody {
                sexpr: SYMBOL_2_PINS.into(),
            },
            ..Default::default()
        },
        pcb: PcbSide {
            footprint: FootprintBody {
                sexpr: FOOTPRINT_2_PADS.into(),
            },
            ..Default::default()
        },
        shared,
        content_hash: [0u8; 32],
    };
    rev.refresh_content_hash();
    SnxPartFile {
        schema: 1,
        uuid,
        internal_pn: InternalPn::new(FIXTURE_INTERNAL_PN),
        revision: rev,
    }
}

fn build_v1_1() -> SnxPartFile {
    // Identical to v1.0 except `shared.mpn` (a metadata-only minor bump).
    let mut f = build_v1_0();
    f.revision.version = Version::new(1, 1);
    f.revision.message = "supplier MPN reissued".into();
    f.revision.shared.mpn = "RC0805FR-0710KL_REISSUE".into();
    f.revision.refresh_content_hash();
    f
}

fn build_v2_0() -> SnxPartFile {
    // Adds a 3rd pin to the symbol — major-bump worthy.
    let mut f = build_v1_0();
    f.revision.version = Version::new(2, 0);
    f.revision.message = "kelvin sense pin added".into();
    f.revision.schematic.symbol.sexpr = SYMBOL_3_PINS.into();
    f.revision.refresh_content_hash();
    f
}

fn write_fixture(name: &str, file: &SnxPartFile) {
    let path = fixtures_dir().join(name);
    let bytes = serde_json::to_vec_pretty(file).unwrap();
    std::fs::create_dir_all(fixtures_dir()).unwrap();
    std::fs::write(&path, bytes).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

#[test]
#[ignore = "regenerates committed JSON fixtures; run on schema changes"]
fn regenerate_fixtures() {
    write_fixture("r0805_v1_0.snxpart", &build_v1_0());
    write_fixture("r0805_v1_1.snxpart", &build_v1_1());
    write_fixture("r0805_v2_0.snxpart", &build_v2_0());
    eprintln!("wrote 3 fixtures into {}", fixtures_dir().display());
}
