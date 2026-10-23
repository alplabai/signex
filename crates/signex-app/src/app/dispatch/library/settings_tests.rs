//! #834 review: a `DigiKeyOAuthResult` from a cancelled flow that still
//! carries `connected_label: Some` means the token was stored before the
//! cancel landed. The handler must request the keyring delete (off the
//! `update` thread, as a `Task`) and must not request it otherwise.

use std::sync::atomic::{AtomicUsize, Ordering};

use iced::futures::StreamExt;

use crate::app::{Message, Signex};
use crate::library::messages::{LibraryMessage, SettingsMsg};

static DELETE_REQUESTS: AtomicUsize = AtomicUsize::new(0);

fn recording_remover() -> Result<(), String> {
    DELETE_REQUESTS.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

fn result_message(generation: u64, connected_label: Option<&str>) -> SettingsMsg {
    SettingsMsg::DigiKeyOAuthResult {
        generation,
        connected_label: connected_label.map(str::to_string),
        error: None,
    }
}

/// Run every `Message` the task produces to completion on a throwaway
/// runtime, the way iced's executor would.
fn drain(task: iced::Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(m) = action {
                out.push(m);
            }
        }
        out
    })
}

#[test]
fn stale_connected_result_requests_the_token_delete_and_reports_it() {
    let (mut app, _startup) = Signex::new();
    app.library.settings.digikey_token_remover = recording_remover;
    app.library.settings.digikey_flow_generation = 5;
    let before = DELETE_REQUESTS.load(Ordering::SeqCst);

    // Not stale: the live flow's own result never deletes the token.
    let live = app.handle_library_settings_message(result_message(5, Some("me@example.com")));
    assert!(drain(live).is_empty(), "a live result schedules no task");
    assert_eq!(DELETE_REQUESTS.load(Ordering::SeqCst), before);

    // Stale + Cancelled/Failed outcome: nothing was stored, nothing to delete.
    let stale_none = app.handle_library_settings_message(result_message(1, None));
    assert!(drain(stale_none).is_empty());
    assert_eq!(DELETE_REQUESTS.load(Ordering::SeqCst), before);

    // Stale + Connected: the delete is requested, and its result comes
    // back as a message for the handler to log.
    let stale = app.handle_library_settings_message(result_message(1, Some("me@example.com")));
    assert_eq!(stale.units(), 1, "the delete must be a Task, not inline IO");
    let messages = drain(stale);
    assert_eq!(
        DELETE_REQUESTS.load(Ordering::SeqCst),
        before + 1,
        "the stale Connected branch must request the delete"
    );
    assert!(matches!(
        messages.as_slice(),
        [Message::Library(LibraryMessage::Settings(
            SettingsMsg::DigiKeyStaleTokenRemoved(Ok(()))
        ))]
    ));
}
