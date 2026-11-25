//! What one command request came to, read the same way on every surface
//! (#2495): [`RequestOutcome`] is the typed reading of a [`CommandOutcome`].
//! The agent reply prints it through [`crate::headless::outcome_verdict`],
//! and `signex-cli` through [`RequestOutcome::after_notice`] then
//! [`RequestOutcome::into_verdict`]; nothing unknown is ever read as ran.

use serde_json::Value;
use signex_i18n::RenderTranslatable;

use crate::app::agent::Status;
use crate::app::command::CommandOutcome;

/// Why a request was refused. Nothing ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RefusalCause {
    /// The id is in no catalog, or has no dispatch arm yet.
    NoSuchCommand,
    /// The app said no: nothing to act on, or operands or a document that do
    /// not suit the command.
    NotAvailable,
    /// The request's `if_revision` was not met: the document is at another
    /// revision, or has none to compare.
    Revision,
}

/// The outcome of one command request.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RequestOutcome {
    /// A query ran; this is what it found.
    Answered(Value),
    /// The command ran.
    Ran,
    /// Nothing ran, for `reason` (en-US, so a reply is locale-invariant).
    Refused { reason: String, cause: RefusalCause },
    /// The command ran and did not succeed, or never finished.
    Failed(String),
    /// The command waits on its own off-thread work (a file read); its
    /// outcome comes with the ticket's answer (`docs/AGENT_EDITS.md`).
    Waiting { ticket: u64 },
}

impl RequestOutcome {
    /// The reading of what the command `id` returned.
    pub fn of(id: &str, outcome: CommandOutcome) -> Self {
        match outcome {
            CommandOutcome::Ran => Self::Ran,
            CommandOutcome::Answered(value) => Self::Answered(value),
            CommandOutcome::UnknownId => Self::Refused {
                reason: format!("unknown command `{id}`{}", crate::keymap::did_you_mean(id)),
                cause: RefusalCause::NoSuchCommand,
            },
            CommandOutcome::NoBridgeArm => Self::Refused {
                reason: format!("not runnable headless: `{id}` has no dispatch arm yet"),
                cause: RefusalCause::NoSuchCommand,
            },
            CommandOutcome::Disabled { reason } => Self::Refused {
                reason: reason.render_en_us(),
                cause: RefusalCause::NotAvailable,
            },
            CommandOutcome::Deferred { ticket } => Self::Waiting { ticket },
            CommandOutcome::NotHeadless => Self::Failed("unexpected outcome NotHeadless".into()),
        }
    }

    /// The outcome once the request's tasks settled, with the error notice
    /// they left, if any: a `Ran` that left one failed, and so did a refusal
    /// the app explained in one (an open of a file that is not there).
    #[must_use]
    pub fn after_notice(self, notice: Option<String>) -> Self {
        match (self, notice) {
            (Self::Ran, Some(why))
            | (
                Self::Refused {
                    cause: RefusalCause::NotAvailable,
                    ..
                },
                Some(why),
            ) => Self::Failed(why),
            (outcome, _) => outcome,
        }
    }

    /// The status, reason and data a reply line carries. A request still
    /// waiting has no status yet; read as a reply it failed.
    pub fn into_verdict(self) -> (Status, Option<String>, Option<Value>) {
        match self {
            Self::Answered(value) => (Status::Answered, None, Some(value)),
            Self::Ran => (Status::Ran, None, None),
            Self::Refused { reason, .. } => (Status::Refused, Some(reason), None),
            Self::Failed(why) => (Status::Failed, Some(why), None),
            Self::Waiting { ticket } => (
                Status::Failed,
                Some(format!(
                    "unexpected outcome {:?}",
                    CommandOutcome::Deferred { ticket }
                )),
                None,
            ),
        }
    }

    /// A refusal for `cause`.
    pub(crate) fn refused(reason: impl Into<String>, cause: RefusalCause) -> Self {
        Self::Refused {
            reason: reason.into(),
            cause,
        }
    }
}
