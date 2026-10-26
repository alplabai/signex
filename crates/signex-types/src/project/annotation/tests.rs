use crate::designator::DesignatorStandard;
use crate::project::{ProjectData, parse_project, write_project};

fn project_file(dir: &std::path::Path, extra: &str) -> std::path::PathBuf {
    let path = dir.join("p.snxprj");
    let body = format!(r#"{{"name":"p","dir":"","schematic_root":null,"pcb_file":null{extra}}}"#);
    std::fs::write(&path, body).unwrap();
    path
}

fn resave(path: &std::path::Path) -> (ProjectData, Vec<u8>) {
    let data = parse_project(path).unwrap();
    write_project(path, &data).unwrap();
    (data, std::fs::read(path).unwrap())
}

#[test]
fn a_missing_key_is_none_and_a_resave_is_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let path = project_file(dir.path(), "");
    let (data, first) = resave(&path);
    assert_eq!(data.designator_standard(), None);
    assert!(data.annotation.is_none());
    assert!(!String::from_utf8_lossy(&first).contains("annotation"));
    let (_, second) = resave(&path);
    assert_eq!(first, second);
}

#[test]
fn a_known_standard_loads_and_round_trips_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let path = project_file(
        dir.path(),
        r#","annotation":{"designator_standard":"din40719_2"}"#,
    );
    let (data, first) = resave(&path);
    assert_eq!(
        data.designator_standard(),
        Some(DesignatorStandard::Din40719_2)
    );
    let (_, second) = resave(&path);
    assert_eq!(first, second);
    assert!(String::from_utf8_lossy(&first).contains("\"din40719_2\""));
}

#[test]
fn an_unknown_standard_loads_is_not_applied_and_is_kept_verbatim() {
    let dir = tempfile::tempdir().unwrap();
    let path = project_file(
        dir.path(),
        r#","annotation":{"designator_standard":"iso9999","future":[1,2]}"#,
    );
    let (data, first) = resave(&path);
    assert_eq!(data.designator_standard(), None);
    assert!(data.has_unreadable_annotation());
    let json: serde_json::Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(json["annotation"]["designator_standard"], "iso9999");
    assert_eq!(json["annotation"]["future"][1], 2);
    let (_, second) = resave(&path);
    assert_eq!(first, second);
}

#[test]
fn an_explicit_choice_replaces_an_unreadable_section() {
    let dir = tempfile::tempdir().unwrap();
    let path = project_file(
        dir.path(),
        r#","annotation":{"designator_standard":"iso9999"}"#,
    );
    let mut data = parse_project(&path).unwrap();
    data.set_designator_standard(Some(DesignatorStandard::Ieee315));
    write_project(&path, &data).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["annotation"]["designator_standard"], "ieee315");
    assert!(!parse_project(&path).unwrap().has_unreadable_annotation());
}

#[test]
fn clearing_the_choice_removes_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = project_file(dir.path(), "");
    let before = resave(&path).1;
    let mut data = parse_project(&path).unwrap();
    data.set_designator_standard(Some(DesignatorStandard::Iec81346_2));
    assert_eq!(
        data.designator_standard(),
        Some(DesignatorStandard::Iec81346_2)
    );
    data.set_designator_standard(None);
    assert!(data.annotation.is_none());
    write_project(&path, &data).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn an_empty_or_null_section_loads_as_none_and_resave_omits_it() {
    for extra in [
        r#","annotation":{}"#,
        r#","annotation":{"designator_standard":null}"#,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = project_file(dir.path(), extra);
        let (data, bytes) = resave(&path);
        assert!(data.annotation.is_none(), "{extra}");
        assert!(!data.has_unreadable_annotation(), "{extra}");
        assert!(
            !String::from_utf8_lossy(&bytes).contains("annotation"),
            "{extra}"
        );
    }
}
