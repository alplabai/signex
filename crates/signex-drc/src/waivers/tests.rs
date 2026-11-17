//! #2307: DRC waivers match by rule and items and lapse on a geometry change.

use std::collections::BTreeMap;

use pcb_geom::{ItemRef, PointNm};
use serde_json::Map;
use signex_types::check::CheckReport;
use signex_types::pcb::{FpGraphic, NetDef, PadType, PcbBoard, Point, ViaType};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_sketch::{BoardSketch, PcbKeepout};
use signex_types::project::DrcWaiver;
use signex_types::sketch::attr::KeepoutKinds;

use super::*;
use crate::findings::check_report;
use crate::tests::{board, footprint, id, pad, track, via};
use crate::{DrcRules, ViolationKind, run};

const RULE: i64 = 200_000;

/// Tracks 1 (GND) and 2 (VCC) 0.4 mm apart, a clearance finding, and an
/// unrelated GND track 3 far away.
fn pair() -> PcbBoard {
    let mut b = board(
        vec![],
        vec![
            track(1, (0.0, 0.0), (10.0, 0.0), 1, "TopCopper"),
            track(2, (0.0, 0.4), (10.0, 0.4), 2, "TopCopper"),
            track(3, (0.0, 20.0), (10.0, 20.0), 1, "TopCopper"),
        ],
        vec![],
    );
    b.nets = vec![
        NetDef {
            number: 1,
            name: "GND".into(),
        },
        NetDef {
            number: 2,
            name: "VCC".into(),
        },
    ];
    b
}

fn only_key(b: &PcbBoard) -> WaiverKey {
    let report = run(b, &DrcRules::new(RULE));
    assert_eq!(report.violations.len(), 1, "{report:?}");
    waiver_key(&report.violations[0], b).expect("a clearance finding is waivable")
}

fn waived(key: &WaiverKey) -> DrcWaiver {
    DrcWaiver {
        rule_id: key.rule_id.clone(),
        objects: key.objects.clone(),
        fingerprint: key.fingerprint.clone(),
        reason: "approved".into(),
        author: "A. Designer".into(),
        ..Default::default()
    }
}

fn shift(p: &mut Point, dy: f64) {
    p.y += dy;
}

fn violation(items: Vec<ItemRef>, keepout: Option<uuid::Uuid>) -> Violation {
    Violation {
        kind: ViolationKind::Keepout,
        items,
        nets: (0, 0),
        layers: 1,
        at: PointNm::new(0, 0),
        measured_nm: 0,
        required_nm: 0,
        keepout,
        rule: None,
    }
}

#[test]
fn the_key_names_the_rule_and_the_items_sorted() {
    let key = only_key(&pair());
    assert_eq!(key.rule_id, "builtin::clearance");
    assert_eq!(key.objects, [id(2001).to_string(), id(2002).to_string()]);
    assert_eq!(key.fingerprint.len(), 64);
}

#[test]
fn moving_an_unrelated_item_keeps_the_fingerprint() {
    let before = only_key(&pair());
    let mut b = pair();
    shift(&mut b.segments[2].start, 1.0);
    shift(&mut b.segments[2].end, 1.0);
    b.segments[2].width = 0.5;
    assert_eq!(only_key(&b), before);
}

#[test]
fn moving_resizing_or_renetting_an_involved_item_changes_the_fingerprint() {
    let before = only_key(&pair());
    type Edit = fn(&mut PcbBoard);
    let edits: [(&str, Edit); 4] = [
        ("move", |b| shift(&mut b.segments[1].end, -0.05)),
        ("width", |b| b.segments[0].width = 0.2),
        ("layer", |b| b.segments[0].layer = "BottomCopper".into()),
        ("net name", |b| b.nets[1].name = "VBUS".into()),
    ];
    for (what, edit) in edits {
        let mut b = pair();
        edit(&mut b);
        // A layer change can clear the finding; key the original violation.
        let v = &run(&pair(), &DrcRules::new(RULE)).violations[0];
        let key = waiver_key(v, &b).expect(what);
        assert_eq!(key.objects, before.objects, "{what}");
        assert_ne!(key.fingerprint, before.fingerprint, "{what}");
    }
}

