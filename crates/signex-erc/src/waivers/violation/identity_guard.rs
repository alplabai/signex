//! Guard: no two findings of one rule on one sheet may share a waiver
//! identity, or one waiver would hide both (and any later one of that shape).
//! There is no exception: even the two dangling ends of one wire are two.
//! Runs the full engine over every rule's fixture and over one sheet built to
//! trip many rules at once.

use std::collections::BTreeMap;

use signex_types::schematic::{LabelType, PinDirection, Point, SchematicSheet};
use uuid::Uuid;

use super::identity_objects;
use crate::context::{ErcChildSheet, ErcContext, ErcNet, ErcNetPin, ErcPin, ErcSymbol};
use crate::fixtures::{context_for, empty, label, symbol, wire};
use crate::{RuleKind, Violation, engine};

/// `(rule id, identity)` groups holding more than one finding.
fn shared_identities(ctx: &ErcContext) -> Vec<(String, Vec<String>, usize)> {
    let sheet = SchematicSheet::default();
    let mut groups: BTreeMap<(String, Vec<String>), usize> = BTreeMap::new();
    for v in engine::run_all(ctx).into_iter().map(Violation::from) {
        let identity = identity_objects(&v, &sheet);
        if identity.is_empty() {
            continue; // not waivable at all
        }
        *groups.entry((v.rule_id.to_string(), identity)).or_default() += 1;
    }
    groups
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|((rule, id), n)| (rule, id, n))
        .collect()
}

#[test]
fn every_rule_fixture_gives_each_finding_its_own_identity() {
    for kind in RuleKind::ALL {
        let shared = shared_identities(&context_for(kind));
        assert!(shared.is_empty(), "{kind:?}: {shared:?}");
    }
}

fn pin(number: &str, at: Point, direction: PinDirection, connected: bool) -> ErcPin {
    ErcPin {
        world_pos: at,
        local_pos: at,
        number: number.into(),
        electrical_type: direction,
        required: true,
        connected,
    }
}

fn net_pin(s: &ErcSymbol, p: &ErcPin) -> ErcNetPin {
    ErcNetPin {
        symbol: s.uuid,
        occurrence: signex_types::schematic::InstancePath::root(),
        reference: s.reference.clone(),
        number: p.number.clone(),
        position: p.world_pos,
        local_pos: p.local_pos,
        direction: p.electrical_type,
        is_power_flag: false,
    }
}

fn net(name: &str, pins: Vec<ErcNetPin>) -> ErcNet {
    ErcNet {
        name: name.into(),
        names: vec![name.into()],
        class: name.to_ascii_lowercase(),
        pin_types: pins.iter().map(|p| p.direction).collect(),
        pins,
        has_driver: true,
        has_pullup: false,
    }
}

