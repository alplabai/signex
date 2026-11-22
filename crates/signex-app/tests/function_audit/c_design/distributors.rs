//! Row 149: Preferences > Distributor APIs. The DigiKey and Mouser cards and
//! the order of preference. The browser sign-in and the live Mouser request
//! leave the machine, so their results are fed in as the workers send them.

use signex_app::library::messages::SettingsMsg;
use signex_app::library::settings::persistence::load_preferred_order;
use signex_app::preferences::PrefMsg;
use signex_library::DistributorSource as Source;

use super::support::{Prefs, opened};

fn removed() -> Result<(), String> {
    Ok(())
}

fn refused() -> Result<(), String> {
    Err("keyring locked".to_string())
}

fn send(prefs: &mut Prefs, msg: SettingsMsg) {
    prefs.settle(PrefMsg::LibrarySettings(msg));
}

fn on_order(prefs: &mut Prefs) {
    prefs.app.library.settings.preferred_order = vec![
        Source::DigiKey,
        Source::Mouser,
        Source::Lcsc,
        Source::Jlcpcb,
    ];
}

fn order(prefs: &Prefs) -> Vec<Source> {
    prefs.app.library.settings.preferred_order.clone()
}

fn connected(prefs: &mut Prefs) {
    prefs.app.library.settings.digikey_account_email = Some("j.doe@example.com".to_string());
}

// ---- DigiKey ----

#[test]
fn a_finished_sign_in_shows_the_connected_account() {
    let mut prefs = opened();
    let generation = prefs.app.library.settings.digikey_flow_generation;

    send(
        &mut prefs,
        SettingsMsg::DigiKeyOAuthResult {
            generation,
            connected_label: Some("j.doe@example.com".to_string()),
            error: None,
        },
    );

    let settings = &prefs.app.library.settings;
    assert_eq!(
        settings.digikey_account_email.as_deref(),
        Some("j.doe@example.com")
    );
    assert_eq!(
        settings.digikey_status.as_deref(),
        Some("Connected as j.doe@example.com")
    );
    assert!(!settings.digikey_in_flight);
}

#[test]
fn a_failed_sign_in_says_why_and_connects_nothing() {
    let mut prefs = opened();
    let generation = prefs.app.library.settings.digikey_flow_generation;

    send(
        &mut prefs,
        SettingsMsg::DigiKeyOAuthResult {
            generation,
            connected_label: None,
            error: Some("redirect timed out".to_string()),
        },
    );

    let settings = &prefs.app.library.settings;
    assert_eq!(settings.digikey_account_email, None);
    assert_eq!(
        settings.digikey_status.as_deref(),
        Some("Failed: redirect timed out")
    );
}

#[test]
fn cancel_during_a_sign_in_ends_it_and_says_so() {
    let mut prefs = opened();
    prefs.app.library.settings.digikey_in_flight = true;

    send(&mut prefs, SettingsMsg::DigiKeyCancel);

    let settings = &prefs.app.library.settings;
    assert!(!settings.digikey_in_flight);
    assert_eq!(settings.digikey_status.as_deref(), Some("Cancelled"));
}

#[test]
fn a_sign_in_that_was_cancelled_cannot_connect_afterwards() {
    let mut prefs = opened();
    let stale = prefs.app.library.settings.digikey_flow_generation;
    prefs.app.library.settings.digikey_token_remover = removed;
    prefs.app.library.settings.digikey_in_flight = true;
    send(&mut prefs, SettingsMsg::DigiKeyCancel);

    send(
        &mut prefs,
        SettingsMsg::DigiKeyOAuthResult {
            generation: stale,
            connected_label: Some("late@example.com".to_string()),
            error: None,
        },
    );

    assert_eq!(prefs.app.library.settings.digikey_account_email, None);
}

#[test]
fn disconnect_removes_the_account() {
    let mut prefs = opened();
    prefs.app.library.settings.digikey_token_remover = removed;
    connected(&mut prefs);

    send(&mut prefs, SettingsMsg::DigiKeyDisconnectRequested);

    let settings = &prefs.app.library.settings;
    assert_eq!(settings.digikey_account_email, None);
    assert_eq!(settings.digikey_disconnect_error, None);
    assert!(!settings.digikey_worker_busy);
}

