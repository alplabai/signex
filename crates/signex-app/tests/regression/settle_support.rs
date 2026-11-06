//! Drive a message and every task it produces to completion without an iced
//! runtime, for tests whose result lands on a background task.

use iced::Task;
use signex_app::app::{Message, Signex};
use signex_app::task_driver::TaskDriver;

/// Send `message` and run every task it (transitively) produces to
/// completion, the way the iced runtime would - ERC and the Annotate preview
/// finish on background tasks, so a bare `update` leaves them pending.
pub fn settle(app: &mut Signex, message: Message) {
    TaskDriver::new()
        .send(app, message)
        .expect("the task chain settled");
}

/// Run an already-returned `task` (and every task its messages produce) to
/// completion, feeding each message back through `update`.
pub fn settle_task(app: &mut Signex, task: Task<Message>) {
    TaskDriver::new()
        .settle(app, task)
        .expect("the task chain settled");
}

/// Mark the History panel as already showing the active tab, the state a
/// running app is in after its first refresh. The first `finish_update`
/// after a tab appears otherwise starts the panel's off-thread history
/// load (#1107), which a test counting or matching the tasks of one
/// message would see as an extra unit.
pub fn settle_history_panel(app: &mut Signex) {
    let d = &mut app.document_state;
    let path = d.tabs.get(d.active_tab).map(|t| t.path.clone());
    d.history.source_key = path.clone();
    d.history.active_path = path;
}

/// Show the active tab's history as a finished load leaves it, so a Restore
/// click is not refused for want of a loaded list.
pub fn list_active_history(app: &mut Signex) {
    let d = &mut app.document_state;
    d.history.active_path = d.tabs.get(d.active_tab).map(|tab| tab.path.clone());
    d.history.mode = signex_app::panels::history::HistoryRenderMode::Ready;
}
