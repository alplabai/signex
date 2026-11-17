//! `rules.drc_waivers` (#2307): stored like `erc_waivers`, outside the
//! rule-set hash.

use super::{WITH_NET_WAIVERS_ONLY, open_and_resave, project_with_rules, rules_of};
use crate::project::{
    DrcWaiver, ProjectData, ProjectRules, parse_project, rule_set_sha256, write_project,
};

fn waiver() -> DrcWaiver {
    DrcWaiver {
        rule_id: "builtin::clearance".into(),
        objects: vec![
            "00000000-0000-4000-8000-000000000001".into(),
            "00000000-0000-4000-8000-000000000002/00000000-0000-4000-8000-000000000003".into(),
        ],
        fingerprint: "cd".repeat(32),
        reason: "Factory-approved neck-down".into(),
        author: "A. Designer".into(),
        date: "2026-11-17".into(),
        message: "clearance 0.15 mm, needs 0.2 mm, GND / VCC".into(),
        unknown: Default::default(),
    }
}

#[test]
fn drc_waivers_round_trip_identically() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.snxprj");
    let mut data: ProjectData = serde_json::from_str(WITH_NET_WAIVERS_ONLY).unwrap();
    data.rules.as_mut().unwrap().drc_waivers.push(waiver());
    write_project(&path, &data).unwrap();
    let back = parse_project(&path).unwrap();
    assert_eq!(back.rules.as_ref().unwrap().drc_waivers, vec![waiver()]);
    assert_eq!(back.rules, data.rules);
    // `dir` is re-derived on load, so compare two load -> save cycles.
    write_project(&path, &back).unwrap();
    let second = std::fs::read(&path).unwrap();
    write_project(&path, &parse_project(&path).unwrap()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), second, "save is byte-stable");
}

#[test]
fn a_file_without_drc_waivers_resaves_byte_identical_without_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.snxprj");
    std::fs::write(&path, WITH_NET_WAIVERS_ONLY).unwrap();
    let data = parse_project(&path).unwrap();
    assert!(data.rules.as_ref().unwrap().drc_waivers.is_empty());
    write_project(&path, &data).unwrap();
    let first = std::fs::read(&path).unwrap();
    write_project(&path, &parse_project(&path).unwrap()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), first);
    assert!(!String::from_utf8_lossy(&first).contains("drc_waivers"));
    let default = serde_json::to_string(&ProjectRules::default()).unwrap();
    assert!(!default.contains("drc_waivers"), "{default}");
}

#[test]
fn a_drc_waiver_from_a_newer_build_keeps_its_unknown_keys() {
    let text = project_with_rules(
        r#"{"schema":1,"drc_waivers":[{"rule_id":"builtin::short","objects":["u","v"],"fingerprint":"f","reason":"r","author":"a","expires":"2027-01-01"}]}"#,
    );
    let (data, saved) = open_and_resave(&text);
    let w = &data.rules.as_ref().unwrap().drc_waivers[0];
    assert!(w.is_complete());
    assert_eq!(w.objects, ["u", "v"]);
    assert_eq!(saved["rules"], rules_of(&text));
}

#[test]
fn drc_waivers_never_change_the_rule_set_hash() {
    let mut rules = ProjectRules::default();
    rules
        .drc
        .severity
        .insert("builtin::short".into(), "warning".into());
    let before = rule_set_sha256(&rules.erc, &rules.drc);
    rules.drc_waivers.push(waiver());
    assert_eq!(rule_set_sha256(&rules.erc, &rules.drc), before);
}
