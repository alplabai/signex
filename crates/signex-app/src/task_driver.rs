//! Run the `Task`s `update` returns without an iced runtime or a window: for
//! tests and headless runs (#1927, part of #1923).
//!
//! The driver answers every iced `Action` a signex task can ask for that has a
//! sensible window-less answer: clipboard reads and writes go to an in-memory
//! clipboard, `window::open` is acknowledged with the id it asked for (so the
//! app sees `MainWindowOpened`), `window::close` reports the close the way the
//! close-events subscription does, window size and scale queries get fixed
//! answers, and `iced::exit()` ends the session. Anything else is dropped with
//! a warning.
//!
//! Messages are fed back through `update` in a deterministic first-in,
//! first-out order: each task's messages in turn, then their follow-ups.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use iced::Task;
use iced::futures::StreamExt as _;
use iced_runtime::Action;
use iced_runtime::core::clipboard::{Content, Error as ClipboardError, Kind};

use crate::app::{Message, Signex, WindowMsg};
use crate::ignore::IgnoreResult as _;

/// `oneshot::Sender::send` fails only when the task stopped listening.
const UNANSWERED: &str = "the task stopped waiting for the answer";

/// The size every headless window reports.
pub const HEADLESS_WINDOW_SIZE: iced::Size = iced::Size::new(1280.0, 800.0);

/// The scale factor every headless window reports.
pub const HEADLESS_SCALE_FACTOR: f32 = 1.0;

/// How much work one call may do before it gives up.
#[derive(Debug, Clone, Copy)]
pub struct DriverLimits {
    /// Messages fed through `update` per call.
    pub max_messages: usize,
    /// Wall-clock time per call.
    pub max_duration: Duration,
}

impl Default for DriverLimits {
    fn default() -> Self {
        Self {
            max_messages: 100_000,
            max_duration: Duration::from_secs(120),
        }
    }
}

/// Why a call stopped before its work ran out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveError {
    StepLimit(usize),
    TimedOut(Duration),
}

impl std::fmt::Display for DriveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StepLimit(limit) => {
                write!(f, "the task chain produced more than {limit} messages")
            }
            Self::TimedOut(limit) => write!(f, "the task chain did not finish within {limit:?}"),
        }
    }
}

impl std::error::Error for DriveError {}

/// What the driver did with the effects tasks asked for.
#[derive(Debug, Default)]
struct Effects {
    clipboard: Option<Content>,
    has_exited: bool,
    /// System quit requests the app answered with a cancel.
    cancelled_quits: u64,
    /// Window actions that got no answer (see [`TaskDriver::dropped_window_actions`]).
    dropped_windows: u64,
}

/// One headless session: one tokio runtime and one clipboard.
pub struct TaskDriver {
    runtime: tokio::runtime::Runtime,
    limits: DriverLimits,
    effects: Effects,
}

impl TaskDriver {
    /// A session with the default limits.
    pub fn new() -> Self {
        Self::with_limits(DriverLimits::default())
    }

