//! What [`super::Signex::dispatch_command_reporting`] did with a command.

/// The result of dispatching a command id, returned next to the `Task`.
///
/// A click, a key or the palette ignores it; a CLI or an agent (#1923) reads
/// it to tell "ran" from "ignored".
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandOutcome {
    /// The command resolved to its handler and the handler was run. The
    /// handler may still report that it had nothing to do in the Messages
    /// panel.
    Ran,
    /// The id is in no command catalog: almost always a typo in a
    /// user-edited keymap profile.
    UnknownId,
    /// The id is a catalog command with no arm in the bridge yet.
    NoBridgeArm,
    /// The command has nothing to act on in the window it was invoked from.
    /// Where the command's handler explains this to the user, it still ran
    /// (and said why); every other disabled command was dropped.
    Disabled { reason: String },
    /// A headless dispatch on an app that was not booted headless (no config
    /// root override, or native dialogs): nothing ran (#1933).
    NotHeadless,
    /// A query command ran and this is its answer (#2172). A query never
    /// reports a bare [`Self::Ran`]: what it found is the point of running it.
    Answered(serde_json::Value),
    /// The edit's checks need files read off the UI thread first (#2199): the
    /// returned task reads them and runs the edit on completion, and the
    /// outcome is reported then (the socket reply, or the headless run that
    /// drives the task and takes the outcome by `ticket`).
    Deferred { ticket: u64 },
}

impl CommandOutcome {
    pub(super) fn disabled(reason: impl Into<String>) -> Self {
        Self::Disabled {
            reason: reason.into(),
        }
    }
}
