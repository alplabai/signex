//! Drive a message and every task it produces to completion without an iced
//! runtime, for tests whose result lands on a background task.

use signex_app::app::{Message, Signex};

/// Send `message` and run every task it (transitively) produces to
/// completion, the way the iced runtime would - ERC and the Annotate preview
/// finish on background tasks, so a bare `update` leaves them pending.
pub fn settle(app: &mut Signex, message: Message) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut pending = vec![message];
    while let Some(message) = pending.pop() {
        let task = app.update(message);
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            continue;
        };
        runtime.block_on(async {
            while let Some(action) = futures::StreamExt::next(&mut stream).await {
                if let iced_runtime::Action::Output(next) = action {
                    pending.push(next);
                }
            }
        });
    }
}
