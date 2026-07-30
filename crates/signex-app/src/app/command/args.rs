//! [`CommandArgs`] — the Command Registry's argument bag (#278 slice 2).
//!
//! All 134 catalog commands are nullary today, so this type has nothing
//! to carry yet. It exists only so [`super::Signex::dispatch_command`]'s
//! signature will not have to change once a consumer (a future CLI is
//! the expected one) needs to pass data through it. Do not add an
//! arg-consuming command against this slice — [`super::bridge::core_to_message`]
//! does not read `CommandArgs` at all yet.

/// An ordered bag of string arguments for a dispatched command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CommandArgs(#[allow(dead_code)] Vec<String>);

impl CommandArgs {
    /// No arguments — what every current catalog command dispatches with.
    pub(crate) fn none() -> Self {
        Self(Vec::new())
    }
}
