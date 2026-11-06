//! The headless entry (#1933): boot Signex without a window and run commands
//! by id, for the command runner (#1923). The whole public API of this crate
//! for a non-GUI caller is here and in the modules this one names.
//!
//! A headless process, in this order:
//! 1. [`crate::config_root::set_config_root_override`] first thing, so no
//!    config file of the user's is read or written;
//! 2. [`crate::fonts::catalog::install_before_boot`], so text is measured
//!    as in the GUI;
//! 3. [`Signex::boot_headless`], then [`crate::task_driver::TaskDriver`] to
//!    run the returned task;
//! 4. per command, [`crate::io_ports::HeadlessAnswers::replace_all`] on
//!    [`Signex::headless_answers`] with that command's dialog answers (none
//!    when it has none), then [`Signex::dispatch_command_headless`], driving
//!    each task;
//! 5. [`Signex::end_headless_session`] to flush and exit.

use std::path::PathBuf;

use iced::Task;
use signex_types::project::RuleProfile;

pub use crate::app::command::{CommandArgs, CommandOutcome, OperandError, OperandValue};
use crate::app::{Message, Signex};
use crate::io_ports::{HeadlessAnswers, Picker};
use crate::keymap::AppCommandId;

/// Why [`Signex::boot_headless`] refused.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HeadlessBootError {
    /// No [`crate::config_root::set_config_root_override`]: a headless run
    /// would read and write the user's real config.
    NoConfigRootOverride,
    /// A path to open is relative, so it would depend on the process's
    /// working folder (the dialog answers follow the same rule).
    NotAbsolute(PathBuf),
}

impl std::fmt::Display for HeadlessBootError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoConfigRootOverride => {
                f.write_str("a headless run needs a config root override set first")
            }
            Self::NotAbsolute(path) => {
                write!(f, "{} is not an absolute path", path.display())
            }
        }
    }
}

impl std::error::Error for HeadlessBootError {}

impl Signex {
    /// Boot for a headless run, opening `paths` like a command-line launch
    /// and checking ERC with `rules`, never the machine's User Default
    /// profile.
    ///
    /// Unlike [`Signex::boot`] it does not sweep or move recovery copies,
    /// scan fonts (see [`crate::fonts::catalog::install_before_boot`]),
    /// migrate or load rule profiles, or probe the desktop. Dialogs are
    /// answered from [`crate::io_ports::HeadlessAnswers`] and launches are
    /// refused ([`crate::io_ports::IoPorts::headless`]).
    pub fn boot_headless(
        paths: Vec<PathBuf>,
        rules: &RuleProfile,
    ) -> Result<(Self, Task<Message>), HeadlessBootError> {
        if !crate::config_root::is_override_active() {
            return Err(HeadlessBootError::NoConfigRootOverride);
        }
        if let Some(path) = paths.iter().find(|path| !path.is_absolute()) {
            return Err(HeadlessBootError::NotAbsolute(path.clone()));
        }
        let (mut app, boot) = Self::new();
        app.ui_state.io = crate::io_ports::IoPorts::headless(Default::default());
        let rules_applied = app.handle_user_default_loaded(&Ok(Some(rules.clone())));
        let opens = Self::startup_open_messages(paths)
            .into_iter()
            .map(Task::done);
        Ok((
            app,
            Task::batch([boot, rules_applied].into_iter().chain(opens)),
        ))
    }

    /// The dialog answers of an app from [`Self::boot_headless`]; `None`
    /// for an app with native dialogs.
    pub fn headless_answers(&self) -> Option<&HeadlessAnswers> {
        match &self.ui_state.io.picker {
            Picker::Headless(answers) => Some(answers),
            Picker::Native => None,
        }
    }

    /// Run the command `command` with `args` as every GUI surface runs it,
    /// and say what happened. On an app that was not booted headless (no
    /// config root override, native dialogs, or launches not refused)
    /// nothing runs: [`CommandOutcome::NotHeadless`].
    pub fn dispatch_command_headless(
        &mut self,
        command: &AppCommandId,
        args: CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        if !crate::config_root::is_override_active()
            || self.headless_answers().is_none()
            || self.ui_state.io.opener != crate::io_ports::Opener::Refuse
        {
            return (CommandOutcome::NotHeadless, Task::none());
        }
        self.dispatch_command_reporting(command, args, None)
    }
}
