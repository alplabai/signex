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

/// Connected, then a reconnect cancelled after its token was stored: the
/// cleanup empties the keyring, so the panel must stop saying "Connected".
#[test]
fn removing_the_stale_token_clears_the_connected_label() {
    let (mut app, _startup) = Signex::new();
    app.library.settings.digikey_account_email = Some("DigiKey".to_string());
    app.library.settings.digikey_status = Some("Connected as DigiKey".to_string());
    app.library.settings.digikey_worker_busy = true;

    drop(app.handle_library_settings_message(SettingsMsg::DigiKeyStaleTokenRemoved(Ok(()))));

    assert_eq!(app.library.settings.digikey_account_email, None);
    assert_eq!(
        app.library.settings.digikey_status.as_deref(),
        Some("Not connected")
    );

    // A failed delete leaves the token in place, so the label stays.
    app.library.settings.digikey_account_email = Some("DigiKey".to_string());
    drop(
        app.handle_library_settings_message(SettingsMsg::DigiKeyStaleTokenRemoved(Err(
            "locked".to_string()
        ))),
    );
    assert_eq!(
        app.library.settings.digikey_account_email.as_deref(),
        Some("DigiKey")
    );
}

#[test]
fn the_busy_refusal_is_one_clean_sentence() {
    let (mut app, _startup) = Signex::new();
    app.library.settings.digikey_worker_busy = true;

    drop(app.handle_library_settings_message(SettingsMsg::DigiKeyConnect));

    let status = app.library.settings.digikey_status.expect("status");
    assert!(!status.contains("  "), "no run of spaces: {status:?}");
    assert!(status.contains("winding down; try Connect"), "{status:?}");
}

// Disconnect deletes the stored token through the same remover seam.
static DISCONNECT_DELETES: AtomicUsize = AtomicUsize::new(0);