/// One sheet with several distinct findings per object pair or object.
fn many_rules_at_once() -> ErcContext {
    let mut c = empty();
    let p = |x: f64, y: f64| Point::new(x, y);
    // U1 and U2: two output/output conflicts, on D0 and on VCC.
    let mut u1 = symbol("U1", p(0.0, 0.0));
    u1.pins = vec![
        pin("1", p(0.0, 0.0), PinDirection::Output, true),
        pin("2", p(0.0, 12.7), PinDirection::PowerOutput, true),
        pin("3", p(0.0, 25.4), PinDirection::Passive, false),
        pin("4", p(0.0, 38.1), PinDirection::Passive, false),
    ];
    let mut u2 = symbol("U2", p(25.4, 0.0));
    u2.pins = vec![
        pin("1", p(25.4, 0.0), PinDirection::Output, true),
        pin("2", p(25.4, 12.7), PinDirection::PowerOutput, true),
    ];
    c.wires.push(wire(p(0.0, 0.0), p(25.4, 0.0)));
    c.wires.push(wire(p(0.0, 12.7), p(25.4, 12.7)));
    c.labels.push(label("D0", p(12.7, 0.0), LabelType::Net));
    c.labels.push(label("VCC", p(12.7, 12.7), LabelType::Net));
    c.nets.push(net(
        "D0",
        vec![net_pin(&u1, &u1.pins[0]), net_pin(&u2, &u2.pins[0])],
    ));
    c.nets.push(net(
        "VCC",
        vec![net_pin(&u1, &u1.pins[1]), net_pin(&u2, &u2.pins[1])],
    ));
    // U3 and U4 touch at two pin pairs with no wire.
    let mut u3 = symbol("U3", p(50.8, 0.0));
    let mut u4 = symbol("U4", p(50.8, 0.0));
    for (n, y) in [("1", 0.0), ("2", 12.7)] {
        u3.pins
            .push(pin(n, p(50.8, y), PinDirection::Passive, false));
        u4.pins
            .push(pin(n, p(50.8, y), PinDirection::Passive, false));
    }
    // R1, placed twice (a reused sheet), shown as R7 and R107, with no
    // footprint, value or MPN.
    let mut r1 = symbol("R?", p(76.2, 0.0));
    r1.instance_designators = vec!["R7".into(), "R107".into()];
    // Two two-pin power ports, +3V3 and +5V, shorted on two separate nets.
    let mut p33 = symbol("#PWR1", p(127.0, 0.0));
    let mut p5 = symbol("#PWR2", p(152.4, 0.0));
    for (port, value) in [(&mut p33, "+3V3"), (&mut p5, "+5V")] {
        port.is_power = true;
        port.value = value.into();
    }
    for (n, y) in [("1", 0.0), ("2", 25.4)] {
        p33.pins
            .push(pin(n, p(127.0, y), PinDirection::PowerInput, true));
        p5.pins
            .push(pin(n, p(152.4, y), PinDirection::PowerInput, true));
        c.wires.push(wire(p(127.0, y), p(152.4, y)));
    }
    // U5: two unnumbered pins that share a name (the number falls back to
    // it), both unconnected.
    let mut u5 = symbol("U5", p(177.8, 0.0));
    u5.pins = vec![
        pin("NC", p(177.8, 0.0), PinDirection::Passive, false),
        pin("NC", p(177.8, 2.54), PinDirection::Passive, false),
    ];
    c.symbols.extend([u1, u2, u3, u4, r1, p33, p5, u5]);
    // A sheet symbol with no pins whose file did not load.
    c.child_sheets.push(ErcChildSheet {
        uuid: Uuid::new_v4(),
        name: "Missing".into(),
        filename: "missing.snxsch".into(),
        position: p(101.6, 0.0),
        pins: Vec::new(),
    });
    c
}

#[test]
fn a_sheet_tripping_many_rules_gives_each_finding_its_own_identity() {
    let c = many_rules_at_once();
    let found: Vec<RuleKind> = engine::run_all(&c).iter().map(|d| d.rule_kind).collect();
    // The fixture must actually produce the shapes it is here for.
    let count = |k: RuleKind| found.iter().filter(|f| **f == k).count();
    assert_eq!(count(RuleKind::PinTypeConflict), 2, "{found:?}");
    assert_eq!(count(RuleKind::PinsTouchNotConnected), 2, "{found:?}");
    assert!(count(RuleKind::UnusedPin) >= 2, "{found:?}");
    let r1 = engine::run_all(&c)
        .iter()
        .filter(|d| d.rule_kind == RuleKind::PartWithoutFootprint)
        .filter(|d| d.message.contains(" R7 ") || d.message.contains(" R107 "))
        .count();
    assert_eq!(r1, 2, "{found:?}");
    assert!(count(RuleKind::BadHierSheetPin) >= 2, "{found:?}");
    assert_eq!(count(RuleKind::PowerPortShort), 2, "{found:?}");
    let u5_unused = engine::run_all(&c)
        .iter()
        .filter(|d| d.rule_kind == RuleKind::UnusedPin && d.message.contains("U5"))
        .count();
    assert_eq!(u5_unused, 2, "{found:?}");
    let shared = shared_identities(&c);
    assert!(shared.is_empty(), "{shared:?}");
}

#[test]
fn a_waived_conflict_between_two_parts_does_not_hide_a_later_one() {
    let c = many_rules_at_once();
    let conflicts: Vec<Violation> = engine::run_all(&c)
        .into_iter()
        .map(Violation::from)
        .filter(|v| v.rule == RuleKind::PinTypeConflict)
        .collect();
    let sheet = SchematicSheet::default();
    let on = |net: &str| {
        let mut list: Vec<Violation> = conflicts
            .iter()
            .filter(|v| v.net.as_ref().is_some_and(|n| n.name == net))
            .cloned()
            .collect();
        super::stamp_keys(&mut list, &sheet);
        list
    };
    let d0 = on("D0");
    let key = d0[0].waiver_key.clone().expect("waivable");
    let waiver = super::new_waiver(&key, "D0", "bus by design", "me", "").expect("waiver");
    let later = super::split_waived(on("VCC"), &[waiver]);
    assert_eq!((later.kept.len(), later.waived.len()), (1, 0));
}

