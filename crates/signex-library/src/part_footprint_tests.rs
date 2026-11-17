use std::collections::HashMap;

use signex_types::part::FootprintResolveError;
use uuid::Uuid;

use super::*;
use crate::component::tests::fixture_row;
use crate::footprint_binding::FootprintBinding;
use crate::primitive::footprint::{LayerId, Pad, PadKind, PadShape};

fn library_footprint(name: &str, version: &str) -> Footprint {
    let mut fp = Footprint::empty(name);
    fp.version = version.into();
    fp.pads = vec![Pad {
        number: "1".into(),
        kind: PadKind::Smd,
        shape: PadShape::Rect,
        size: [1.0, 1.0],
        layers: vec![LayerId::new("F.Cu")],
        ..Pad::default()
    }];
    fp
}

/// A row with a default footprint (pin map 1 -> A1, 2 -> B1) and one
/// alternate (pin map 1 -> K, version pinned to 2.0.0), and a library that
/// holds both.
fn fixture() -> (ComponentRow, PrimitiveRef, HashMap<PrimitiveRef, Footprint>) {
    let mut row = fixture_row();
    row.pin_map_overrides = vec![
        PinPadOverride::new("1", "A1"),
        PinPadOverride::new("2", "B1"),
    ];
    let alternate = PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7());
    row.alternate_footprints = vec![FootprintBinding {
        footprint: alternate,
        version: "2.0.0".into(),
        pin_map_overrides: vec![PinPadOverride::new("1", "K")],
    }];
    let library = HashMap::from([
        (
            row.footprint_ref.unwrap(),
            library_footprint("R0805", "1.4.0"),
        ),
        (alternate, library_footprint("R0805-THT", "2.0.0")),
    ]);
    (row, alternate, library)
}

fn lookup(
    library: &HashMap<PrimitiveRef, Footprint>,
) -> impl Fn(&PrimitiveRef, &str) -> Option<Footprint> + '_ {
    |address, _version| library.get(address).cloned()
}

#[test]
fn the_default_binding_uses_the_row_pin_map_and_records_its_source() {
    let (row, _, library) = fixture();
    let default = row.footprint_ref.unwrap();
    let resolved = resolve_part_footprint(&row, None, lookup(&library)).unwrap();
    let source = FootprintSourceRef {
        library: default.library_id,
        footprint: default.uuid,
        version: "1.4.0".into(),
    };
    assert_eq!(resolved.source, source);
    assert_eq!(resolved.footprint.library_ref, Some(source));
    assert_eq!(resolved.footprint.footprint_id, "R0805");
    assert_eq!(resolved.pin_map.pad_for("1"), "A1");
    assert_eq!(resolved.pin_map.pad_for("2"), "B1");
    assert_eq!(resolved.pin_map.pad_for("3"), "3", "unlisted pins are 1:1");
    let named = resolve_part_footprint(&row, Some(&default), lookup(&library)).unwrap();
    assert_eq!(
        named.source, resolved.source,
        "naming the default is the default"
    );
}

#[test]
fn an_alternate_pin_map_takes_priority_over_the_row_pin_map() {
    let (row, alternate, library) = fixture();
    let resolved = resolve_part_footprint(&row, Some(&alternate), lookup(&library)).unwrap();
    assert_eq!(resolved.footprint.footprint_id, "R0805-THT");
    assert_eq!(resolved.source.footprint, alternate.uuid);
    assert_eq!(resolved.source.version, "2.0.0");
    assert_eq!(resolved.pin_map.pad_for("1"), "K", "the alternate wins");
    assert_eq!(
        resolved.pin_map.pad_for("2"),
        "B1",
        "the row fills the rest"
    );
}

#[test]
fn the_pinned_version_reaches_the_lookup_and_a_mismatch_is_refused() {
    let (row, alternate, mut library) = fixture();
    let asked = std::cell::RefCell::new(Vec::new());
    let recording = |address: &PrimitiveRef, version: &str| {
        asked.borrow_mut().push(version.to_string());
        library.get(address).cloned()
    };
    resolve_part_footprint(&row, Some(&alternate), recording).unwrap();
    assert_eq!(*asked.borrow(), ["2.0.0"]);

    library.get_mut(&alternate).unwrap().version = "2.1.0".into();
    assert_eq!(
        resolve_part_footprint(&row, Some(&alternate), lookup(&library)),
        Err(FootprintResolveError::VersionMismatch {
            wanted: "2.0.0".into(),
            found: "2.1.0".into()
        })
    );
}

#[test]
fn a_row_without_a_footprint_an_unoffered_one_and_a_missing_one_are_refused() {
    let (mut row, alternate, library) = fixture();
    let stranger = PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7());
    assert_eq!(
        resolve_part_footprint(&row, Some(&stranger), lookup(&library)),
        Err(FootprintResolveError::NotOffered {
            library: stranger.library_id,
            footprint: stranger.uuid
        })
    );
    assert_eq!(
        resolve_part_footprint(&row, Some(&alternate), |_, _| None),
        Err(FootprintResolveError::NotFound {
            library: alternate.library_id,
            footprint: alternate.uuid,
            version: "2.0.0".into()
        })
    );
    row.footprint_ref = None;
    row.alternate_footprints.clear();
    assert_eq!(
        resolve_part_footprint(&row, None, lookup(&library)),
        Err(FootprintResolveError::NoFootprint)
    );
}