fn counting_remover() -> Result<(), String> {
    DISCONNECT_DELETES.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

fn connected_app() -> Signex {
    let (mut app, _startup) = Signex::new();
    app.library.settings.digikey_account_email = Some("me@example.com".to_string());
    app.library.settings.digikey_status = Some("Connected as me@example.com".to_string());
    app
}

#[test]
fn disconnect_removes_the_token_off_the_update_thread_then_clears_the_account() {
    let mut app = connected_app();
    app.library.settings.digikey_token_remover = counting_remover;
    let before = DISCONNECT_DELETES.load(Ordering::SeqCst);

    let task = app.handle_library_settings_message(SettingsMsg::DigiKeyDisconnectRequested);
    assert_eq!(task.units(), 1, "the delete must be a Task, not inline IO");
    assert_eq!(DISCONNECT_DELETES.load(Ordering::SeqCst), before);
    assert!(
        app.library.settings.digikey_worker_busy,
        "the keyring slot is held until the delete is done"
    );
    let again = app.handle_library_settings_message(SettingsMsg::DigiKeyDisconnectRequested);
    assert!(drain(again).is_empty(), "a second press starts nothing");

    let messages = drain(task);
    assert_eq!(DISCONNECT_DELETES.load(Ordering::SeqCst), before + 1);
    assert!(matches!(
        messages.as_slice(),
        [Message::Library(LibraryMessage::Settings(
            SettingsMsg::DigiKeyDisconnected(Ok(()))
        ))]
    ));
    for message in messages {
        if let Message::Library(LibraryMessage::Settings(m)) = message {
            drop(app.handle_library_settings_message(m));
        }
    }
    assert_eq!(app.library.settings.digikey_account_email, None);
    assert_eq!(app.library.settings.digikey_status, None);
    assert!(!app.library.settings.digikey_worker_busy);
}

#[test]
fn a_failed_disconnect_keeps_the_account_and_says_why() {
    let mut app = connected_app();
    app.library.settings.digikey_worker_busy = true;

    drop(
        app.handle_library_settings_message(SettingsMsg::DigiKeyDisconnected(Err(
            "keyring locked".to_string(),
        ))),
    );

    assert_eq!(
        app.library.settings.digikey_account_email.as_deref(),
        Some("me@example.com"),
        "the token is still stored, so the account stays"
    );
    assert_eq!(
        app.library.settings.digikey_disconnect_error.as_deref(),
        Some("keyring locked")
    );
    assert!(!app.library.settings.digikey_worker_busy);
}

#[test]
fn disconnect_waits_while_a_connect_is_running() {
    let mut app = connected_app();
    app.library.settings.digikey_in_flight = true;
    let task = app.handle_library_settings_message(SettingsMsg::DigiKeyDisconnectRequested);
    assert!(drain(task).is_empty());
    assert!(app.library.settings.digikey_account_email.is_some());
}

/// Dropping a dragged distributor on a row moves it there and saves
/// the order the way Up / Down do. One test, since both write the shared
/// `distributors.toml` of this test process.
#[test]
fn a_dropped_distributor_takes_the_row_and_the_order_is_saved_like_up_down() {
    use signex_library::DistributorSource::{DigiKey, Jlcpcb, Lcsc, Mouser};
    let (mut app, _startup) = Signex::new();
    app.library.settings.preferred_order = vec![DigiKey, Mouser, Lcsc, Jlcpcb];

    // Hovering without a drag marks nothing.
    drop(app.handle_library_settings_message(SettingsMsg::PreferenceDragHovered(Lcsc)));
    assert_eq!(app.library.settings.preferred_order_drop_target, None);

    drop(app.handle_library_settings_message(SettingsMsg::PreferenceDragStarted(Jlcpcb)));
    drop(app.handle_library_settings_message(SettingsMsg::PreferenceDragHovered(Mouser)));
    assert_eq!(
        app.library.settings.preferred_order_drop_target,
        Some(Mouser)
    );
    drop(app.handle_library_settings_message(SettingsMsg::PreferenceDropped(Mouser)));

    let moved = vec![DigiKey, Jlcpcb, Mouser, Lcsc];
    assert_eq!(app.library.settings.preferred_order, moved);
    assert_eq!(app.library.settings.preferred_order_drag, None);
    assert_eq!(app.library.settings.preferred_order_drop_target, None);
    assert_eq!(app.library.settings.preferred_order_error, None);
    assert_eq!(
        crate::library::settings::persistence::load_preferred_order(),
        moved,
        "saved to distributors.toml"
    );

    // Up keeps saving through the same path.
    drop(app.handle_library_settings_message(SettingsMsg::PreferenceUp(Lcsc)));
    let up = vec![DigiKey, Jlcpcb, Lcsc, Mouser];
    assert_eq!(app.library.settings.preferred_order, up);
    assert_eq!(
        crate::library::settings::persistence::load_preferred_order(),
        up
    );

    // A release off the list ends the drag; a drop with no drag moves nothing.
    drop(app.handle_library_settings_message(SettingsMsg::PreferenceDragStarted(DigiKey)));
    drop(app.handle_library_settings_message(SettingsMsg::PreferenceDragEnded));
    drop(app.handle_library_settings_message(SettingsMsg::PreferenceDropped(Mouser)));
    assert_eq!(app.library.settings.preferred_order, up);
    assert_eq!(app.library.settings.preferred_order_drag, None);
}

/// The Mouser pill reads the last test; an edited key is untested again.
#[test]
fn the_mouser_test_outcome_is_kept_until_the_key_changes() {
    let (mut app, _startup) = Signex::new();
    app.library.settings.mouser_in_flight = true;
    drop(app.handle_library_settings_message(SettingsMsg::MouserTestResult(Ok(()))));
    assert_eq!(app.library.settings.mouser_key_verified, Some(true));

    drop(app.handle_library_settings_message(SettingsMsg::MouserApiKeyChanged("k2".into())));
    assert_eq!(app.library.settings.mouser_key_verified, None);
    assert_eq!(app.library.settings.mouser_status, None);
    assert_eq!(app.library.settings.mouser_api_key_buf, "k2");
}