    pub fn with_limits(limits: DriverLimits) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build the task driver's tokio runtime");
        Self {
            runtime,
            limits,
            effects: Effects::default(),
        }
    }

    /// Whether a task asked the app to exit. Once it has, nothing more runs.
    pub fn has_exited(&self) -> bool {
        self.effects.has_exited
    }

    /// How many system quit requests tasks have answered with a cancel
    /// (`iced::system::cancel_quit`). There is no real request to cancel
    /// here, so the driver only counts them.
    pub fn cancelled_quits(&self) -> u64 {
        self.effects.cancelled_quits
    }

    /// How many window actions (a window the command asked for that is not
    /// opened, closed or measured here) the driver has dropped since it was
    /// made. A caller that must not report such a command as done compares
    /// it before and after.
    pub fn dropped_window_actions(&self) -> u64 {
        self.effects.dropped_windows
    }

    /// The text on the in-memory clipboard, if it holds text.
    pub fn clipboard_text(&self) -> Option<&str> {
        match &self.effects.clipboard {
            Some(Content::Text(text)) => Some(text),
            _ => None,
        }
    }

    /// Put `text` on the in-memory clipboard, as a copy in another app would.
    pub fn set_clipboard_text(&mut self, text: impl Into<String>) {
        self.effects.clipboard = Some(Content::Text(text.into()));
    }

    /// The messages `task` emits when it runs to completion, in order. They
    /// are not fed back through `update`.
    pub fn outputs(&mut self, task: Task<Message>) -> Result<Vec<Message>, DriveError> {
        let deadline = Instant::now() + self.limits.max_duration;
        self.collect(task, deadline)
    }

    /// Run `task`, feed every message it produces through `update`, and run
    /// the tasks those return, first in first out, until nothing is left.
    pub fn settle(&mut self, app: &mut Signex, task: Task<Message>) -> Result<(), DriveError> {
        self.settle_with(|message| app.update(message), task)
    }

    fn settle_with(
        &mut self,
        mut update: impl FnMut(Message) -> Task<Message>,
        task: Task<Message>,
    ) -> Result<(), DriveError> {
        let deadline = Instant::now() + self.limits.max_duration;
        let mut steps = 0usize;
        let mut queue = VecDeque::from([task]);
        while let Some(task) = queue.pop_front() {
            for message in self.collect(task, deadline)? {
                if self.effects.has_exited {
                    tracing::info!(
                        target: "signex::task_driver",
                        "the app exited; dropping the messages still queued"
                    );
                    return Ok(());
                }
                steps += 1;
                if steps > self.limits.max_messages {
                    return Err(DriveError::StepLimit(self.limits.max_messages));
                }
                queue.push_back(update(message));
            }
        }
        Ok(())
    }

    /// [`Self::settle`] for one message.
    pub fn send(&mut self, app: &mut Signex, message: Message) -> Result<(), DriveError> {
        let task = app.update(message);
        self.settle(app, task)
    }

    fn collect(
        &mut self,
        task: Task<Message>,
        deadline: Instant,
    ) -> Result<Vec<Message>, DriveError> {
        if self.effects.has_exited {
            return Ok(Vec::new());
        }
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            return Ok(Vec::new());
        };
        let limit = self.limits.max_duration;
        let remaining = deadline.saturating_duration_since(Instant::now());
        let effects = &mut self.effects;
        self.runtime.block_on(async {
            let run = async {
                let mut out = Vec::new();
                while let Some(action) = stream.next().await {
                    if let Some(message) = effects.perform(action) {
                        out.push(message);
                    }
                    // Exit stops the runtime at once: work still running in
                    // the same task never reports back (#1932).
                    if effects.has_exited {
                        break;
                    }
                }
                out
            };
            tokio::time::timeout(remaining, run)
                .await
                .map_err(|_| DriveError::TimedOut(limit))
        })
    }
}

impl Default for TaskDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl Effects {
    /// Answer one action; the message, when it is an output.
    fn perform(&mut self, action: Action<Message>) -> Option<Message> {
        match action {
            Action::Output(message) => return Some(message),
            Action::Clipboard(action) => self.clipboard(action),
            Action::Window(action) => return self.window(action),
            Action::Exit => self.has_exited = true,
            Action::System(iced_runtime::system::Action::CancelQuit) => self.cancelled_quits += 1,
            other => dropped(action_kind(&other)),
        }
        None
    }

    fn clipboard(&mut self, action: iced_runtime::clipboard::Action) {
        use iced_runtime::clipboard::Action as Clip;
        match action {
            Clip::Read { kind, channel } => {
                channel.send(self.read(kind)).ignore(UNANSWERED);
            }
            Clip::Write { content, channel } => {
                self.clipboard = Some(content);
                channel.send(Ok(())).ignore(UNANSWERED);
            }
        }
    }

