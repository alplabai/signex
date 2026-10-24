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

// #856: the keyring has one slot, so the stale cleanup is only safe while no
// second writer can exist. The fake keyring below stands in for that slot.
static FAKE_KEYRING: std::sync::Mutex<Option<&'static str>> = std::sync::Mutex::new(None);

fn clearing_remover() -> Result<(), String> {
    *FAKE_KEYRING.lock().expect("fake keyring") = None;
    Ok(())
}

fn keyring_slot() -> Option<&'static str> {
    *FAKE_KEYRING.lock().expect("fake keyring")
}

fn write_keyring(token: &'static str) {
    *FAKE_KEYRING.lock().expect("fake keyring") = Some(token);
}

/// Cancelled flow A already wrote its token; the user presses Connect again
/// before A's stale `Connected` is processed. B must not start, so B can
/// never write between A's write and A's cleanup, and B's token survives.
#[test]
fn a_second_connect_waits_for_the_cancelled_flows_cleanup_so_its_token_survives() {
    let (mut app, _startup) = Signex::new();
    app.library.settings.digikey_token_remover = clearing_remover;

    // Flow A starts (its worker task is never run here), then writes.
    drop(app.handle_library_settings_message(SettingsMsg::DigiKeyConnect));
    let generation_a = app.library.settings.digikey_flow_generation;
    write_keyring("token-a");
    drop(app.handle_library_settings_message(SettingsMsg::DigiKeyCancel));

    // Connect B while A's result is still outstanding: refused, no new flow.
    let refused = app.handle_library_settings_message(SettingsMsg::DigiKeyConnect);
    assert!(drain(refused).is_empty());
    assert!(!app.library.settings.digikey_in_flight);
    assert!(app.library.settings.digikey_worker_busy);
    assert!(
        app.library
            .settings
            .digikey_status
            .as_deref()
            .is_some_and(|s| s.contains("winding down")),
        "the refusal must be visible to the user"
    );

    // A's stale Connected arrives: delete scheduled, B still refused.
    let cleanup =
        app.handle_library_settings_message(result_message(generation_a, Some("DigiKey")));
    drop(app.handle_library_settings_message(SettingsMsg::DigiKeyConnect));
    assert!(
        !app.library.settings.digikey_in_flight,
        "B waits for the delete"
    );
    let messages = drain(cleanup);
    assert_eq!(keyring_slot(), None, "A's token was removed");
    for message in messages {
        if let Message::Library(LibraryMessage::Settings(m)) = message {
            drop(app.handle_library_settings_message(m));
        }
    }

    // Only now can B start; its token is written after A's cleanup.
    drop(app.handle_library_settings_message(SettingsMsg::DigiKeyConnect));
    assert!(app.library.settings.digikey_in_flight);
    write_keyring("token-b");
    let generation_b = app.library.settings.digikey_flow_generation;
    drop(app.handle_library_settings_message(result_message(generation_b, Some("DigiKey"))));
    assert_eq!(keyring_slot(), Some("token-b"), "B's token must survive");
}

/// A cancelled flow whose worker stored nothing frees the slot at once.
#[test]
fn a_cancelled_flow_that_stored_nothing_frees_connect() {
    let (mut app, _startup) = Signex::new();
    drop(app.handle_library_settings_message(SettingsMsg::DigiKeyConnect));
    let generation = app.library.settings.digikey_flow_generation;
    drop(app.handle_library_settings_message(SettingsMsg::DigiKeyCancel));
    assert!(app.library.settings.digikey_worker_busy);
    drop(app.handle_library_settings_message(result_message(generation, None)));
    assert!(!app.library.settings.digikey_worker_busy);
}
