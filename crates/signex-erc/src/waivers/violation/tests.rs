use super::*;
use crate::{RuleId, RuleKind, Severity};
use signex_types::schematic::{Point, Symbol};
use uuid::Uuid;

fn sheet_with(symbol: Symbol) -> SchematicSheet {
    SchematicSheet {
        symbols: vec![symbol],
        ..Default::default()
    }
}

fn part(uuid: Uuid) -> Symbol {
    Symbol {
        uuid,
        reference: "TP1".into(),
        position: Point::new(25.4, 25.4),
        ..Default::default()
    }
}

fn finding_on(uuid: Uuid, kind: RuleKind) -> Violation {
    let mut v = Violation::new(
        RuleId::builtin(kind),
        kind,
        Severity::Warning,
        "Pin on TP1 is not connected",
        Point::new(25.4, 25.4),
    );
    v.primary = Some(SelectedItem::new(uuid, SelectedKind::Symbol));
    v
}

fn waive(v: &Violation, sheet: &SchematicSheet) -> ErcWaiver {
    let key = waiver_key(v, sheet).expect("waivable");
    new_waiver(&key, &v.message, "test point", "A. Designer", "2026-10-29").expect("valid")
}

fn split(v: Violation, sheet: &SchematicSheet, waivers: &[ErcWaiver]) -> WaiverSplit {
    let mut list = vec![v];
    stamp_keys(&mut list, sheet);
    split_waived(list, waivers)
}

#[test]
fn a_waiver_hides_its_finding_and_says_which_waiver_did() {
    let id = Uuid::now_v7();
    let sheet = sheet_with(part(id));
    let v = finding_on(id, RuleKind::UnusedPin);
    let waivers = vec![waive(&v, &sheet)];
    let out = split(v, &sheet, &waivers);
    assert!(out.kept.is_empty());
    assert_eq!(out.waived.len(), 1);
    assert_eq!(out.waived[0].1, 0);
    assert!(out.lapsed.is_empty());
}

#[test]
fn editing_an_involved_object_lapses_the_waiver() {
    let id = Uuid::now_v7();
    let sheet = sheet_with(part(id));
    let v = finding_on(id, RuleKind::UnusedPin);
    let waivers = vec![waive(&v, &sheet)];
    let mut moved = part(id);
    moved.position = Point::new(50.8, 25.4);
    let out = split(v.clone(), &sheet_with(moved), &waivers);
    assert_eq!(out.kept.len(), 1, "the finding is back");
    assert!(out.lapsed.contains(&0));
    let mut revalued = part(id);
    revalued.value = "changed".into();
    assert_eq!(split(v, &sheet_with(revalued), &waivers).kept.len(), 1);
}

#[test]
fn a_deleted_object_or_another_rule_never_matches() {
    let id = Uuid::now_v7();
    let sheet = sheet_with(part(id));
    let v = finding_on(id, RuleKind::UnusedPin);
    let waivers = vec![waive(&v, &sheet)];
    let empty = SchematicSheet::default();
    let gone = split(v, &empty, &waivers);
    assert_eq!((gone.kept.len(), gone.lapsed.len()), (1, 0));
    let other = split(finding_on(id, RuleKind::OffGrid), &sheet, &waivers);
    assert_eq!(other.kept.len(), 1);
}

#[test]
fn the_fingerprint_does_not_depend_on_field_map_order() {
    let id = Uuid::now_v7();
    let (mut a, mut b) = (part(id), part(id));
    for i in 0..32 {
        a.fields.insert(format!("k{i}"), format!("v{i}"));
    }
    for i in (0..32).rev() {
        b.fields.insert(format!("k{i}"), format!("v{i}"));
    }
    let v = finding_on(id, RuleKind::UnusedPin);
    assert_eq!(
        waiver_key(&v, &sheet_with(a)),
        waiver_key(&v, &sheet_with(b))
    );
}

#[test]
fn reason_and_author_are_required_and_incomplete_waivers_hide_nothing() {
    let id = Uuid::now_v7();
    let sheet = sheet_with(part(id));
    let v = finding_on(id, RuleKind::UnusedPin);
    let key = waiver_key(&v, &sheet).expect("key");
    assert_eq!(
        new_waiver(&key, "", "  ", "me", ""),
        Err(WaiverError::MissingReason)
    );
    assert_eq!(
        new_waiver(&key, "", "why", " ", ""),
        Err(WaiverError::MissingAuthor)
    );
    let mut hand_edited = waive(&v, &sheet);
    hand_edited.reason.clear();
    assert_eq!(split(v, &sheet, &[hand_edited]).kept.len(), 1);
}

#[test]
fn a_finding_without_objects_is_not_waivable() {
    let v = Violation::new(
        RuleId::user("r"),
        RuleKind::UnusedPin,
        Severity::Info,
        "m",
        Point::new(0.0, 0.0),
    );
    assert!(waiver_key(&v, &SchematicSheet::default()).is_none());
}