#[test]
fn renumbering_the_nets_keeps_the_fingerprint() {
    let before = only_key(&pair());
    let mut b = pair();
    b.nets[1].number = 7;
    b.segments[1].net = 7;
    assert_eq!(only_key(&b).fingerprint, before.fingerprint);
}

#[test]
fn a_pad_fingerprint_follows_its_footprint_and_its_own_geometry() {
    let fp = footprint(1, vec![pad(5, (1.0, 0.0), 1, PadType::Smd, "TopCopper")]);
    let b = board(vec![fp], vec![], vec![]);
    let v = violation(
        vec![ItemRef::Pad {
            footprint: id(1001),
            pad: id(5),
        }],
        None,
    );
    let before = waiver_key(&v, &b).unwrap();
    assert_eq!(before.objects, [format!("{}/{}", id(1001), id(5))]);
    let mut moved = b.clone();
    shift(&mut moved.footprints[0].position, 1.0);
    let mut turned = b.clone();
    turned.footprints[0].rotation = 90.0;
    let mut grown = b.clone();
    grown.footprints[0].pads[0].size = Point::new(2.0, 1.0);
    let mut renamed = b.clone();
    renamed.footprints[0].reference = "U9".into();
    for (what, edited, lapses) in [
        ("footprint moved", moved, true),
        ("footprint turned", turned, true),
        ("pad grown", grown, true),
        ("designator renamed", renamed, false),
    ] {
        let key = waiver_key(&v, &edited).unwrap();
        assert_eq!(key.fingerprint != before.fingerprint, lapses, "{what}");
    }
}

#[test]
fn a_keepout_is_part_of_the_identity_and_its_geometry_of_the_fingerprint() {
    let mut b = board(
        vec![],
        vec![track(1, (15.0, 25.0), (35.0, 25.0), 0, "TopCopper")],
        vec![],
    );
    let corners = [(20.0, 20.0), (30.0, 20.0), (30.0, 30.0), (20.0, 30.0)];
    b.sketch = Some(BoardSketch {
        keepouts: vec![PcbKeepout {
            uuid: id(0x900),
            boundary: corners.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            layers: vec![PcbLayer::Copper(CopperLayer::Top)],
            forbids: KeepoutKinds {
                no_routing: true,
                ..KeepoutKinds::default()
            },
        }],
        ..BoardSketch::default()
    });
    let v = violation(vec![ItemRef::Track(id(2001))], Some(id(0x900)));
    let before = waiver_key(&v, &b).unwrap();
    assert_eq!(
        before.objects,
        [id(2001).to_string(), format!("keepout:{}", id(0x900))]
    );
    let mut moved = b.clone();
    shift(
        &mut moved.sketch.as_mut().unwrap().keepouts[0].boundary[0],
        1.0,
    );
    assert_ne!(
        waiver_key(&v, &moved).unwrap().fingerprint,
        before.fingerprint
    );
    let mut deleted = b.clone();
    deleted.sketch = None;
    assert_eq!(waiver_key(&v, &deleted), None, "a missing keepout");
}

#[test]
fn a_finding_that_names_nothing_has_no_key() {
    let v = violation(Vec::new(), None);
    assert_eq!(waiver_key(&v, &pair()), None);
}

#[test]
fn a_written_rule_keys_its_kind_too() {
    let b = pair();
    let mut v = run(&b, &DrcRules::new(RULE)).violations[0].clone();
    v.rule = Some("user::hv".into());
    let key = waiver_key(&v, &b).unwrap();
    assert_eq!(key.rule_id, "user::hv");
    assert!(key.objects.contains(&"kind:builtin::clearance".to_string()));
}

#[test]
fn split_matches_by_rule_and_items_and_lapses_on_a_changed_fingerprint() {
    let b = pair();
    let key = only_key(&b);
    let report = run(&b, &DrcRules::new(RULE));

    let matched = split_waived(&report.violations, &b, &[waived(&key)]);
    assert!(matched.kept.is_empty() && matched.lapsed.is_empty());
    assert_eq!(matched.waived.len(), 1);
    assert_eq!(matched.waived[0].1, 0);

    let stale = DrcWaiver {
        fingerprint: "0".repeat(64),
        ..waived(&key)
    };
    let lapsed = split_waived(&report.violations, &b, &[stale]);
    assert_eq!(lapsed.kept.len(), 1);
    assert!(lapsed.waived.is_empty());
    assert_eq!(lapsed.lapsed.iter().copied().collect::<Vec<_>>(), [0]);

    let unrelated = DrcWaiver {
        rule_id: "builtin::short".into(),
        ..waived(&key)
    };
    let other_items = DrcWaiver {
        objects: vec![id(2001).to_string(), id(2003).to_string()],
        ..waived(&key)
    };
    let split = split_waived(&report.violations, &b, &[unrelated, other_items]);
    assert_eq!(split.kept.len(), 1);
    assert!(split.waived.is_empty() && split.lapsed.is_empty());
}