/// Every waivable `(rule id, identity)` of a run, sorted, duplicates kept.
fn identities(ctx: &ErcContext) -> Vec<(String, Vec<String>)> {
    let sheet = SchematicSheet::default();
    let mut out: Vec<(String, Vec<String>)> = engine::run_all(ctx)
        .into_iter()
        .map(Violation::from)
        .map(|v| (v.rule_id.to_string(), identity_objects(&v, &sheet)))
        .filter(|(_, id)| !id.is_empty())
        .collect();
    out.sort();
    out
}

/// The same sheet with its symbols (and each net's pins) in reverse order.
fn reversed(ctx: &ErcContext) -> ErcContext {
    let mut r = ctx.clone();
    r.symbols.reverse();
    for net in &mut r.nets {
        net.pins.reverse();
        net.pin_types.reverse();
    }
    r
}

#[test]
fn identities_do_not_depend_on_symbol_order() {
    let mut cases: Vec<(String, ErcContext)> = RuleKind::ALL
        .into_iter()
        .map(|k| (format!("{k:?}"), context_for(k)))
        .collect();
    cases.push(("many rules".into(), many_rules_at_once()));
    for (name, ctx) in cases {
        assert_eq!(identities(&ctx), identities(&reversed(&ctx)), "{name}");
    }
}

#[test]
fn re_annotating_a_single_placement_part_lapses_its_waiver() {
    use signex_types::schematic::Symbol;
    let id = Uuid::new_v4();
    let run = |reference: &str| {
        let mut c = empty();
        let mut s = symbol(reference, Point::new(25.4, 25.4));
        s.uuid = id;
        c.symbols.push(s);
        let sheet = SchematicSheet {
            symbols: vec![Symbol {
                uuid: id,
                reference: reference.into(),
                position: Point::new(25.4, 25.4),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut found: Vec<Violation> = engine::run_all(&c)
            .into_iter()
            .map(Violation::from)
            .filter(|v| v.rule == RuleKind::PartWithoutFootprint)
            .collect();
        super::stamp_keys(&mut found, &sheet);
        found
    };
    let before = run("R5");
    let key = before[0].waiver_key.clone().expect("waivable");
    let waiver = super::new_waiver(&key, "R5", "fitted later", "me", "").expect("waiver");
    let after = super::split_waived(run("R7"), &[waiver]);
    assert!(after.lapsed.contains(&0), "LAPSED, not unmatched");
    assert_eq!(after.kept.len(), 1);
}

/// A sheet holding `parts` (designator, unit) of one four-unit library part.
fn unit_sheet(id: u128, parts: &[(&str, u32)]) -> SchematicSheet {
    use signex_types::schematic::{LibSymbol, Symbol};
    let graphics: Vec<serde_json::Value> = (1..=4)
        .map(|u| serde_json::json!({"unit": u, "graphic": {"type": "polyline", "points": []}}))
        .collect();
    let lib: Option<LibSymbol> =
        serde_json::from_value(serde_json::json!({"id": "Amp:Quad", "graphics": graphics})).ok();
    let mut sheet = SchematicSheet {
        uuid: Uuid::from_u128(id),
        symbols: parts
            .iter()
            .enumerate()
            .map(|(i, (reference, unit))| Symbol {
                uuid: Uuid::from_u128(id * 100 + i as u128),
                lib_id: "Amp:Quad".into(),
                reference: (*reference).into(),
                unit: *unit,
                position: Point::new(25.4 * (i as f64 + 1.0), 25.4),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    if let Some(lib) = lib {
        sheet.lib_symbols.insert("Amp:Quad".into(), lib);
    }
    sheet
}

#[test]
fn the_project_units_pass_gives_each_finding_its_own_identity() {
    // U1 split across both sheets, U2 and U3 on one sheet each; every part
    // misses units, so several findings land on each sheet.
    let a = unit_sheet(1, &[("U1", 1), ("U2", 1), ("U2", 2), ("U3", 4)]);
    let b = unit_sheet(2, &[("U1", 3), ("U4", 2), ("U5", 1)]);
    let found = crate::project_units_not_placed(&[("a", &a), ("b", &b)]);
    assert!(found.len() >= 4, "{found:?}");
    for (key, sheet) in [("a", &a), ("b", &b)] {
        let mut ids: Vec<Vec<String>> = found
            .iter()
            .filter(|(k, _)| *k == key)
            .map(|(_, v)| identity_objects(v, sheet))
            .collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "sheet {key}: {found:?}");
    }
}