    fn read(&self, kind: Kind) -> Result<Content, ClipboardError> {
        match (&self.clipboard, kind) {
            (Some(content @ Content::Text(_)), Kind::Text)
            | (Some(content @ Content::Html(_)), Kind::Html)
            | (Some(content @ Content::Files(_)), Kind::Files) => Ok(content.clone()),
            _ => Err(ClipboardError::ContentNotAvailable),
        }
    }

    /// The message a window action produces, when it produces one.
    fn window(&mut self, action: iced_runtime::window::Action) -> Option<Message> {
        use iced_runtime::window::Action as Win;
        match action {
            // What `bootstrap::subscription`'s `close_events` delivers.
            Win::Close(id) => return Some(Message::Window(WindowMsg::SecondaryWindowClosed(id))),
            Win::Open(id, _settings, channel) => channel.send(id).ignore(UNANSWERED),
            Win::GetSize(_, channel) => channel.send(HEADLESS_WINDOW_SIZE).ignore(UNANSWERED),
            Win::GetScaleFactor(_, channel) => {
                channel.send(HEADLESS_SCALE_FACTOR).ignore(UNANSWERED)
            }
            _ => {
                self.dropped_windows += 1;
                dropped("window");
            }
        }
        None
    }
}

fn dropped(kind: &'static str) {
    tracing::warn!(
        target: "signex::task_driver",
        kind,
        "dropped an iced action a headless run does not perform"
    );
}

fn action_kind(action: &Action<Message>) -> &'static str {
    match action {
        Action::Output(_) => "output",
        Action::Widget(_) => "widget operation",
        Action::Clipboard(_) => "clipboard",
        Action::Window(_) => "window",
        Action::System(_) => "system",
        Action::Font(_) => "font",
        Action::Image(_) => "image",
        Action::Backend(_) => "backend",
        Action::Event { .. } => "event",
        Action::Tick => "tick",
        Action::Reload => "reload",
        Action::Exit => "exit",
    }
}