#[test]
fn an_incomplete_waiver_hides_nothing() {
    let b = pair();
    let key = only_key(&b);
    let report = run(&b, &DrcRules::new(RULE));
    let no_reason = DrcWaiver {
        reason: "  ".into(),
        ..waived(&key)
    };
    let split = split_waived(&report.violations, &b, &[no_reason]);
    assert_eq!(split.kept.len(), 1);
    assert!(split.waived.is_empty() && split.lapsed.is_empty());
}

fn checked(b: &PcbBoard, waivers: &[DrcWaiver]) -> CheckReport {
    let names = BTreeMap::from([(1, "GND".to_string()), (2, "VCC".to_string())]);
    let report = run(b, &DrcRules::new(RULE));
    check_report(
        &report,
        &names,
        "/p/b.snxpcb",
        Vec::new(),
        Map::new(),
        b,
        waivers,
    )
}

#[test]
fn check_drc_lists_a_waived_finding_under_waived_with_its_waiver() {
    let b = pair();
    let waiver = DrcWaiver {
        date: "2026-11-17".into(),
        message: "clearance".into(),
        ..waived(&only_key(&b))
    };
    let plain = checked(&b, &[]);
    let report = checked(&b, std::slice::from_ref(&waiver));
    assert!(report.findings.is_empty(), "{report:?}");
    assert!(report.lapsed.is_empty());
    assert_eq!(report.waived.len(), 1);
    assert_eq!(report.waived[0].finding, plain.findings[0]);
    assert_eq!(report.waived[0].waiver, waiver_json(&waiver));
    assert_eq!(report.waived[0].waiver["kind"], "finding");
    assert_eq!(
        report.waived[0].waiver["objects"],
        serde_json::json!(waiver.objects)
    );
}

#[test]
fn check_drc_lists_a_lapsed_waiver_and_shows_its_finding_again() {
    let b = pair();
    let waiver = waived(&only_key(&b));
    let mut moved = pair();
    shift(&mut moved.segments[1].start, -0.05);
    shift(&mut moved.segments[1].end, -0.05);
    let report = checked(&moved, std::slice::from_ref(&waiver));
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert!(report.waived.is_empty());
    assert_eq!(report.lapsed, [waiver_json(&waiver)]);
}

#[test]
fn a_waiver_of_a_rule_switched_off_neither_hides_nor_lapses() {
    let b = pair();
    let stale = DrcWaiver {
        fingerprint: "0".repeat(64),
        ..waived(&only_key(&b))
    };
    let mut rules = DrcRules::new(RULE);
    let off = signex_types::project::DrcRuleSet {
        severity: BTreeMap::from([("builtin::clearance".to_string(), "off".to_string())]),
        ..Default::default()
    };
    rules.severities = crate::resolve_severities(None, Some(&off));
    let report = run(&b, &rules);
    let checked = check_report(
        &report,
        &BTreeMap::new(),
        "/p/b.snxpcb",
        Vec::new(),
        Map::new(),
        &b,
        &[stale],
    );
    assert!(checked.findings.is_empty() && checked.waived.is_empty());
    assert!(checked.lapsed.is_empty(), "{checked:?}");
}

/// A blind via (TopCopper to Inner1) of a keepout finding, keyed and waived.
fn blind_via_board() -> (PcbBoard, Violation, DrcWaiver) {
    let mut v = via(1, (25.0, 25.0), 0);
    v.via_type = ViaType::Blind;
    v.layers = vec!["TopCopper".into(), "Inner1Copper".into()];
    let b = board(vec![], vec![], vec![v, via(2, (60.0, 60.0), 0)]);
    let finding = violation(vec![ItemRef::Via(id(3001))], None);
    let waiver = waived(&waiver_key(&finding, &b).unwrap());
    (b, finding, waiver)
}

