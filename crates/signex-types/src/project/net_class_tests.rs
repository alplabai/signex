//! `net_classes` in the `.snxprj`: byte-stable when empty, round-trips,
//! loads from files that predate it.

use super::*;
use crate::net_class::{NetClass, NetClassStyle, NetLineStyle};
use crate::theme::Color;

const OLD_FILE: &str = r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#;

fn project_file(json: &str) -> (std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("P.snxprj");
    std::fs::write(&path, json).unwrap();
    (path, dir)
}

fn sample_class() -> NetClass {
    NetClass {
        name: "Power".into(),
        nets: vec!["GND".into()],
        patterns: vec!["VCC*".into()],
        style: NetClassStyle {
            color: Some(Color::new(220, 30, 30, 255)),
            wire_width_mm: Some(0.35),
            line: NetLineStyle::Dotted,
        },
        ..NetClass::default()
    }
}

#[test]
fn old_file_loads_with_no_classes() {
    let (path, _dir) = project_file(OLD_FILE);
    assert!(parse_project(&path).unwrap().net_classes.is_empty());
}

#[test]
fn project_without_classes_resaves_byte_identical() {
    let (path, _dir) = project_file(OLD_FILE);
    let first = parse_project(&path).unwrap();
    write_project(&path, &first).unwrap();
    let saved = std::fs::read(&path).unwrap();
    let second = parse_project(&path).unwrap();
    write_project(&path, &second).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    assert!(
        !String::from_utf8(saved).unwrap().contains("net_classes"),
        "an empty table must not appear in the file"
    );
}

#[test]
fn classes_round_trip_through_write_and_parse() {
    let (path, _dir) = project_file(OLD_FILE);
    let mut data = parse_project(&path).unwrap();
    data.net_classes = vec![sample_class(), NetClass::named("Empty")];
    write_project(&path, &data).unwrap();
    let back = parse_project(&path).unwrap();
    assert_eq!(back.net_classes, data.net_classes);
    // Table order is semantic (first match wins) and must be preserved.
    assert_eq!(back.net_classes[0].name, "Power");
    assert_eq!(back.net_classes[1].name, "Empty");
}

#[test]
fn saved_form_is_stable_across_a_second_round_trip() {
    let (path, _dir) = project_file(OLD_FILE);
    let mut data = parse_project(&path).unwrap();
    data.net_classes = vec![sample_class()];
    write_project(&path, &data).unwrap();
    let first = std::fs::read(&path).unwrap();
    let again = parse_project(&path).unwrap();
    write_project(&path, &again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), first);
}

#[test]
fn an_unknown_key_in_a_class_row_does_not_stop_the_file_opening() {
    let (path, _dir) = project_file(
        r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null,
            "net_classes":[{"name":"A","future_field":1}]}"#,
    );
    let data = parse_project(&path).unwrap();
    assert_eq!(data.net_classes[0].name, "A");

    // A save by this build must not drop the key a newer build wrote.
    write_project(&path, &data).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("future_field"), "{saved}");
    let again = parse_project(&path).unwrap();
    assert_eq!(
        again.net_classes[0].unknown.get("future_field"),
        Some(&serde_json::json!(1))
    );
}

/// An older build has no `net_classes` field; its flatten bag must carry the
/// key through a load/save. Simulated by a struct with only the bag.
#[test]
fn a_build_without_the_field_preserves_the_table() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct OldProject {
        name: String,
        #[serde(flatten)]
        unknown: UnknownJson,
    }
    let mut data = parse_project(&project_file(OLD_FILE).0).unwrap();
    data.net_classes = vec![sample_class()];
    let json = serde_json::to_string(&data).unwrap();
    let old: OldProject = serde_json::from_str(&json).unwrap();
    let resaved = serde_json::to_string(&old).unwrap();
    let back: ProjectData = serde_json::from_str(&resaved).unwrap();
    assert_eq!(back.net_classes, data.net_classes);
}
