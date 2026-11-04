//! The per-field three-way decision (#2004 item 7). One test per row of the
//! table in `docs/architecture/snxsch-library-link.md`, "Value stamp".

use super::*;
use uuid::Uuid;

fn value(t: &str) -> Theirs<'_> {
    Theirs::Value(t)
}

// Row 1
#[test]
fn an_unreadable_row_value_is_unknown_and_never_flags() {
    for base in [None, Some("A"), Some("")] {
        for ours in ["", "A", "B"] {
            let v = resolve(base, ours, Theirs::Unreadable);
            assert_eq!(v, Verdict::Unknown);
            assert!(!v.is_flagged());
            assert_eq!(v.base_after(base, true), base.map(str::to_owned));
        }
    }
}

// Row 2
#[test]
fn a_row_that_offers_nothing_never_clears_the_part() {
    let v = resolve(Some("A"), "B", value(""));
    assert_eq!(v, Verdict::RowOffersNothing);
    assert!(!v.is_flagged());
    assert_eq!(v.base_after(Some("A"), true), Some("A".into()));
    assert_eq!(resolve(None, "B", value("  ")), Verdict::RowOffersNothing);
}

// Row 3
#[test]
fn a_part_holding_the_rows_value_is_in_sync_and_takes_it_as_base() {
    for base in [None, Some("A"), Some("B")] {
        let v = resolve(base, "B", value("B"));
        assert_eq!(v, Verdict::InSync("B".into()));
        assert!(!v.is_flagged());
        assert_eq!(v.base_after(base, false), Some("B".into()));
    }
    // Typing the row's new value by hand is not a conflict.
    assert_eq!(
        resolve(Some("A"), "C", value("C")),
        Verdict::InSync("C".into())
    );
    // Both empty is in sync too.
    assert_eq!(
        resolve(Some("A"), "", value("")),
        Verdict::InSync(String::new())
    );
}

// Row 4
#[test]
fn an_untouched_part_follows_a_row_that_moved() {
    let v = resolve(Some("A"), "A", value("B"));
    assert_eq!(v, Verdict::Follows("B".into()));
    assert!(v.is_flagged());
    assert_eq!(v.follows(), Some("B"));
    assert_eq!(v.base_after(Some("A"), true), Some("B".into()), "applied");
    assert_eq!(
        v.base_after(Some("A"), false),
        Some("A".into()),
        "a pending follow keeps its base"
    );
}

// Row 5
#[test]
fn an_override_of_a_row_that_did_not_move_is_kept_and_not_flagged() {
    let v = resolve(Some("A"), "X", value("A"));
    assert_eq!(v, Verdict::Override);
    assert!(!v.is_flagged());
    assert_eq!(v.base_after(Some("A"), true), Some("A".into()));
}

// Row 6
#[test]
fn an_override_of_a_row_that_moved_is_a_conflict_until_answered() {
    let v = resolve(Some("A"), "X", value("B"));
    assert_eq!(v, Verdict::Conflict("B".into()));
    assert!(v.is_flagged());
    assert_eq!(v.follows(), None, "an update never applies a conflict");
    assert_eq!(v.base_after(Some("A"), false), Some("A".into()));
    assert_eq!(v.base_after(Some("A"), true), Some("B".into()), "answered");
}

// Row 7
#[test]
fn a_cleared_field_stays_empty_and_flags_only_when_the_row_moved() {
    assert_eq!(resolve(Some("A"), "", value("A")), Verdict::Override);
    assert_eq!(
        resolve(Some("A"), "", value("B")),
        Verdict::Conflict("B".into())
    );
}

// Row 8
#[test]
fn an_empty_legacy_part_is_filled_from_the_row() {
    let v = resolve(None, "", value("B"));
    assert_eq!(v, Verdict::Follows("B".into()));
    assert_eq!(v.base_after(None, true), Some("B".into()));
    assert_eq!(v.base_after(None, false), None);
}

// Row 9
#[test]
fn a_legacy_part_with_a_different_value_is_a_conflict_never_applied() {
    let v = resolve(None, "X", value("B"));
    assert_eq!(v, Verdict::Conflict("B".into()));
    assert_eq!(v.follows(), None);
    assert_eq!(v.base_after(None, false), None, "the base stays unknown");
}

#[test]
fn an_absent_stamp_key_is_unknown_and_not_inferred_from_the_version() {
    let mut stamp = RowStamp::new(Uuid::nil());
    stamp.mpn = Some("A".into());
    // Version unmoved would derive a base for a part with no stamp ...
    assert_eq!(
        base_of(None, StampField::Footprint, true, value("F")),
        Some("F")
    );
    // ... but a stamp that lacks the key leaves it unknown.
    assert_eq!(
        base_of(Some(&stamp), StampField::Footprint, true, value("F")),
        None
    );
    assert_eq!(
        base_of(Some(&stamp), StampField::Mpn, true, value("Z")),
        Some("A")
    );
    assert_eq!(base_of(None, StampField::Mpn, false, value("Z")), None);
    assert_eq!(
        base_of(None, StampField::Mpn, true, Theirs::Unreadable),
        None
    );
}

#[test]
fn a_stamp_of_another_row_is_no_stamp_and_a_link_stamp_writes_version_and_values_together() {
    let row = Uuid::from_u128(7);
    let mut part = Symbol {
        row_id: Some(row),
        library_stamp: Some(RowStamp::new(Uuid::from_u128(8))),
        ..Default::default()
    };
    assert!(part.valid_stamp().is_none());
    let link = LinkStamp {
        version: "1.0.1".into(),
        values: StampValues {
            mpn: Some("X".into()),
            ..Default::default()
        },
    };
    assert!(part.apply_link_stamp(&link));
    assert_eq!(part.library_version, "1.0.1");
    let stamp = part.valid_stamp().expect("replaced by one of this row");
    assert_eq!((stamp.row_id, stamp.mpn.as_deref()), (row, Some("X")));
    assert_eq!(stamp.footprint, None, "only what is proven is written");
    assert!(!part.apply_link_stamp(&link), "nothing left to change");
    // A part with no row keeps no stamp.
    let mut free = Symbol::default();
    assert!(free.apply_link_stamp(&link));
    assert!(free.library_stamp.is_none());
}
