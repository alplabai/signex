//! Drive an iced `Task` to completion the way the runtime would, for tests
//! that need the background work an `update()` hands back (#1351).

use futures::StreamExt as _;
use iced::Task;
use signex_app::app::{Message, Signex};

/// Run `task` on a fresh runtime and return the messages it produced.
pub fn run(task: Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

/// Feed every message `task` produces back through `update()`, following the
/// tasks those return, until nothing is left to run.
pub fn drive(app: &mut Signex, task: Task<Message>) {
    for message in run(task) {
        let follow_up = app.update(message);
        drive(app, follow_up);
    }
}
