//! The result envelope of one command, shared by the headless runner
//! (`signex-cli`) and the running app's agent channel (#1924): the same
//! statuses and the same reading of a [`CommandOutcome`], so a caller sees one
//! shape whichever process ran its command. See `docs/AGENT_SOCKET.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;

use crate::app::Signex;
use crate::app::command::CommandOutcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// The command ran and changed or did what it names.
    Ran,
    /// A query ran; its answer is in `data`.
    Answered,
    /// Nothing ran: the policy or the app said no, with a reason.
    Refused,
    /// The command ran and did not succeed, or never finished.
    Failed,
}

impl Status {
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ran | Self::Answered)
    }
}

/// One entry of the app's Messages log, as it was logged during a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MessageLine {
    pub level: &'static str,
    pub message: String,
}

/// What a dispatch outcome means for the result line. A `Ran` that left an
/// error notice failed; nothing unknown is ever reported as ran.
pub fn outcome_verdict(
    id: &str,
    outcome: CommandOutcome,
    notice: Option<String>,
) -> (Status, Option<String>, Option<Value>) {
    match outcome {
        CommandOutcome::Ran => match notice {
            Some(why) => (Status::Failed, Some(why), None),
            None => (Status::Ran, None, None),
        },
        CommandOutcome::UnknownId => (
            Status::Refused,
            Some(format!("unknown command `{id}`")),
            None,
        ),
        CommandOutcome::NoBridgeArm => (
            Status::Refused,
            Some(format!(
                "not runnable headless: `{id}` has no dispatch arm yet"
            )),
            None,
        ),
        // A command that made nothing because the app said why in an error
        // notice (an open of a missing file) failed; it was not refused.
        CommandOutcome::Disabled { reason } => match notice {
            Some(why) => (Status::Failed, Some(why), None),
            None => (Status::Refused, Some(reason), None),
        },
        // A query ran: what it found is the result, never a bare `ran`.
        CommandOutcome::Answered(value) => (Status::Answered, None, Some(value)),
        other => (
            Status::Failed,
            Some(format!("unexpected outcome {other:?}")),
            None,
        ),
    }
}

/// The state a command can change, per document: engine revision and unsaved
/// flag. A document that is gone or new differs too.
pub type ChangeSnapshot = BTreeMap<PathBuf, (u64, bool)>;

/// The documents whose content or unsaved state differ between two snapshots.
pub fn changed_between(before: &ChangeSnapshot, after: &ChangeSnapshot) -> Vec<String> {
    let paths: BTreeSet<&PathBuf> = before.keys().chain(after.keys()).collect();
    paths
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .map(|path| path.display().to_string())
        .collect()
}

/// What the Messages log gained after the entry `mark`.
pub fn messages_since(mark: Option<u64>) -> Vec<MessageLine> {
    crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|entry| MessageLine {
            level: entry.level.label(),
            message: entry.message,
        })
        .collect()
}

impl Signex {
    /// Every document's revision and unsaved flag now.
    pub fn change_snapshot(&self) -> ChangeSnapshot {
        let state = &self.document_state;
        let dirty = state.dirty_document_paths();
        let mut all: ChangeSnapshot = state
            .engines
            .iter()
            .map(|(path, engine)| (path.clone(), (engine.revision(), dirty.contains(path))))
            .collect();
        for path in dirty {
            all.entry(path).or_insert((0, true));
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(entries: &[(&str, u64, bool)]) -> ChangeSnapshot {
        entries
            .iter()
            .map(|(path, rev, dirty)| (PathBuf::from(path), (*rev, *dirty)))
            .collect()
    }

    #[test]
    fn a_changed_revision_a_flipped_flag_a_new_or_a_gone_document_is_changed() {
        let before = snap(&[
            ("a", 1, false),
            ("b", 1, false),
            ("c", 1, false),
            ("d", 1, true),
        ]);
        let after = snap(&[
            ("a", 2, false),
            ("b", 1, true),
            ("c", 1, false),
            ("e", 1, false),
        ]);
        assert_eq!(changed_between(&before, &after), ["a", "b", "d", "e"]);
        assert!(changed_between(&before, &before).is_empty());
    }

    #[test]
    fn an_error_notice_makes_a_ran_command_failed_and_unknown_outcomes_never_ran() {
        let ran = outcome_verdict("x", CommandOutcome::Ran, None);
        assert_eq!(ran.0, Status::Ran);
        let failed = outcome_verdict(
            "x",
            CommandOutcome::Ran,
            Some("Open failed: no file".into()),
        );
        assert_eq!(
            (failed.0, failed.1.as_deref()),
            (Status::Failed, Some("Open failed: no file"))
        );
        let unknown = outcome_verdict("x", CommandOutcome::UnknownId, None);
        assert_eq!(unknown.0, Status::Refused);
        let arm = outcome_verdict("x", CommandOutcome::NoBridgeArm, None);
        assert!(arm.1.unwrap().contains("no dispatch arm"));
        let disabled = outcome_verdict(
            "x",
            CommandOutcome::Disabled {
                reason: "why".into(),
            },
            None,
        );
        assert_eq!(
            (disabled.0, disabled.1.as_deref()),
            (Status::Refused, Some("why"))
        );
        // A refusal the app explained in an error notice (an open of a file
        // that is not there) is a failure with that notice.
        let noticed = outcome_verdict(
            "x",
            CommandOutcome::Disabled {
                reason: "made no document".into(),
            },
            Some("Open failed: no file".into()),
        );
        assert_eq!(
            (noticed.0, noticed.1.as_deref()),
            (Status::Failed, Some("Open failed: no file"))
        );
        // A query's answer is the result: `answered` with `data`.
        let answer = serde_json::json!({"count": 1});
        let answered = outcome_verdict("x", CommandOutcome::Answered(answer.clone()), None);
        assert_eq!(answered, (Status::Answered, None, Some(answer)));
        let not_headless = outcome_verdict("x", CommandOutcome::NotHeadless, None);
        assert_eq!(not_headless.0, Status::Failed);
    }
}