/// Maintainer decision (Caner, 2026-11-09): a footprint with a 3D body and a
/// STEP model converts, and the board reaches the model through `library_ref`.
#[test]
fn a_footprint_with_a_3d_model_resolves_and_keeps_its_library_ref() {
    use crate::primitive::footprint::StepAttachment;
    let (row, _, mut library) = fixture();
    let default = row.footprint_ref.unwrap();
    let fp = library.get_mut(&default).unwrap();
    fp.body_3d.height_mm = 1.1;
    fp.step_attachment = Some(StepAttachment {
        content_hash: "cd".into(),
        filename: "R0805.step".into(),
        offset_xyz: [0.0; 3],
        rotation_xyz: [0.0, 0.0, 90.0],
    });
    let resolved = resolve_part_footprint(&row, None, lookup(&library)).unwrap();
    let source = FootprintSourceRef {
        library: default.library_id,
        footprint: default.uuid,
        version: "1.4.0".into(),
    };
    assert_eq!(resolved.footprint.library_ref, Some(source.clone()));
    assert_eq!(resolved.source, source);
}

#[test]
fn a_library_footprint_with_an_empty_version_is_refused() {
    let (row, _, mut library) = fixture();
    let default = row.footprint_ref.unwrap();
    library.get_mut(&default).unwrap().version = String::new();
    assert_eq!(
        resolve_part_footprint(&row, None, lookup(&library)),
        Err(FootprintResolveError::Unconvertible {
            what: "library footprint identity",
            value: format!("{}:{}@", default.library_id, default.uuid),
        })
    );
}

#[test]
fn a_footprint_the_board_cannot_hold_is_refused_through_the_resolver() {
    let (row, _, mut library) = fixture();
    let default = row.footprint_ref.unwrap();
    library.get_mut(&default).unwrap().pads[0].layers = vec![LayerId::new("Dwgs.User")];
    assert_eq!(
        resolve_part_footprint(&row, None, lookup(&library)),
        Err(FootprintResolveError::Unconvertible {
            what: "footprint layer",
            value: "Dwgs.User".into()
        })
    );
}

/// A placed unit of `row` from library `library`, stamped at version `1.0.0`
/// with `binding`; `stamp_version` is the version its stamp was taken at.
fn placed(
    row: &ComponentRow,
    library: Uuid,
    binding: Option<&PrimitiveRef>,
    stamp_version: &str,
) -> signex_types::schematic::Symbol {
    signex_types::schematic::Symbol {
        library_id: Some(library),
        row_id: Some(row.row_id),
        library_version: "1.0.0".into(),
        library_stamp: Some(signex_types::schematic::RowStamp {
            row_id: row.row_id,
            version: Some(stamp_version.into()),
            footprint_ref: binding.map(|b| format!("{}:{}", b.library_id, b.uuid)),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn a_symbol_resolves_through_its_row_and_the_binding_its_valid_stamp_names() {
    let (row, alternate, library) = fixture();
    let lib = Uuid::now_v7();
    let find = |l: Uuid, r: Uuid| (l == lib && r == row.row_id).then_some(&row);

    let symbol = placed(&row, lib, Some(&alternate), "1.0.0");
    assert_eq!(chosen_footprint(&symbol), Some(alternate));
    let resolved = resolve_symbol_footprint(&symbol, find, lookup(&library)).unwrap();
    assert_eq!(resolved.source.footprint, alternate.uuid);

    // A stamp taken at another version is not trusted: the row default.
    let stale = placed(&row, lib, Some(&alternate), "0.9.0");
    assert_eq!(chosen_footprint(&stale), None);
    assert_eq!(
        stamped_footprint(stale.library_stamp.as_ref().unwrap()),
        Some(alternate),
        "the stamp itself still records the binding"
    );
    let resolved = resolve_symbol_footprint(&stale, find, lookup(&library)).unwrap();
    assert_eq!(resolved.source.footprint, row.footprint_ref.unwrap().uuid);
}

#[test]
fn a_symbol_with_no_row_uses_the_footprint_it_names_else_has_none() {
    let (row, alternate, library) = fixture();
    let no_row = |_: Uuid, _: Uuid| None::<&ComponentRow>;

    let mut symbol = placed(&row, Uuid::now_v7(), None, "1.0.0");
    assert_eq!(
        resolve_symbol_footprint(&symbol, no_row, lookup(&library)),
        Err(FootprintResolveError::RowNotFound {
            library: symbol.library_id.unwrap(),
            row: row.row_id,
        })
    );
    symbol.footprint = format!(" {}:{} ", alternate.library_id, alternate.uuid);
    let named = resolve_symbol_footprint(&symbol, no_row, lookup(&library)).unwrap();
    assert_eq!(named.source.footprint, alternate.uuid);
    assert_eq!(named.pin_map.pad_for("1"), "1", "a named footprint is 1:1");

    let bare = signex_types::schematic::Symbol::default();
    assert_eq!(
        resolve_symbol_footprint(&bare, no_row, lookup(&library)),
        Err(FootprintResolveError::NoFootprint)
    );
}
