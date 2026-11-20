//! The declared, short-lived test adapter of ADR-0006 (slice C, "Safety net
//! before the high-risk slices"): it drives today's `Signex` the way the
//! future `AppCore` port is driven, so a test written against it passes
//! before and after a slice moves the behaviour behind it.
//!
//! Its test API is the frozen part: [`TestAdapter::execute`],
//! [`TestAdapter::query`], [`TestAdapter::deliver`] and the three gestures
//! (`open_from_tree`, `restore`, `drag`, in paths and mm). Later slices swap
//! the inside, never the API. Raw `Message`s stay outside it. It lives behind
//! the crate's self-dev-dependency feature (Cargo.toml `test-prefs-redirect`)
//! so the tests under `tests/` reach it; the slice that lands `AppCore`
//! removes it (ADR-0006, "Consequences").

mod disk;
mod gestures;
#[cfg(test)]
mod hostile_tests;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use iced::Task;

pub use disk::{DiskWrite, TestDisk};

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::persist::DocumentSaves;
use crate::task_driver::{DriverLimits, TaskDriver};

/// Who calls: a person at the GUI, or an agent (the socket, a headless run).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Person,
    Agent,
}

/// Why a call did nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalKind {
    /// The id is in no catalog.
    UnknownId,
    /// The operands did not parse against the catalog's descriptor.
    Operands,
    /// The command had nothing to act on, or refused what it was given.
    Disabled,
    /// No route runs the id from this origin.
    NoRoute,
}

/// What a call did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Ran,
    Refused {
        kind: RefusalKind,
        reason_en: String,
    },
}

/// Why the adapter could not do what a test asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterError {
    /// A gesture found nothing to act on (no open project holds the path,
    /// no tab shows it, the tree refused the open).
    NothingToActOn(String),
    /// Delivered work did not finish within the limit.
    Unfinished(String),
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NothingToActOn(why) => write!(f, "nothing to act on: {why}"),
            Self::Unfinished(why) => write!(f, "unfinished: {why}"),
        }
    }
}

impl std::error::Error for AdapterError {}

/// Background work a call started, unrun until [`TestAdapter::deliver`].
#[must_use = "deliver it, or the work it holds is dropped unrun"]
#[derive(Debug, PartialEq, Eq)]
pub struct Pending(u64);

/// One app on a temporary root, with a [`TestDisk`] for its document writes.
pub struct TestAdapter {
    app: Box<Signex>,
    driver: TaskDriver,
    pending: BTreeMap<u64, Task<Message>>,
    next_pending: u64,
    disk: Arc<TestDisk>,
    root: tempfile::TempDir,
}

/// How long one [`TestAdapter::deliver`] may run by default.
const DELIVER_LIMIT: Duration = Duration::from_secs(30);

impl TestAdapter {
    pub fn new() -> Self {
        Self::with_deliver_limit(DELIVER_LIMIT)
    }

    /// An adapter whose [`Self::deliver`] gives up after `limit`.
    pub fn with_deliver_limit(limit: Duration) -> Self {
        let root = tempfile::tempdir().expect("the test adapter's temporary root");
        let disk = TestDisk::new(root.path().to_path_buf());
        let mut app = Box::new(Signex::new().0);
        // Dialogs answer nothing, launches and git commits are refused.
        app.ui_state.io = crate::io_ports::IoPorts::headless(Default::default());
        app.ui_state.first_run_tour_open = false;
        app.document_state.document_saves = DocumentSaves::new(disk.clone());
        let limits = DriverLimits {
            max_messages: DriverLimits::default().max_messages,
            max_duration: limit,
        };
        Self {
            app,
            driver: TaskDriver::with_limits(limits),
            pending: BTreeMap::new(),
            next_pending: 0,
            disk,
            root,
        }
    }

    /// The temporary folder every document of the test lives under.
    pub fn root(&self) -> &Path {
        self.root.path()
    }

    /// The document writes that landed.
    pub fn disk(&self) -> &TestDisk {
        &self.disk
    }