#[test]
fn a_via_whose_span_changes_lapses_its_waiver() {
    let (mut b, finding, waiver) = blind_via_board();
    b.vias[0].via_type = ViaType::Through;
    let split = split_waived([&finding], &b, std::slice::from_ref(&waiver));
    assert!(split.waived.is_empty());
    assert_eq!(split.lapsed.iter().copied().collect::<Vec<_>>(), [0]);
}

#[test]
fn a_through_vias_listed_layers_or_another_via_never_lapse_it() {
    let (mut b, _, _) = blind_via_board();
    b.vias[0].via_type = ViaType::Through;
    let finding = violation(vec![ItemRef::Via(id(3001))], None);
    let waiver = waived(&waiver_key(&finding, &b).unwrap());
    // A through via reaches every layer whatever it lists.
    b.vias[0].layers = vec!["TopCopper".into(), "Inner2Copper".into()];
    shift(&mut b.vias[1].position, 5.0);
    b.vias[1].diameter = 1.0;
    let split = split_waived([&finding], &b, std::slice::from_ref(&waiver));
    assert_eq!(split.waived.len(), 1, "{split:?}");
    assert!(split.lapsed.is_empty());
}

fn courtyard(to: (f64, f64)) -> FpGraphic {
    FpGraphic {
        graphic_type: "rect".into(),
        layer: "TopCourtyard".into(),
        width: 0.0,
        start: Some(Point::new(-1.0, -1.0)),
        end: Some(Point::new(to.0, to.1)),
        center: None,
        mid: None,
        radius: 0.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
        position: None,
        rotation: 0.0,
        fill: String::new(),
    }
}

#[test]
fn a_footprint_whose_courtyard_grows_lapses_its_waiver() {
    let mut fp = footprint(1, vec![pad(5, (0.0, 0.0), 1, PadType::Smd, "TopCopper")]);
    fp.graphics = vec![courtyard((1.0, 1.0))];
    let b = board(vec![fp], vec![], vec![]);
    let finding = violation(vec![ItemRef::Footprint(id(1001))], None);
    let before = waiver_key(&finding, &b).unwrap();
    let mut grown = b.clone();
    grown.footprints[0].graphics = vec![courtyard((3.0, 1.0))];
    assert_ne!(
        waiver_key(&finding, &grown).unwrap().fingerprint,
        before.fingerprint
    );
    let mut silk = b.clone();
    silk.footprints[0].graphics.push(FpGraphic {
        layer: "TopSilkscreen".into(),
        ..courtyard((9.0, 9.0))
    });
    assert_eq!(waiver_key(&finding, &silk).unwrap(), before);
}

#[test]
fn two_findings_of_one_identity_cannot_be_waived_and_their_waiver_lapses() {
    let b = board(
        vec![],
        vec![track(1, (0.0, 0.0), (1.0, 0.0), 1, "TopCopper")],
        vec![],
    );
    let one = violation(vec![ItemRef::Track(id(2001))], None);
    let mut two = one.clone();
    two.at = PointNm::new(500_000, 0);
    let waiver = waived(&waiver_key(&one, &b).unwrap());
    assert_eq!(waiver_keys(&[&one, &two], &b), [None, None]);
    assert!(waiver_keys(&[&one], &b)[0].is_some());
    let split = split_waived([&one, &two], &b, std::slice::from_ref(&waiver));
    assert_eq!(split.kept.len(), 2, "{split:?}");
    assert!(split.waived.is_empty());
    assert_eq!(split.lapsed.iter().copied().collect::<Vec<_>>(), [0]);
}

#[test]
fn without_a_complete_waiver_every_finding_is_kept_in_order() {
    let b = pair();
    let report = run(&b, &DrcRules::new(RULE));
    let incomplete = DrcWaiver {
        author: String::new(),
        ..waived(&only_key(&b))
    };
    let split = split_waived(&report.violations, &b, &[incomplete]);
    let kept: Vec<&Violation> = report.violations.iter().collect();
    assert_eq!(split.kept, kept);
    assert!(split.waived.is_empty() && split.lapsed.is_empty());
}
