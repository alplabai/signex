//! Drive an iced `Task` to completion the way the runtime would, for tests
//! that need the background work an `update()` hands back (#1351). It
//! delegates to `signex_app::task_driver` (#1927).

use iced::Task;
use signex_app::app::{Message, Signex};
use signex_app::task_driver::TaskDriver;

/// Feed every message `task` produces back through `update()`, following the
/// tasks those return, until nothing is left to run.
pub fn drive(app: &mut Signex, task: Task<Message>) {
    TaskDriver::new()
        .settle(app, task)
        .expect("the task chain settled");
}
