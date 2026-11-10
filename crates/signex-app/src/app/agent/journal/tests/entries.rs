//! The entry types: the size cap, the replay check, the version.

use super::{SESSION, access, edit};
use crate::app::agent::journal::{AccessEntry, EditEntry, MAX_ENTRY_BYTES};

#[test]
fn every_line_carries_version_one_first() {
    let line = edit("color_net", 1).to_line();
    assert!(line.starts_with(r#"{"v":1,"#), "{line}");
    assert!(access("closed").to_line().starts_with(r#"{"v":1,"#));
    // The writer sets it; a caller's value cannot change it.
    let mut wrong = edit("color_net", 1);
    wrong.v = 9;
    assert!(wrong.to_line().starts_with(r#"{"v":1,"#));
}

#[test]
fn an_entry_over_the_cap_keeps_its_summary_and_drops_the_operands() {
    let mut big = edit("apply_edits", 7);
    big.operands
        .insert("edits".into(), "e".repeat(MAX_ENTRY_BYTES));
    big.operands
        .insert("summary".into(), "move the pull-up".into());

    let line = big.to_line();

    assert!(line.len() < MAX_ENTRY_BYTES, "{}", line.len());
    let back: EditEntry = serde_json::from_str(&line).unwrap();
    assert!(back.truncated);
    assert!(!back.replayable);
    assert_eq!(back.operands.len(), 1);
    assert_eq!(back.operands["summary"], "move the pull-up");
}

#[test]
fn an_entry_within_the_cap_is_whole_and_replayable() {
    let back: EditEntry = serde_json::from_str(&edit("color_net", 3).to_line()).unwrap();
    assert!(!back.truncated && back.replayable);
    assert_eq!(back.operands["at"], "5,0");
}

#[test]
fn a_refused_command_is_never_replayable() {
    let mut refused = edit("color_net", 3);
    refused.outcome = "refused".into();
    refused.reason = Some("no net there".into());
    let back: EditEntry = serde_json::from_str(&refused.to_line()).unwrap();
    assert!(!back.replayable);
}

#[test]
fn the_free_text_fields_are_bounded() {
    let mut long = edit("color_net", 3);
    long.reason = Some("r".repeat(10_000));
    long.path = Some("p".repeat(10_000));
    let back: EditEntry = serde_json::from_str(&long.to_line()).unwrap();
    assert_eq!(back.reason.unwrap().chars().count(), 1024);
    assert_eq!(back.path.unwrap().chars().count(), 1024);
}

#[test]
fn replay_works_in_the_recorded_session_at_the_recorded_revision_and_refuses_at_any_other() {
    let back: EditEntry = serde_json::from_str(&edit("color_net", 12).to_line()).unwrap();

    let (command, operands) = back.replay_request(SESSION, 12).unwrap();
    assert_eq!(command, "color_net");
    assert_eq!(operands, [("at".to_string(), "5,0".to_string())]);

    for other in [0, 11, 13, 99] {
        let refused = back.replay_request(SESSION, other).unwrap_err();
        assert!(refused.contains("revision"), "{refused}");
        assert!(refused.contains("Nothing was applied"));
    }
}

/// A revision means something only within the session that recorded it: the
/// engine's counter starts again in every process, so another session at the
/// very same number is not the document the line ran on.
#[test]
fn a_line_is_refused_in_another_session_even_at_the_same_revision() {
    let back: EditEntry = serde_json::from_str(&edit("color_net", 12).to_line()).unwrap();

    let refused = back.replay_request("another-session", 12).unwrap_err();

    assert!(refused.contains("another session"), "{refused}");
    assert!(refused.contains("Nothing was applied"));
}

#[test]
fn a_line_with_no_session_is_never_replayable() {
    let mut old = edit("color_net", 12);
    old.session = String::new();
    let back: EditEntry = serde_json::from_str(&old.to_line()).unwrap();
    assert!(!back.replayable);
    assert!(back.replay_request("", 12).is_err());
    // A line an older build wrote has no `session` key at all.
    let line = r#"{"v":1,"ts":"t","pid":1,"client":null,"document":null,"path":null,"command":"color_net","operands":{},"revision_before":1,"revision_after":2,"group":1,"outcome":"ran","reason":null,"truncated":false,"replayable":true}"#;
    let older: EditEntry = serde_json::from_str(line).unwrap();
    assert!(older.replay_request("s", 1).is_err());
}

/// A `revert` names an undo group that exists only in its session.
#[test]
fn a_revert_line_is_not_replayable() {
    let back: EditEntry = serde_json::from_str(&edit("revert", 4).to_line()).unwrap();
    assert!(!back.replayable);
    assert!(back.replay_request(SESSION, 4).is_err());
}

#[test]
fn a_truncated_or_refused_entry_cannot_be_replayed_at_all() {
    let mut big = edit("apply_edits", 5);
    big.operands
        .insert("edits".into(), "e".repeat(MAX_ENTRY_BYTES));
    let truncated: EditEntry = serde_json::from_str(&big.to_line()).unwrap();
    assert!(truncated.replay_request(SESSION, 5).is_err());
}

/// An access line is held to the same 64 KiB rule as an edit line: a long
/// list of rights keeps its first entries, each bounded, and says so.
#[test]
fn an_access_line_over_the_cap_is_cut_and_says_so() {
    let many: Vec<String> = (0..500)
        .map(|n| format!("allow-path:/{n}/{}", "x".repeat(2_000)))
        .collect();
    let line = access("grants").with_capabilities(many).to_line();

    assert!(line.len() < MAX_ENTRY_BYTES, "{}", line.len());
    let back: AccessEntry = serde_json::from_str(&line).unwrap();
    assert!(back.truncated);
    assert!(back.capabilities.len() <= 64);
    assert!(back.capabilities.iter().all(|c| c.chars().count() <= 1024));
}

#[test]
fn a_short_access_line_is_not_marked_truncated() {
    let line = access("grants")
        .with_capabilities(vec!["read".into(), "edit".into()])
        .to_line();
    assert!(!line.contains("truncated"), "{line}");
}
