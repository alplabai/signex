//! `description` in the `.snxprj` (#1202): optional, byte-stable when
//! absent, round-trips, and `encode_project` is what `write_project` writes.

use super::*;

const OLD_FILE: &str = r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#;

fn project_file(json: &str) -> (std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("P.snxprj");
    std::fs::write(&path, json).unwrap();
    (path, dir)
}

#[test]
fn old_file_loads_without_a_description() {
    let (path, _dir) = project_file(OLD_FILE);
    assert_eq!(parse_project(&path).unwrap().description, None);
}

#[test]
fn untouched_project_resaves_byte_identical_without_a_description_key() {
    let (path, _dir) = project_file(OLD_FILE);
    let first = parse_project(&path).unwrap();
    write_project(&path, &first).unwrap();
    let saved = std::fs::read(&path).unwrap();
    let second = parse_project(&path).unwrap();
    write_project(&path, &second).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    assert!(
        !String::from_utf8(saved).unwrap().contains("description"),
        "an absent description must not appear in the file"
    );
}

#[test]
fn description_round_trips_through_write_and_parse() {
    let (path, _dir) = project_file(OLD_FILE);
    let mut data = parse_project(&path).unwrap();
    data.description = Some("Motor controller, rev B".to_string());
    write_project(&path, &data).unwrap();
    let back = parse_project(&path).unwrap();
    assert_eq!(back.description.as_deref(), Some("Motor controller, rev B"));
    // And a second save of what was read is byte-identical.
    let saved = std::fs::read(&path).unwrap();
    write_project(&path, &back).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), saved);
}

#[test]
fn encode_project_is_exactly_what_write_project_writes() {
    let (path, _dir) = project_file(OLD_FILE);
    let mut data = parse_project(&path).unwrap();
    data.description = Some("x".to_string());
    write_project(&path, &data).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        encode_project(&data).unwrap()
    );
}