    /// Run `id` with `args` (`name=value`) as `origin` calls it. Background
    /// work it starts waits in the returned handle.
    pub fn execute(&mut self, id: &str, args: &[&str], origin: Origin) -> (Outcome, Pending) {
        let (command, args) = match parse(id, args) {
            Ok(parsed) => parsed,
            Err(refusal) => return (refusal, self.hold(Task::none())),
        };
        let (outcome, task) = match origin {
            Origin::Agent => self.app.run_headless_command_in_test(&command, args),
            Origin::Person => self.run_as_person(&command, args),
        };
        (outcome_of(outcome), self.hold(task))
    }

    /// The answer of the query `id`, asked by a person.
    pub fn query(&mut self, id: &str, args: &[&str]) -> Result<serde_json::Value, Outcome> {
        let (command, args) = parse(id, args)?;
        let asked = self
            .app
            .with_origin(crate::app::agent::Origin::Person, |app| {
                app.run_query_headless(&command, &args)
            });
        match asked {
            Some((CommandOutcome::Answered(value), _)) => Ok(value),
            Some((outcome, _)) => Err(outcome_of(outcome)),
            None => Err(Outcome::Refused {
                kind: RefusalKind::NoRoute,
                reason_en: format!("`{id}` is not a query"),
            }),
        }
    }

    /// Run the work `pending` holds and feed what it reports back, until
    /// nothing is left or the limit is reached.
    pub fn deliver(&mut self, pending: Pending) -> Result<(), AdapterError> {
        let Some(task) = self.pending.remove(&pending.0) else {
            return Ok(());
        };
        self.driver
            .settle(&mut self.app, task)
            .map_err(|error| AdapterError::Unfinished(error.to_string()))
    }

    fn hold(&mut self, task: Task<Message>) -> Pending {
        self.next_pending += 1;
        self.pending.insert(self.next_pending, task);
        Pending(self.next_pending)
    }

    /// A person's call: a query, the operand handler, or the route a key,
    /// menu row or the palette takes.
    fn run_as_person(
        &mut self,
        command: &AppCommandId,
        args: CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        self.app
            .with_origin(crate::app::agent::Origin::Person, |app| {
                if let Some(answered) = app.run_query_headless(command, &args) {
                    return answered;
                }
                let ran = if args.has_operands() {
                    app.run_operands(command, &args)
                } else {
                    app.dispatch_command_reporting(command, args, None)
                };
                match ran {
                    (CommandOutcome::Deferred { ticket }, task) => {
                        app.settle_deferred(ticket, task)
                    }
                    ran => ran,
                }
            })
    }
}

impl Default for TestAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// The command and its parsed operands, or why they did not parse.
fn parse(id: &str, args: &[&str]) -> Result<(AppCommandId, CommandArgs), Outcome> {
    let command = AppCommandId::new(id)
        .ok()
        .filter(|command| crate::keymap::metadata_for(command).is_some())
        .ok_or_else(|| outcome_of(CommandOutcome::UnknownId))?;
    // No operands is the surface that gives none (a key, a menu row), not a
    // call missing the required ones; the route decides what that means.
    if args.is_empty() {
        return Ok((command, CommandArgs::none()));
    }
    let parsed = CommandArgs::parse_operands(&command, args).map_err(|error| Outcome::Refused {
        kind: RefusalKind::Operands,
        reason_en: error.to_string(),
    })?;
    Ok((command, parsed))
}

fn outcome_of(outcome: CommandOutcome) -> Outcome {
    let refused = |kind, reason_en: String| Outcome::Refused { kind, reason_en };
    match outcome {
        CommandOutcome::Ran | CommandOutcome::Answered(_) | CommandOutcome::Deferred { .. } => {
            Outcome::Ran
        }
        CommandOutcome::UnknownId => refused(RefusalKind::UnknownId, "unknown id".into()),
        CommandOutcome::NoBridgeArm => refused(RefusalKind::NoRoute, "no bridge arm".into()),
        CommandOutcome::NotHeadless => refused(RefusalKind::NoRoute, "not headless".into()),
        CommandOutcome::Disabled { reason } => refused(
            RefusalKind::Disabled,
            signex_i18n::RenderTranslatable::render_en_us(&reason),
        ),
        // `CommandOutcome` is `#[non_exhaustive]` since it moved to
        // signex-commands; a new variant is named here, never taken as run.
        other => refused(RefusalKind::NoRoute, format!("{other:?}")),
    }
}
