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

#[test]
fn a_link_stamp_with_no_values_still_stamps_a_legacy_part_so_the_legacy_rule_is_off() {
    let row = Uuid::from_u128(7);
    let mut legacy = Symbol {
        row_id: Some(row),
        library_version: "1.0.0".into(),
        ..Default::default()
    };
    assert!(legacy.valid_stamp().is_none());
    assert!(legacy.apply_link_stamp(&LinkStamp::version("1.0.1")));
    let stamp = legacy.valid_stamp().expect("an empty stamp is written");
    assert!(stamp.footprint.is_none() && stamp.manufacturer.is_none() && stamp.mpn.is_none());
    assert!(!legacy.apply_link_stamp(&LinkStamp::version("1.0.1")));
}

/// The base a part's MPN has against a row at `row_version` holding `theirs`.
fn mpn_base<'a>(part: &'a Symbol, row_version: &str, theirs: &'a str) -> Option<&'a str> {
    let unmoved = !part.library_version.is_empty() && part.library_version == row_version;
    base_of(part.valid_stamp(), StampField::Mpn, unmoved, value(theirs))
}

fn stamped_part(stamp_version: Option<&str>, library_version: &str) -> Symbol {
    let row = Uuid::from_u128(7);
    Symbol {
        row_id: Some(row),
        library_version: library_version.into(),
        library_stamp: Some(RowStamp {
            mpn: Some("A".into()),
            version: stamp_version.map(str::to_owned),
            ..RowStamp::new(row)
        }),
        ..Default::default()
    }
}

/// An older build moved the part to the row's value B at version 2 and left
/// the stamp (A, taken at version 1) behind. The stale stamp is not a base.
#[test]
fn a_stamp_taken_at_another_version_is_not_trusted() {
    let part = stamped_part(Some("1"), "2");
    assert!(part.valid_stamp().is_none());
    // The part holds A, deliberately set back; the row is at B, version 2.
    // The legacy rule reads the unmoved version: an override, kept and
    // unflagged. The stale stamp would have read as "follows" and let an
    // update overwrite A.
    let base = mpn_base(&part, "2", "B");
    assert_eq!(resolve(base, "A", value("B")), Verdict::Override);
    // The part holds C, row B: an override too, not a spurious conflict.
    assert_eq!(resolve(base, "C", value("B")), Verdict::Override);
}

/// Version moved in the row and the stamp matches the part's version: the
/// stamp is the base and the conflict is real.
#[test]
fn a_stamp_taken_at_the_parts_version_is_the_base() {
    let part = stamped_part(Some("1"), "1");
    assert!(part.valid_stamp().is_some());
    assert_eq!(
        resolve(mpn_base(&part, "2", "B"), "X", value("B")),
        Verdict::Conflict("B".into())
    );
    assert_eq!(
        resolve(mpn_base(&part, "2", "B"), "A", value("B")),
        Verdict::Follows("B".into())
    );
}

#[test]
fn a_stamp_with_no_version_is_not_trusted() {
    assert!(stamped_part(None, "1").valid_stamp().is_none());
}

#[test]
fn a_link_stamp_tags_the_stamp_with_its_version_and_keeps_the_values_of_a_valid_one() {
    let mut part = stamped_part(Some("1"), "1");
    assert!(part.apply_link_stamp(&LinkStamp::version("2")));
    let stamp = part.valid_stamp().expect("retagged with the new version");
    assert_eq!(stamp.mpn.as_deref(), Some("A"), "the base is kept");
    // A stale stamp is replaced, not retagged: its values are not a base.
    let mut stale = stamped_part(Some("1"), "2");
    assert!(stale.apply_link_stamp(&LinkStamp::version("3")));
    assert_eq!(stale.valid_stamp().unwrap().mpn, None);
}

#[test]
fn building_a_part_with_a_footprint_binding_stamps_the_name_and_the_choice() {
    let row = Uuid::from_u128(7);
    let mut part = Symbol {
        row_id: Some(row),
        library_version: "1".into(),
        footprint: "Chip-0805".into(),
        library_stamp: Some(RowStamp {
            footprint: Some("Chip-0805".into()),
            mpn: Some("A".into()),
            version: Some("1".into()),
            ..RowStamp::new(row)
        }),
        ..Default::default()
    };
    part.apply_footprint_binding("Chip-0402", Some("lib:fp".into()));
    assert_eq!(part.footprint, "Chip-0402");
    let stamp = part.valid_stamp().expect("the stamp stays valid");
    assert_eq!(stamp.footprint.as_deref(), Some("Chip-0402"));
    assert_eq!(stamp.footprint_ref.as_deref(), Some("lib:fp"));
    assert_eq!(stamp.mpn.as_deref(), Some("A"), "other bases are kept");

    // Back to the default: the choice is gone, the base follows the name.
    part.apply_footprint_binding("Chip-0805", None);
    let stamp = part.valid_stamp().unwrap();
    assert_eq!(stamp.footprint.as_deref(), Some("Chip-0805"));
    assert_eq!(stamp.footprint_ref, None);
}

#[test]
fn a_part_with_no_row_gets_the_footprint_and_no_stamp() {
    let mut part = Symbol::default();
    part.apply_footprint_binding("Chip-0402", Some("lib:fp".into()));
    assert_eq!(part.footprint, "Chip-0402");
    assert!(part.library_stamp.is_none());
}

#[test]
fn a_link_stamp_that_resets_the_footprint_binding_drops_the_recorded_alternate() {
    let row = Uuid::from_u128(7);
    let mut part = Symbol {
        row_id: Some(row),
        library_version: "1".into(),
        library_stamp: Some(RowStamp {
            footprint: Some("Chip-0402".into()),
            footprint_ref: Some("lib:fp".into()),
            version: Some("1".into()),
            ..RowStamp::new(row)
        }),
        ..Default::default()
    };
    // A plain footprint update keeps the recorded binding...
    let mut link = LinkStamp {
        version: "2".into(),
        values: StampValues {
            footprint: Some("Chip-0805".into()),
            ..StampValues::default()
        },
    };
    part.apply_link_stamp(&link);
    assert_eq!(
        part.library_stamp
            .as_ref()
            .unwrap()
            .footprint_ref
            .as_deref(),
        Some("lib:fp")
    );
    // ...and the reset forgets it.
    link.values.reset_footprint_binding = true;
    part.apply_link_stamp(&link);
    let stamp = part.library_stamp.as_ref().unwrap();
    assert_eq!(stamp.footprint_ref, None);
    assert_eq!(stamp.footprint.as_deref(), Some("Chip-0805"));
}

#[test]
fn a_reset_without_a_footprint_value_keeps_the_recorded_binding() {
    let row = Uuid::from_u128(7);
    let mut part = Symbol {
        row_id: Some(row),
        library_version: "1".into(),
        library_stamp: Some(RowStamp {
            footprint_ref: Some("lib:fp".into()),
            version: Some("1".into()),
            ..RowStamp::new(row)
        }),
        ..Default::default()
    };
    let link = LinkStamp {
        version: "2".into(),
        values: StampValues {
            reset_footprint_binding: true,
            ..StampValues::default()
        },
    };
    part.apply_link_stamp(&link);
    let stamp = part.library_stamp.as_ref().unwrap();
    assert_eq!(stamp.footprint_ref.as_deref(), Some("lib:fp"));
}
