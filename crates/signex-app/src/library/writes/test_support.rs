//! Running the `Task` an `update` returns, for tests.

use crate::app::{Message, Signex};
use crate::library::messages::LibraryMessage;
use crate::persist::run_task::outputs;

/// Run `task` as the iced runtime would, feeding every message it produces
/// back through `update` and following the tasks those return.
pub(crate) fn run_task(app: &mut Signex, task: iced::Task<Message>) {
    for message in outputs(task) {
        let next = app.update(message);
        run_task(app, next);
    }
}

/// Run `task` and apply only the library writes it finishes, dropping what
/// those return (the refresh a write asks for stays unrun). For tests that
/// then land that refresh themselves.
pub(crate) fn run_write(app: &mut Signex, task: iced::Task<Message>) {
    for message in outputs(task) {
        if matches!(message, Message::Library(LibraryMessage::WriteFinished(_))) {
            drop(app.update(message));
        }
    }
}
