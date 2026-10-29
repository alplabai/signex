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
    let id = Uuid::new_v4();
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
    let id = Uuid::new_v4();
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
    let id = Uuid::new_v4();
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
    let id = Uuid::new_v4();
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
    let id = Uuid::new_v4();
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
    let id = Uuid::new_v4();
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
    let id = Uuid::new_v4();
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
    let id = Uuid::new_v4();
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
    let (r1, c1) = (Uuid::new_v4(), Uuid::new_v4());
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