#[test]
fn adding_a_waiver_replaces_one_for_the_same_finding() {
    let id = Uuid::now_v7();
    let sheet = sheet_with(part(id));
    let v = finding_on(id, RuleKind::UnusedPin);
    let first = waive(&v, &sheet);
    let mut second = first.clone();
    second.reason = "updated".into();
    let list = with_waiver(std::slice::from_ref(&first), second);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].reason, "updated");
    assert!(without_waiver(&list, &first.rule_id, &first.objects).is_empty());
}

fn key_of(v: &Violation, sheet: &SchematicSheet) -> WaiverKey {
    waiver_key(v, sheet).expect("key")
}

#[test]
fn a_waiver_survives_a_save_and_reload_of_the_sheet() {
    // 3.81 mm reads back from its stored 3_810_000 nm as 3.8099999999999996.
    let id = Uuid::now_v7();
    let mut symbol = part(id);
    symbol.position = Point::new(3.81, 1.27 * 7.0);
    symbol.rotation = 90.0;
    let sheet = sheet_with(symbol);
    let text = signex_types::format::SnxSchematic::new(sheet.clone())
        .write_string()
        .expect("save");
    let reloaded = signex_types::format::SnxSchematic::parse(&text)
        .expect("load")
        .sheet;
    assert_ne!(
        reloaded.symbols[0].position.x.to_bits(),
        sheet.symbols[0].position.x.to_bits(),
        "the probe must exercise a drifting coordinate"
    );
    for kind in [RuleKind::UnusedPin, RuleKind::PartWithoutFootprint] {
        let v = finding_on(id, kind);
        assert_eq!(key_of(&v, &sheet), key_of(&v, &reloaded), "{kind:?}");
    }
}

#[test]
fn a_field_rule_waiver_ignores_placement_but_not_the_field() {
    let id = Uuid::now_v7();
    let sheet = sheet_with(part(id));
    let v = finding_on(id, RuleKind::PartWithoutFootprint);
    let waivers = vec![waive(&v, &sheet)];
    let mut moved = part(id);
    moved.position = Point::new(50.8, 50.8);
    moved.rotation = 180.0;
    assert_eq!(
        split(v.clone(), &sheet_with(moved), &waivers).waived.len(),
        1
    );
    let mut fitted = part(id);
    fitted.footprint = "R_0603".into();
    let out = split(v, &sheet_with(fitted), &waivers);
    assert!(out.lapsed.contains(&0));
}

fn net_finding(pins: &[(Uuid, &str)]) -> Violation {
    let mut v = Violation::new(
        RuleId::builtin(RuleKind::SinglePinNet),
        RuleKind::SinglePinNet,
        Severity::Warning,
        "Net 'SENSE' connects only one pin",
        Point::new(0.0, 0.0),
    );
    v.primary = Some(SelectedItem::new(pins[0].0, SelectedKind::Symbol));
    v.net = Some(crate::NetFinding::from_pins(
        "SENSE".into(),
        pins.iter()
            .map(|(id, reference)| crate::NetFindingPin {
                symbol: *id,
                reference: (*reference).into(),
                number: "1".into(),
                position: Point::new(0.0, 0.0),
                direction: signex_types::schematic::PinDirection::Passive,
            })
            .collect(),
    ));
    v
}

#[test]
fn a_net_waiver_ignores_part_values_but_not_the_pin_set() {
    let (r1, c1) = (Uuid::now_v7(), Uuid::now_v7());
    let sheet = |cap_value: &str| SchematicSheet {
        symbols: vec![part(r1), {
            let mut c = part(c1);
            c.value = cap_value.into();
            c
        }],
        ..Default::default()
    };
    let v = net_finding(&[(r1, "R1"), (c1, "C1")]);
    let waivers = vec![waive(&v, &sheet("100n"))];
    assert_eq!(split(v, &sheet("1u"), &waivers).waived.len(), 1);
    let renamed = net_finding(&[(r1, "R1"), (c1, "C9")]);
    assert!(split(renamed, &sheet("100n"), &waivers).lapsed.contains(&0));
}

#[test]
fn a_part_joining_a_waived_net_lapses_the_waiver_rather_than_orphaning_it() {
    let (r1, c1, c2) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let sheet = SchematicSheet {
        symbols: vec![part(r1), part(c1), part(c2)],
        ..Default::default()
    };
    let v = net_finding(&[(r1, "R1"), (c1, "C1")]);
    let waiver = waive(&v, &sheet);
    assert_eq!(
        waiver.objects,
        vec![r1.to_string(), format!("net:SENSE@{}", sheet.uuid)]
    );
    let grown = net_finding(&[(r1, "R1"), (c1, "C1"), (c2, "C2")]);
    let out = split(grown, &sheet, &[waiver]);
    assert!(out.lapsed.contains(&0), "same identity, new fingerprint");
    assert_eq!(out.kept.len(), 1);
}

