//! The router tool diameter of a fab profile (mockup row 179d): a `.snxrules` field
//! that round-trips, written at schema 2 only while it is present.

use super::*;
use crate::project::FabTemplate;

fn json(profile: &RuleProfile) -> serde_json::Value {
    serde_json::from_slice(&profile.to_json_bytes()).unwrap()
}

#[test]
fn a_fab_section_with_a_router_diameter_round_trips_at_schema_2() {
    let text = br#"{"schema":1,"name":"Maker","version":"1",
        "fab":{"manufacturer":"Maker","router_tool_diameter":2.4}}"#;
    let profile = RuleProfile::from_json_bytes(text).unwrap();
    assert_eq!(profile.router_tool_diameter(), Some(2.4));
    let saved = json(&profile);
    assert_eq!(saved["schema"], RULE_PROFILE_SCHEMA_V2);
    assert_eq!(saved["fab"]["router_tool_diameter"], 2.4);
    let again = RuleProfile::from_json_bytes(&profile.to_json_bytes()).unwrap();
    assert_eq!(again.router_tool_diameter(), Some(2.4));
    assert_eq!(again.fab, profile.fab);
    assert_eq!(again.schema, Some(RULE_PROFILE_SCHEMA_V2));
    // Saved twice, the bytes do not move.
    assert_eq!(again.to_json_bytes(), profile.to_json_bytes());
}

#[test]
fn a_fab_section_without_a_router_diameter_keeps_its_bytes_and_schema_1() {
    let text = b"{\n  \"schema\": 1,\n  \"name\": \"C\",\n  \"version\": \"1\",\n  \"fab\": {\n    \"manufacturer\": \"M\"\n  }\n}\n";
    let profile = RuleProfile::from_json_bytes(text).unwrap();
    assert_eq!(profile.router_tool_diameter(), None);
    assert_eq!(profile.to_json_bytes(), text);
    assert!(json(&profile)["fab"].get("router_tool_diameter").is_none());
}

#[test]
fn a_profile_without_a_schema_key_stays_without_one_until_it_needs_2() {
    let mut profile = RuleProfile::from_json_bytes(br#"{"name":"C"}"#).unwrap();
    assert!(json(&profile).get("schema").is_none());
    profile.fab = Some(FabProfileInfo {
        router_tool_diameter: Some(1.6),
        ..FabProfileInfo::default()
    });
    assert_eq!(json(&profile)["schema"], RULE_PROFILE_SCHEMA_V2);
}

#[test]
fn removing_the_router_diameter_writes_schema_1_again() {
    let text = br#"{"schema":2,"name":"C","fab":{"router_tool_diameter":2.0}}"#;
    let mut profile = RuleProfile::from_json_bytes(text).unwrap();
    profile.fab.as_mut().unwrap().router_tool_diameter = None;
    assert_eq!(json(&profile)["schema"], RULE_PROFILE_SCHEMA_V1);
}

#[test]
fn schema_2_is_read_and_a_later_one_still_refused() {
    let read = RuleProfile::from_json_bytes(br#"{"schema":2,"name":"x"}"#).unwrap();
    assert_eq!(read.name, "x");
    let err = RuleProfile::from_json_bytes(br#"{"schema":4,"name":"x"}"#).unwrap_err();
    assert!(matches!(err, RuleProfileError::UnsupportedSchema(4)));
}

#[test]
fn the_router_diameter_is_no_rule_content() {
    let mut a = RuleProfile::from_json_bytes(br#"{"name":"C"}"#).unwrap();
    let before = a.content_sha256();
    a.fab = Some(FabProfileInfo {
        router_tool_diameter: Some(2.0),
        ..FabProfileInfo::default()
    });
    assert_eq!(a.content_sha256(), before);
}

#[test]
fn each_tier_names_its_router_tool() {
    let expected = [
        (FabTemplate::Standard, 2.0),
        (FabTemplate::Advanced, 1.6),
        (FabTemplate::Hdi, 1.0),
    ];
    for (tier, diameter) in expected {
        assert_eq!(
            tier.capabilities().router_tool_diameter,
            diameter,
            "{tier:?}"
        );
        assert_eq!(tier.info().router_tool_diameter, Some(diameter), "{tier:?}");
    }
}

/// The project's copy (mockup row 179d): a top-level `.snxprj` key, absent until a
/// tier or profile sets it, read back as written.
#[test]
fn the_project_router_diameter_round_trips_and_is_absent_by_default() {
    use crate::project::{ProjectData, parse_project, write_project};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Router.snxprj");
    let mut data: ProjectData = serde_json::from_str(
        r#"{"name":"Router","dir":"","schematic_root":null,"pcb_file":null,"future":1}"#,
    )
    .unwrap();
    assert_eq!(data.router_tool_diameter(), None);
    write_project(&path, &data).unwrap();
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(raw.get("router_tool_diameter").is_none(), "{raw}");

    data.set_router_tool_diameter(Some(2.0));
    write_project(&path, &data).unwrap();
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(raw["router_tool_diameter"], 2.0);
    assert_eq!(raw["future"], 1, "other keys stay");
    let back = parse_project(&path).unwrap();
    assert_eq!(back.router_tool_diameter(), Some(2.0));

    let mut cleared = back;
    cleared.set_router_tool_diameter(None);
    write_project(&path, &cleared).unwrap();
    assert_eq!(parse_project(&path).unwrap().router_tool_diameter(), None);
}

#[test]
fn a_project_router_diameter_that_is_not_a_positive_number_reads_as_absent() {
    for value in ["0", "-1.5", "\"2mm\"", "null", "[2]"] {
        let text = format!(
            r#"{{"name":"R","dir":"","schematic_root":null,"pcb_file":null,"router_tool_diameter":{value}}}"#
        );
        let data: crate::project::ProjectData = serde_json::from_str(&text).unwrap();
        assert_eq!(data.router_tool_diameter(), None, "{value}");
    }
    let mut data: crate::project::ProjectData =
        serde_json::from_str(r#"{"name":"R","dir":"","schematic_root":null,"pcb_file":null}"#)
            .unwrap();
    data.set_router_tool_diameter(Some(f64::NAN));
    assert_eq!(data.router_tool_diameter(), None);
}
