//! Test helper: run a returned `Task` the way the iced runtime does.

use crate::app::Message;

/// The messages `task` emits when it runs to completion, in order.
pub(crate) fn outputs(task: iced::Task<Message>) -> Vec<Message> {
    crate::task_driver::TaskDriver::new()
        .outputs(task)
        .expect("the task finished")
}