fn wire_sheet(wire: signex_types::schematic::Wire) -> SchematicSheet {
    SchematicSheet {
        wires: vec![wire],
        ..Default::default()
    }
}

#[test]
fn a_geometry_waiver_ignores_cosmetics_but_not_the_geometry() {
    use signex_types::schematic::Wire;
    let id = Uuid::now_v7();
    let wire = Wire {
        uuid: id,
        start: Point::new(0.0, 0.0),
        end: Point::new(12.7, 0.0),
        ..Default::default()
    };
    let mut v = Violation::new(
        RuleId::builtin(RuleKind::DanglingWire),
        RuleKind::DanglingWire,
        Severity::Warning,
        "Wire endpoint is not connected",
        Point::new(0.0, 0.0),
    );
    v.primary = Some(SelectedItem::new(id, SelectedKind::Wire));
    let waivers = vec![waive(&v, &wire_sheet(wire.clone()))];
    let thicker = Wire {
        stroke_width: 0.5,
        ..wire.clone()
    };
    assert_eq!(
        split(v.clone(), &wire_sheet(thicker), &waivers)
            .waived
            .len(),
        1
    );
    let moved = Wire {
        end: Point::new(25.4, 0.0),
        ..wire
    };
    assert!(split(v, &wire_sheet(moved), &waivers).lapsed.contains(&0));
}

/// A DSL net-level finding: no primary object, only the net.
fn dsl_net_finding(name: &str, pins: &[(Uuid, &str)]) -> Violation {
    let mut v = net_finding(pins);
    v.rule_id = RuleId::user("net_check");
    v.primary = None;
    if let Some(net) = v.net.as_mut() {
        net.name = name.into();
    }
    v
}

fn sheet_id(n: u128) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::from_u128(n),
        ..Default::default()
    }
}

#[test]
fn one_global_net_on_two_sheets_gets_two_identities() {
    let (a, b) = (sheet_id(1), sheet_id(2));
    let on_a = dsl_net_finding("VBUS", &[(Uuid::now_v7(), "R1")]);
    let on_b = dsl_net_finding("VBUS", &[(Uuid::now_v7(), "R9")]);
    let waiver_a = waive(&on_a, &a);
    assert_ne!(key_of(&on_a, &a).objects, key_of(&on_b, &b).objects);
    // B's run against A's waiver: B's finding stays, neither waived nor lapsed.
    let out = split(on_b.clone(), &b, std::slice::from_ref(&waiver_a));
    assert_eq!(
        (out.kept.len(), out.waived.len(), out.lapsed.len()),
        (1, 0, 0)
    );
    // Waiving B keeps A's waiver.
    let both = with_waiver(std::slice::from_ref(&waiver_a), waive(&on_b, &b));
    assert_eq!(both.len(), 2);
    assert_eq!(split(on_a, &a, &both).waived.len(), 1);
    assert_eq!(split(on_b, &b, &both).waived.len(), 1);
}

#[test]
fn an_unnamed_net_without_an_object_is_waived_by_its_pin_set() {
    let sheet = sheet_id(1);
    let (r1, c1) = (Uuid::now_v7(), Uuid::now_v7());
    let v = dsl_net_finding("", &[(r1, "R1"), (c1, "C1")]);
    let key = key_of(&v, &sheet);
    assert_eq!(key.objects, vec![format!("pins:C1.1,R1.1@{}", sheet.uuid)]);
    let waivers = vec![waive(&v, &sheet)];
    assert_eq!(split(v, &sheet, &waivers).waived.len(), 1);
    // A membership change on an unnamed net orphans the waiver.
    let grown = dsl_net_finding("", &[(r1, "R1"), (c1, "C1"), (Uuid::now_v7(), "C2")]);
    let out = split(grown, &sheet, &waivers);
    assert_eq!((out.kept.len(), out.lapsed.len()), (1, 0));
}

#[test]
fn a_sheet_pin_finding_is_waivable() {
    use signex_types::schematic::{ChildSheet, FillType, SheetPin};
    let pin_id = Uuid::now_v7();
    let sheet = SchematicSheet {
        child_sheets: vec![ChildSheet {
            uuid: Uuid::now_v7(),
            name: "Driver".into(),
            filename: "driver.snxsch".into(),
            position: Point::new(0.0, 0.0),
            size: (10.0, 10.0),
            stroke_width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![SheetPin {
                uuid: pin_id,
                name: "IN".into(),
                direction: "input".into(),
                position: Point::new(0.0, 2.54),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            }],
            instances: Vec::new(),
        }],
        ..Default::default()
    };
    let mut v = finding_on(pin_id, RuleKind::SheetPinUnconnected);
    v.primary = Some(SelectedItem::new(pin_id, SelectedKind::SheetPin));
    let waivers = vec![waive(&v, &sheet)];
    let out = split(v, &sheet, &waivers);
    assert!(out.kept.is_empty(), "the waiver hides the finding");
    assert_eq!(out.waived.len(), 1);
}
