//! #2253 — the agent journal settings: stored as one optional `agent_journal`
//! key, absent while untouched, read back, never able to make a project
//! unopenable, and written with no format token change.

use super::*;
use serde_json::{Value, json};

fn write_raw(dir: &Path, body: &Value) -> PathBuf {
    let path = dir.join("Proj.snxprj");
    std::fs::write(&path, body.to_string()).unwrap();
    path
}

fn saved_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn bare() -> Value {
    json!({"name": "Proj", "dir": "", "schematic_root": null, "pcb_file": null})
}

#[test]
fn an_untouched_project_saves_without_the_key_and_keeps_its_token() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(dir.path(), &bare());
    let data = parse_project(&path).unwrap();
    assert!(data.agent_journal.is_default());
    write_project(&path, &data).unwrap();
    let saved = saved_json(&path);
    assert!(saved.get("agent_journal").is_none());
    assert_eq!(saved["format"], json!(SNXPRJ_FORMAT_V1));
}

#[test]
fn the_settings_are_persisted_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(dir.path(), &bare());
    let mut data = parse_project(&path).unwrap();
    data.agent_journal = AgentJournalSettings {
        keep_in_git: false,
        summarise_in_commit: true,
        max_lines: 250,
    };
    write_project(&path, &data).unwrap();
    assert_eq!(
        saved_json(&path)["agent_journal"],
        json!({"keep_in_git": false, "summarise_in_commit": true, "max_lines": 250})
    );
    assert_eq!(
        parse_project(&path).unwrap().agent_journal,
        data.agent_journal
    );
}

#[test]
fn a_value_of_another_shape_reads_as_the_defaults_and_the_project_opens() {
    let dir = tempfile::tempdir().unwrap();
    for bad in [
        json!("yes"),
        json!(3),
        json!([1]),
        json!({"keep_in_git": "no"}),
    ] {
        let mut body = bare();
        body["agent_journal"] = bad;
        let path = write_raw(dir.path(), &body);
        let data = parse_project(&path).unwrap();
        assert!(data.agent_journal.is_default());
    }
}
