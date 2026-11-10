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
        // A wrong-typed cap is the same refusal, not a silent default.
        json!({"max_lines": "5000"}),
        json!({"max_lines": -3, "keep_in_git": false}),
        json!({"max_lines": 99_999_999_999_u64}),
    ] {
        let bad_text = bad.to_string();
        let mut body = bare();
        body["agent_journal"] = bad;
        let path = write_raw(dir.path(), &body);
        let data = parse_project(&path).unwrap();
        assert!(data.agent_journal.is_default());
        // Not silently reset: it is flagged, and a save keeps it as it was.
        assert!(data.has_unreadable_agent_journal(), "{bad_text}");
        write_project(&path, &data).unwrap();
        assert_eq!(saved_json(&path)["agent_journal"], body["agent_journal"]);
    }
}

#[test]
fn editing_the_settings_replaces_an_unreadable_value() {
    let dir = tempfile::tempdir().unwrap();
    let mut body = bare();
    body["agent_journal"] = json!("garbage");
    let path = write_raw(dir.path(), &body);
    let mut data = parse_project(&path).unwrap();
    data.set_agent_journal(AgentJournalSettings {
        keep_in_git: false,
        ..AgentJournalSettings::default()
    });

    write_project(&path, &data).unwrap();

    assert_eq!(
        saved_json(&path)["agent_journal"]["keep_in_git"],
        json!(false)
    );
    assert!(!parse_project(&path).unwrap().has_unreadable_agent_journal());
}

/// Keys a newer build stored beside the settings survive a re-save, with the
/// settings untouched and with them edited; and they are not "unreadable".
#[test]
fn unknown_keys_inside_the_object_are_kept_across_a_resave() {
    let dir = tempfile::tempdir().unwrap();
    let mut body = bare();
    body["agent_journal"] = json!({"max_lines": 250, "retention": {"days": 30}});
    let path = write_raw(dir.path(), &body);

    let mut data = parse_project(&path).unwrap();
    assert_eq!(data.agent_journal.max_lines, 250);
    assert!(!data.has_unreadable_agent_journal());
    write_project(&path, &data).unwrap();
    let kept = saved_json(&path);
    assert_eq!(kept["agent_journal"]["max_lines"], json!(250));
    assert_eq!(kept["agent_journal"]["retention"], json!({"days": 30}));

    // Edited: the new settings, the unknown key still there.
    data.set_agent_journal(AgentJournalSettings {
        keep_in_git: false,
        max_lines: 300,
        ..AgentJournalSettings::default()
    });
    write_project(&path, &data).unwrap();
    let saved = saved_json(&path);
    assert_eq!(saved["agent_journal"]["max_lines"], json!(300));
    assert_eq!(saved["agent_journal"]["keep_in_git"], json!(false));
    assert_eq!(saved["agent_journal"]["retention"], json!({"days": 30}));

    // Only unknown keys, settings left at their defaults: the object is kept.
    let mut only = bare();
    only["agent_journal"] = json!({"retention": 1});
    let path = write_raw(dir.path(), &only);
    let data = parse_project(&path).unwrap();
    assert!(data.agent_journal.is_default());
    write_project(&path, &data).unwrap();
    assert_eq!(saved_json(&path)["agent_journal"], json!({"retention": 1}));
}

/// An in-range number is clamped; the cap is never stored out of range.
#[test]
fn the_line_cap_read_from_a_file_is_clamped_into_its_range() {
    let dir = tempfile::tempdir().unwrap();
    for (stored, want) in [(5_u64, 100_u32), (4_000_000_000, 1_000_000), (250, 250)] {
        let mut body = bare();
        body["agent_journal"] = json!({ "max_lines": stored });
        let path = write_raw(dir.path(), &body);
        assert_eq!(parse_project(&path).unwrap().agent_journal.max_lines, want);
    }
}
