use serde_json::json;
use signex_types::schematic::{Point, SelectedItem, SelectedKind};
use uuid::Uuid;

use super::*;
use crate::waivers::violation::WaiverKey;
use crate::{NetFinding, RuleId, RuleKind};

fn unused_pin(symbol: Uuid, at: (f64, f64), fingerprint: &str) -> Violation {
    let mut v = Violation::new(
        RuleId::builtin(RuleKind::UnusedPin),
        RuleKind::UnusedPin,
        Severity::Warning,
        "Pin 1 of R1 is not connected",
        Point::new(at.0, at.1),
    );
    v.primary = Some(SelectedItem::new(symbol, SelectedKind::Symbol));
    v.anchor_pin = Some("1".into());
    v.waiver_key = Some(WaiverKey {
        rule_id: v.rule_id.to_string(),
        objects: vec![symbol.to_string(), "pin:1".into()],
        fingerprint: fingerprint.into(),
    });
    v
}

#[test]
fn a_finding_carries_the_shared_fields_and_erc_evidence() {
    let symbol = Uuid::now_v7();
    let f = finding(&unused_pin(symbol, (5.08, 10.16), "f1"), "/p/top.snxsch");
    assert_eq!(f.rule, "builtin::unused_pin");
    assert_eq!(f.severity, CheckSeverity::Warning);
    assert_eq!(f.message, "Pin 1 of R1 is not connected");
    assert_eq!(f.items, [symbol.to_string()]);
    assert_eq!(f.sheet, "/p/top.snxsch");
    assert_eq!(f.key.len(), KEY_HEX_DIGITS);
    assert_eq!(f.evidence["space"], "sheet");
    assert_eq!(f.evidence["at"], json!([5.08, 10.16]));
    assert_eq!(f.evidence["pin"], "1");
    assert_eq!(f.evidence["waiver_key"]["fingerprint"], "f1");
    assert!(f.evidence.get("net").is_none());
}

#[test]
fn the_key_follows_the_waiver_identity_not_the_position_or_content() {
    let symbol = Uuid::now_v7();
    let here = finding(&unused_pin(symbol, (5.08, 10.16), "f1"), "/p/top.snxsch");
    let moved = finding(&unused_pin(symbol, (50.8, 0.0), "f2"), "/p/top.snxsch");
    assert_eq!(here.key, moved.key);
    let other = finding(
        &unused_pin(Uuid::now_v7(), (5.08, 10.16), "f1"),
        "/p/top.snxsch",
    );
    assert_ne!(here.key, other.key);
    let elsewhere = finding(&unused_pin(symbol, (5.08, 10.16), "f1"), "/p/copy.snxsch");
    assert_ne!(here.key, elsewhere.key, "a copied sheet keeps its own keys");
}

#[test]
fn a_finding_that_names_no_object_is_keyed_by_its_message() {
    let plain = |message: &str| {
        Violation::new(
            RuleId::builtin(RuleKind::NetMultipleNames),
            RuleKind::NetMultipleNames,
            Severity::Info,
            message,
            Point::new(1.0, 2.0),
        )
    };
    let a = finding(&plain("net A has two names"), "/p/top.snxsch");
    let mut moved = plain("net A has two names");
    moved.location = Point::new(9.0, 9.0);
    assert_eq!(a.key, finding(&moved, "/p/top.snxsch").key);
    assert_ne!(
        a.key,
        finding(&plain("net B has two names"), "/p/top.snxsch").key
    );
    assert!(a.items.is_empty());
}

#[test]
fn a_net_finding_names_its_net_and_pins() {
    let mut v = Violation::new(
        RuleId::builtin(RuleKind::SinglePinNet),
        RuleKind::SinglePinNet,
        Severity::Warning,
        "net SIG has one pin",
        Point::new(0.0, 0.0),
    );
    v.location_space = LocationSpace::Net;
    v.net = Some(NetFinding {
        name: "SIG".into(),
        pins: Vec::new(),
    });
    let f = finding(&v, "/p/top.snxsch");
    assert_eq!(f.evidence["net"], json!({"name": "SIG", "pins": []}));
    assert_eq!(f.evidence["space"], "net");
}

#[test]
fn a_finding_waiver_says_what_it_waives_and_why() {
    let waiver = ErcWaiver {
        rule_id: "builtin::unused_pin".into(),
        objects: vec!["u1".into(), "pin:1".into()],
        fingerprint: "f1".into(),
        reason: "spare gate".into(),
        author: "hg".into(),
        date: "2026-11-11".into(),
        message: "Pin 1 of R1 is not connected".into(),
        unknown: Default::default(),
    };
    assert_eq!(
        finding_waiver(&waiver),
        json!({
            "kind": "finding",
            "rule_id": "builtin::unused_pin",
            "objects": ["u1", "pin:1"],
            "fingerprint": "f1",
            "reason": "spare gate",
            "author": "hg",
            "date": "2026-11-11",
            "message": "Pin 1 of R1 is not connected",
        })
    );
}

#[test]
fn a_net_waiver_says_its_rule_and_net() {
    assert_eq!(
        net_waiver("builtin::single_pin_net", "SIG"),
        json!({"kind": "net", "rule_id": "builtin::single_pin_net", "net": "SIG"})
    );
}
