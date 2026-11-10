//! The entry types: the size cap, redaction, the replay check, the version.

use super::{access, edit};
use crate::app::agent::journal::{EditEntry, MAX_ENTRY_BYTES};

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
fn a_secret_is_replaced_in_operands_and_reason() {
    let mut leaky = edit("apply_edits", 1);
    leaky
        .operands
        .insert("summary".into(), "use SECRET123 here".into());
    leaky.reason = Some("bad SECRET123".into());
    leaky.redact("SECRET123");
    let line = leaky.to_line();
    assert!(!line.contains("SECRET123"), "{line}");
    assert!(line.contains("[redacted]"));
}

#[test]
fn replay_works_at_the_recorded_revision_and_refuses_at_any_other() {
    let back: EditEntry = serde_json::from_str(&edit("color_net", 12).to_line()).unwrap();

    let (command, operands) = back.replay_request(12).unwrap();
    assert_eq!(command, "color_net");
    assert_eq!(operands, [("at".to_string(), "5,0".to_string())]);

    for other in [0, 11, 13, 99] {
        let refused = back.replay_request(other).unwrap_err();
        assert!(refused.contains("revision"), "{refused}");
        assert!(refused.contains("Nothing was applied"));
    }
}

#[test]
fn a_truncated_or_refused_entry_cannot_be_replayed_at_all() {
    let mut big = edit("apply_edits", 5);
    big.operands
        .insert("edits".into(), "e".repeat(MAX_ENTRY_BYTES));
    let truncated: EditEntry = serde_json::from_str(&big.to_line()).unwrap();
    assert!(truncated.replay_request(5).is_err());
}