#[test]
fn a_failed_disconnect_keeps_the_account_and_says_why() {
    let mut prefs = opened();
    prefs.app.library.settings.digikey_token_remover = refused;
    connected(&mut prefs);

    send(&mut prefs, SettingsMsg::DigiKeyDisconnectRequested);

    let settings = &prefs.app.library.settings;
    assert_eq!(
        settings.digikey_account_email.as_deref(),
        Some("j.doe@example.com")
    );
    assert_eq!(
        settings.digikey_disconnect_error.as_deref(),
        Some("keyring locked")
    );
}

#[test]
fn disconnect_waits_while_a_sign_in_is_running() {
    let mut prefs = opened();
    prefs.app.library.settings.digikey_token_remover = removed;
    connected(&mut prefs);
    prefs.app.library.settings.digikey_in_flight = true;

    send(&mut prefs, SettingsMsg::DigiKeyDisconnectRequested);

    assert_eq!(
        prefs.app.library.settings.digikey_account_email.as_deref(),
        Some("j.doe@example.com")
    );
}

// ---- Mouser ----

#[test]
fn test_with_no_key_is_refused_and_says_so() {
    let mut prefs = opened();
    prefs.app.library.settings.mouser_api_key_buf.clear();

    send(&mut prefs, SettingsMsg::MouserTest);

    let settings = &prefs.app.library.settings;
    assert!(!settings.mouser_in_flight);
    assert_eq!(
        settings.mouser_status.as_deref(),
        Some("Cannot test \u{2014} paste an API key first.")
    );
}

#[test]
fn a_passed_key_test_shows_key_verified() {
    let mut prefs = opened();

    send(&mut prefs, SettingsMsg::MouserTestResult(Ok(())));

    assert_eq!(prefs.app.library.settings.mouser_key_verified, Some(true));
}

#[test]
fn a_failed_key_test_says_why() {
    let mut prefs = opened();

    send(
        &mut prefs,
        SettingsMsg::MouserTestResult(Err("HTTP 401".to_string())),
    );

    let settings = &prefs.app.library.settings;
    assert_eq!(settings.mouser_key_verified, Some(false));
    assert_eq!(settings.mouser_status.as_deref(), Some("Failed: HTTP 401"));
}

#[test]
fn editing_the_key_clears_the_last_test_result() {
    let mut prefs = opened();
    send(&mut prefs, SettingsMsg::MouserTestResult(Ok(())));

    send(
        &mut prefs,
        SettingsMsg::MouserApiKeyChanged("NEW-KEY".to_string()),
    );

    let settings = &prefs.app.library.settings;
    assert_eq!(settings.mouser_api_key_buf, "NEW-KEY");
    assert_eq!(settings.mouser_key_verified, None);
    assert_eq!(settings.mouser_status, None);
}

#[test]
fn a_distributor_edit_is_not_a_preferences_draft() {
    let mut prefs = opened();

    send(
        &mut prefs,
        SettingsMsg::MouserApiKeyChanged("NEW-KEY".to_string()),
    );

    assert!(!prefs.is_dirty());
}

// ---- Order of preference ----

#[test]
fn dropping_a_row_gives_it_that_place_and_saves_the_order() {
    let mut prefs = opened();
    on_order(&mut prefs);

    send(&mut prefs, SettingsMsg::PreferenceDragStarted(Source::Lcsc));
    send(
        &mut prefs,
        SettingsMsg::PreferenceDragHovered(Source::DigiKey),
    );
    send(&mut prefs, SettingsMsg::PreferenceDropped(Source::DigiKey));

    let wanted = vec![
        Source::Lcsc,
        Source::DigiKey,
        Source::Mouser,
        Source::Jlcpcb,
    ];
    assert_eq!(order(&prefs), wanted);
    assert_eq!(prefs.app.library.settings.preferred_order_drag, None);
    assert_eq!(prefs.app.library.settings.preferred_order_error, None);
    assert_eq!(load_preferred_order(), wanted, "saved to distributors.toml");
}

#[test]
fn releasing_a_drag_off_the_rows_changes_nothing() {
    let mut prefs = opened();
    on_order(&mut prefs);
    let before = order(&prefs);

    send(&mut prefs, SettingsMsg::PreferenceDragStarted(Source::Lcsc));
    send(&mut prefs, SettingsMsg::PreferenceDragEnded);

    assert_eq!(order(&prefs), before);
    assert_eq!(prefs.app.library.settings.preferred_order_drag, None);
}

#[test]
fn a_drop_with_no_drag_changes_nothing() {
    let mut prefs = opened();
    on_order(&mut prefs);
    let before = order(&prefs);

    send(&mut prefs, SettingsMsg::PreferenceDropped(Source::Jlcpcb));

    assert_eq!(order(&prefs), before);
}
