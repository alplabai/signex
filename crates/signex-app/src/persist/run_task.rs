//! Test helper: run a returned `Task` the way the iced runtime does.

use futures::StreamExt;

use crate::app::Message;

/// The messages `task` emits when it runs to completion, in order.
pub(crate) fn outputs(task: iced::Task<Message>) -> Vec<Message> {
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
