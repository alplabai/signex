//! #2172 — a project's own id: stored as `document_id`, read back, optional
//! for files an older build wrote, never able to make a project unopenable.

use super::*;
use serde_json::{Value, json};

const ID: &str = "0193a2f0-7b1c-7cde-8123-456789abcdef";

fn write_raw(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("Proj.snxprj");
    std::fs::write(&path, body).unwrap();
    path
}

fn saved_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn the_id_is_written_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(
        dir.path(),
        r#"{"name":"Proj","dir":"","schematic_root":null,"pcb_file":null}"#,
    );
    let mut data = parse_project(&path).unwrap();
    assert_eq!(data.document_id, None);
    let id = DocumentId::new();
    data.document_id = Some(id);
    write_project(&path, &data).unwrap();
    assert_eq!(saved_json(&path)["document_id"], json!(id.to_string()));
    assert_eq!(parse_project(&path).unwrap().document_id, Some(id));
}

#[test]
fn the_id_survives_a_load_then_save_cycle() {
    let dir = tempfile::tempdir().unwrap();
    let body = json!({
        "name": "Proj", "dir": "", "schematic_root": null, "pcb_file": null,
        "document_id": ID
    });
    let path = write_raw(dir.path(), &body.to_string());
    let data = parse_project(&path).unwrap();
    write_project(&path, &data).unwrap();
    assert_eq!(saved_json(&path)["document_id"], json!(ID));
}

#[test]
fn a_value_that_is_not_an_id_reads_as_none_and_the_project_still_opens() {
    let dir = tempfile::tempdir().unwrap();
    for bad in [
        json!("nope"),
        json!(7),
        json!(null),
        json!("00000000-0000-0000-0000-000000000000"),
    ] {
        let body = json!({
            "name": "Proj", "dir": "", "schematic_root": null, "pcb_file": null,
            "document_id": bad
        });
        let path = write_raw(dir.path(), &body.to_string());
        assert_eq!(parse_project(&path).unwrap().document_id, None, "{bad}");
    }
}

#[test]
fn the_header_reader_finds_the_id_a_save_wrote() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(
        dir.path(),
        r#"{"name":"Proj","dir":"","schematic_root":null,"pcb_file":null}"#,
    );
    let mut data = parse_project(&path).unwrap();
    let id = DocumentId::new();
    data.document_id = Some(id);
    write_project(&path, &data).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        crate::format::document_id_in_header("snxprj", &bytes),
        Some(id)
    );
}