/// Test helper: `task` must do exactly one thing, cancel the pending system
/// quit request. No exit, no window close, no message.
#[cfg(test)]
pub(crate) fn assert_quit_declined(task: Task<Message>) {
    let mut driver = TaskDriver::new();
    let outputs = driver.outputs(task).expect("the task settles");
    assert_eq!(driver.cancelled_quits(), 1, "the quit was not cancelled");
    assert!(!driver.has_exited(), "the app exited over a declined quit");
    assert!(outputs.is_empty(), "a declined quit did more: {outputs:?}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::UiMsg;
    use crate::menu_bar::MenuMessage;

    fn noop() -> Message {
        Message::Ui(UiMsg::Noop)
    }

    #[test]
    fn the_boot_window_opens_and_reports_the_headless_size() {
        let (mut app, boot) = Signex::new();
        TaskDriver::new()
            .settle(&mut app, boot)
            .expect("boot settled");
        assert!(app.ui_state.main_window_opened, "MainWindowOpened arrived");
        assert_eq!(
            app.ui_state.window_size,
            (HEADLESS_WINDOW_SIZE.width, HEADLESS_WINDOW_SIZE.height)
        );
    }

    #[test]
    fn a_window_action_the_driver_does_not_perform_is_counted_and_an_answered_one_is_not() {
        let mut driver = TaskDriver::new();
        let id = iced::window::Id::unique();
        assert_eq!(driver.dropped_window_actions(), 0);
        let size = iced::window::size(id).then(|_| Task::done(noop()));
        assert_eq!(driver.outputs(size).expect("size").len(), 1);
        assert_eq!(
            driver.dropped_window_actions(),
            0,
            "a measured window is answered"
        );
        let drop_one = iced::window::maximize::<Message>(id, true);
        assert!(driver.outputs(drop_one).expect("maximize").is_empty());
        assert_eq!(driver.dropped_window_actions(), 1);
        let drop_two = iced::window::minimize::<Message>(id, true);
        assert!(driver.outputs(drop_two).expect("minimize").is_empty());
        assert_eq!(driver.dropped_window_actions(), 2);
    }

    #[test]
    fn a_clipboard_write_is_read_back() {
        let mut driver = TaskDriver::new();
        let write = iced::clipboard::write("copied".to_string()).discard();
        assert!(driver.outputs(write).expect("write").is_empty());
        assert_eq!(driver.clipboard_text(), Some("copied"));
        let read = iced::clipboard::read_text().then(|text| match text {
            Ok(text) if *text == "copied" => Task::done(noop()),
            _ => Task::none(),
        });
        assert_eq!(driver.outputs(read).expect("read").len(), 1);
    }

    #[test]
    fn an_empty_clipboard_answers_that_nothing_is_there() {
        let read = iced::clipboard::read_text().then(|text| match text {
            Err(ClipboardError::ContentNotAvailable) => Task::done(noop()),
            _ => Task::none(),
        });
        assert_eq!(TaskDriver::new().outputs(read).expect("read").len(), 1);
    }

    /// Messages run in the order they are produced, as the iced runtime
    /// delivers them: both of a batch, then their follow-ups in order.
    #[test]
    fn messages_run_first_in_first_out() {
        let tag = |n: u32| Message::Window(WindowMsg::NativeMenuFailed(n.to_string()));
        let mut seen = Vec::new();
        TaskDriver::new()
            .settle_with(
                |message| {
                    let Message::Window(WindowMsg::NativeMenuFailed(n)) = &message else {
                        return Task::none();
                    };
                    seen.push(n.clone());
                    match n.as_str() {
                        "1" => Task::done(tag(3)),
                        "2" => Task::done(tag(4)),
                        _ => Task::none(),
                    }
                },
                Task::batch([Task::done(tag(1)), Task::done(tag(2))]),
            )
            .expect("settled");
        // Depth-first would give 1 3 2 4, last-in-first-out 1 2 4 3.
        assert_eq!(seen, ["1", "2", "3", "4"]);
    }

    #[test]
    fn exit_ends_the_session() {
        let (mut app, _boot) = Signex::new();
        let mut driver = TaskDriver::new();
        driver
            .settle(&mut app, iced::exit::<Message>().chain(Task::done(noop())))
            .expect("settled");
        assert!(driver.has_exited());
        assert!(
            driver
                .outputs(Task::done(noop()))
                .expect("after exit")
                .is_empty()
        );
    }

    /// File > Exit closes the main window; its close event ends the session.
    #[test]
    fn file_exit_ends_a_booted_session() {
        let (mut app, boot) = Signex::new();
        let mut driver = TaskDriver::new();
        driver.settle(&mut app, boot).expect("boot settled");
        driver
            .send(&mut app, Message::Menu(MenuMessage::Exit))
            .expect("exit settled");
        assert!(driver.has_exited());
    }

    #[test]
    fn a_chain_past_the_message_cap_stops_with_an_error() {
        let (mut app, _boot) = Signex::new();
        let mut driver = TaskDriver::with_limits(DriverLimits {
            max_messages: 1,
            ..DriverLimits::default()
        });
        let two = Task::batch([Task::done(noop()), Task::done(noop())]);
        assert_eq!(driver.settle(&mut app, two), Err(DriveError::StepLimit(1)));
    }

    #[test]
    fn a_task_past_the_time_cap_stops_with_an_error() {
        let limit = Duration::from_millis(20);
        let mut driver = TaskDriver::with_limits(DriverLimits {
            max_duration: limit,
            ..DriverLimits::default()
        });
        let slow = Task::perform(
            async { tokio::time::sleep(Duration::from_secs(30)).await },
            |()| noop(),
        );
        assert!(matches!(driver.outputs(slow), Err(DriveError::TimedOut(l)) if l == limit));
    }
}
