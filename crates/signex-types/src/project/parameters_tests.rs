//! `parameters` in the `.snxprj` (#1080): optional, byte-stable when absent,
//! round-trips with user pairs, and sits beside keys this build does not know.

use super::*;

const OLD_FILE: &str = r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#;

fn project_file(json: &str) -> (std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("P.snxprj");
    std::fs::write(&path, json).unwrap();
    (path, dir)
}

#[test]
fn old_file_loads_with_no_parameters_and_resaves_byte_identical() {
    let (path, _dir) = project_file(OLD_FILE);
    let first = parse_project(&path).unwrap();
    assert!(first.parameters.is_empty());
    write_project(&path, &first).unwrap();
    let saved = std::fs::read(&path).unwrap();
    assert!(
        !String::from_utf8(saved.clone())
            .unwrap()
            .contains("parameters")
    );
    write_project(&path, &parse_project(&path).unwrap()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), saved);
}

#[test]
fn parameters_round_trip_with_user_pairs_and_unknown_keys() {
    let (path, _dir) = project_file(
        r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null,
            "parameters":{"title":"Motor","rev":"B","Voltage":"24 V"},
            "future_key":{"a":1}}"#,
    );
    let data = parse_project(&path).unwrap();
    assert_eq!(
        data.parameters.get("title").map(String::as_str),
        Some("Motor")
    );
    assert_eq!(
        data.parameters.get("Voltage").map(String::as_str),
        Some("24 V")
    );
    write_project(&path, &data).unwrap();
    let back = parse_project(&path).unwrap();
    assert_eq!(back.parameters, data.parameters);
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(raw["future_key"]["a"], 1);
    let saved = std::fs::read(&path).unwrap();
    write_project(&path, &back).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), saved);
}
