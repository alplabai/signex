//! The min slot of a fab profile (mockup row 179d): a `.snxrules` field
//! that round-trips, written at schema 3 only while it is present.

use super::*;
use crate::project::FabTemplate;

fn json(profile: &RuleProfile) -> serde_json::Value {
    serde_json::from_slice(&profile.to_json_bytes()).unwrap()
}

#[test]
fn each_tier_names_its_min_slot() {
    let expected = [
        (FabTemplate::Standard, 0.8),
        (FabTemplate::Advanced, 0.6),
        (FabTemplate::Hdi, 0.5),
    ];
    for (tier, min_slot) in expected {
        assert_eq!(tier.capabilities().min_slot, min_slot, "{tier:?}");
        assert_eq!(tier.info().min_slot, Some(min_slot), "{tier:?}");
    }
}

#[test]
fn a_fab_section_with_a_min_slot_round_trips_at_schema_3() {
    let text = br#"{"schema":2,"name":"Maker","version":"1",
        "fab":{"router_tool_diameter":2.0,"min_slot":0.8}}"#;
    let profile = RuleProfile::from_json_bytes(text).unwrap();
    assert_eq!(profile.min_slot(), Some(0.8));
    let saved = json(&profile);
    assert_eq!(saved["schema"], RULE_PROFILE_SCHEMA_V3);
    assert_eq!(saved["fab"]["min_slot"], 0.8);
    let again = RuleProfile::from_json_bytes(&profile.to_json_bytes()).unwrap();
    assert_eq!(again.fab, profile.fab);
    assert_eq!(again.schema, Some(RULE_PROFILE_SCHEMA_V3));
    assert_eq!(again.to_json_bytes(), profile.to_json_bytes());
}

#[test]
fn removing_the_min_slot_steps_the_schema_back() {
    let text = br#"{"schema":3,"name":"C","fab":{"router_tool_diameter":2.0,"min_slot":0.6}}"#;
    let mut profile = RuleProfile::from_json_bytes(text).unwrap();
    profile.fab.as_mut().unwrap().min_slot = None;
    assert_eq!(
        json(&profile)["schema"],
        RULE_PROFILE_SCHEMA_V2,
        "the router still needs 2"
    );
    assert!(json(&profile)["fab"].get("min_slot").is_none());
    profile.fab.as_mut().unwrap().router_tool_diameter = None;
    assert_eq!(json(&profile)["schema"], RULE_PROFILE_SCHEMA_V1);
}

#[test]
fn a_tier_profile_is_written_at_schema_3() {
    let mut profile = RuleProfile::from_json_bytes(br#"{"name":"C"}"#).unwrap();
    profile.fab = Some(FabTemplate::Hdi.info());
    let saved = json(&profile);
    assert_eq!(saved["schema"], RULE_PROFILE_SCHEMA_V3);
    assert_eq!(saved["fab"]["min_slot"], 0.5);
}

#[test]
fn schema_3_is_read() {
    let read = RuleProfile::from_json_bytes(br#"{"schema":3,"name":"x"}"#).unwrap();
    assert_eq!(read.name, "x");
}

#[test]
fn the_min_slot_is_no_rule_content() {
    let mut a = RuleProfile::from_json_bytes(br#"{"name":"C"}"#).unwrap();
    let before = a.content_sha256();
    a.fab = Some(FabProfileInfo {
        min_slot: Some(0.8),
        ..FabProfileInfo::default()
    });
    assert_eq!(a.content_sha256(), before);
}
