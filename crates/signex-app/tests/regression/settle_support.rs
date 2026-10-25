//! Drive a message and every task it produces to completion without an iced
//! runtime, for tests whose result lands on a background task.

use iced::Task;
use signex_app::app::{Message, Signex};

/// Send `message` and run every task it (transitively) produces to
/// completion, the way the iced runtime would - ERC and the Annotate preview
/// finish on background tasks, so a bare `update` leaves them pending.
pub fn settle(app: &mut Signex, message: Message) {
    let task = app.update(message);
    settle_task(app, task);
}

/// Run an already-returned `task` (and every task its messages produce) to
/// completion, feeding each message back through `update`.
pub fn settle_task(app: &mut Signex, task: Task<Message>) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut pending = vec![task];
    while let Some(task) = pending.pop() {
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            continue;
        };
        let mut messages = Vec::new();
        runtime.block_on(async {
            while let Some(action) = futures::StreamExt::next(&mut stream).await {
                if let iced_runtime::Action::Output(next) = action {
                    messages.push(next);
                }
            }
        });
        pending.extend(messages.into_iter().map(|m| app.update(m)));
    }
}
