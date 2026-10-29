//! #788 — `.snxprj` forward compatibility: unknown keys survive a
//! load -> save cycle, and a format token guards against files from a
//! newer build.

use super::*;
use serde_json::{Value, json};

fn write_raw(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("Proj.snxprj");
    std::fs::write(&path, body).unwrap();
    path
}

fn saved_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn unknown_top_level_and_entry_keys_survive_load_then_save() {
    let dir = tempfile::tempdir().unwrap();
    let original = json!({
        "name": "Proj",
        "dir": "",
        "schematic_root": "Proj.snxsch",
        "pcb_file": null,
        "sheets": [{
            "name": "Root", "filename": "Proj.snxsch",
            "future_sheet_key": {"nested": [1, 2, null]}
        }],
        "libraries": [{
            "path": "L.snxlib", "kind": "project_local", "library_id": null,
            "future_lib_key": "pinned"
        }],
        "future_top_scalar": 7,
        "future_top_null": null,
        "future_top_table": {"a": {"b": [true, "x"]}}
    });
    let path = write_raw(dir.path(), &original.to_string());

    let data = parse_project(&path).unwrap();
    write_project(&path, &data).unwrap();

    let after = saved_json(&path);
    assert_eq!(after["future_top_scalar"], json!(7));
    assert!(after.as_object().unwrap().contains_key("future_top_null"));
    assert_eq!(after["future_top_table"], original["future_top_table"]);
    assert_eq!(
        after["sheets"][0]["future_sheet_key"],
        original["sheets"][0]["future_sheet_key"]
    );
    assert_eq!(after["libraries"][0]["future_lib_key"], json!("pinned"));
}

#[test]
fn old_file_without_format_token_loads_and_gains_current_token_on_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(
        dir.path(),
        r#"{"name":"Proj","dir":"","schematic_root":"Proj.snxsch","pcb_file":null}"#,
    );
    let data = parse_project(&path).unwrap();
    assert_eq!(data.schematic_root.as_deref(), Some("Proj.snxsch"));
    write_project(&path, &data).unwrap();
    assert_eq!(saved_json(&path)["format"], json!("snxprj/1"));
    // And the re-saved file still loads.
    parse_project(&path).unwrap();
}

#[test]
fn current_format_token_loads() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(
        dir.path(),
        r#"{"format":"snxprj/1","name":"Proj","dir":"","schematic_root":null,"pcb_file":null}"#,
    );
    parse_project(&path).unwrap();
}

#[test]
fn newer_format_version_is_refused_and_file_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let body = r#"{"format":"snxprj/2","name":"Proj","dir":"","schematic_root":null,"pcb_file":null,"future":1}"#;
    let path = write_raw(dir.path(), body);
    let err = parse_project(&path).expect_err("newer format must be refused");
    assert!(err.to_string().contains("snxprj/2"), "got {err}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), body);
}

#[test]
fn unrecognised_format_token_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(
        dir.path(),
        r#"{"format":"something-else","name":"Proj","dir":"","schematic_root":null,"pcb_file":null}"#,
    );
    assert!(parse_project(&path).is_err());
}

fn retarget_fixture(dir: &Path, libraries: Value) -> PathBuf {
    let body = json!({
        "name": "Proj", "dir": "", "schematic_root": "Proj.snxsch", "pcb_file": null,
        "libraries": libraries,
        "future_top_scalar": 7
    });
    write_raw(dir, &body.to_string())
}

#[test]
fn retarget_keeps_unknown_root_and_entry_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = retarget_fixture(
        dir.path(),
        json!([{"path": "L.snxlib", "kind": "project_local", "library_id": null,
                "future_lib_key": "pinned"}]),
    );
    let old = parse_project(&path).unwrap();
    let old_lib = old.resolve_library_path(&old.libraries[0]);

    let out = retarget_library_entry(&path, &old_lib, "M.snxlib".as_ref()).unwrap();

    assert_eq!(out, LibraryRetarget::Patched);
    let after = saved_json(&path);
    assert_eq!(after["libraries"][0]["path"], json!("M.snxlib"));
    assert_eq!(after["libraries"][0]["future_lib_key"], json!("pinned"));
    assert_eq!(after["future_top_scalar"], json!(7));
}

#[test]
fn retarget_repoints_every_duplicate_entry() {
    let dir = tempfile::tempdir().unwrap();
    let entry = json!({"path": "L.snxlib", "kind": "project_local", "library_id": null});
    let path = retarget_fixture(dir.path(), json!([entry, entry]));
    let old = parse_project(&path).unwrap();
    let old_lib = old.resolve_library_path(&old.libraries[0]);

    retarget_library_entry(&path, &old_lib, "M.snxlib".as_ref()).unwrap();

    let after = saved_json(&path);
    assert_eq!(after["libraries"][0]["path"], json!("M.snxlib"));
    assert_eq!(after["libraries"][1]["path"], json!("M.snxlib"));
}

#[test]
fn retarget_reports_a_shared_disk_entry_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("L.snxlib");
    let path = retarget_fixture(
        dir.path(),
        json!([{"path": lib, "kind": "shared", "library_id": null}]),
    );
    let before = std::fs::read(&path).unwrap();

    let out = retarget_library_entry(&path, &lib, "M.snxlib".as_ref()).unwrap();

    assert_eq!(out, LibraryRetarget::ListedAsDifferentKind);
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
