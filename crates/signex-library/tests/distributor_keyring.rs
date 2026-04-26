//! Integration tests for `KeyringStore`.
//!
//! WS-C: round-trip test gated by platform availability. Windows Credential
//! Manager is available in tests; Linux/macOS CI runners often lack a
//! `Secret Service` daemon, so those tests run conditionally.

#![cfg(feature = "distributors-community")]

use signex_library::distributors::keyring::{KeyringError, KeyringStore};

fn entry_for(test_name: &str) -> KeyringStore {
    // Per-test username keeps parallel tests from clobbering each other.
    KeyringStore::for_provider("test-provider", &format!("ws-c-{test_name}"))
}

fn cleanup(store: &KeyringStore) {
    let _ = store.delete();
}

/// Helper: did the keyring backend even initialise? Many CI envs (Linux
/// without Secret Service, macOS sandboxed runners) return `Backend(_)` on
/// first set/get; we detect that and skip without failing the test.
fn keyring_available(store: &KeyringStore) -> bool {
    match store.set_secret("availability-probe") {
        Ok(()) => {
            let _ = store.delete();
            true
        }
        Err(KeyringError::Backend(_)) => false,
        Err(_) => true, // Real semantic error — let the actual test surface it.
    }
}

#[test]
fn keyring_round_trip() {
    let store = entry_for("round-trip");
    if !keyring_available(&store) {
        eprintln!("keyring backend unavailable on this platform — skipping");
        return;
    }

    store.set_secret("super-secret-token").unwrap();
    let got = store.get_secret().unwrap();
    assert_eq!(got, "super-secret-token");

    cleanup(&store);
}

#[test]
fn keyring_missing_entry_yields_not_found() {
    let store = entry_for("not-found");
    if !keyring_available(&store) {
        return;
    }
    // Ensure clean slate.
    let _ = store.delete();

    match store.get_secret() {
        Err(KeyringError::NotFound) => {}
        Ok(v) => panic!("expected NotFound, got Ok({v:?})"),
        Err(e) => panic!("expected NotFound, got Err({e:?})"),
    }
}

#[test]
fn keyring_delete_is_idempotent() {
    let store = entry_for("delete-idempotent");
    if !keyring_available(&store) {
        return;
    }
    // Deleting an absent entry must not error.
    store.delete().unwrap();
    store.set_secret("x").unwrap();
    store.delete().unwrap();
    // Second delete also OK.
    store.delete().unwrap();
}

#[test]
fn keyring_overwrite_replaces_value() {
    let store = entry_for("overwrite");
    if !keyring_available(&store) {
        return;
    }
    store.set_secret("first").unwrap();
    store.set_secret("second").unwrap();
    let got = store.get_secret().unwrap();
    assert_eq!(got, "second");
    cleanup(&store);
}

#[test]
fn service_name_format_matches_spec() {
    // Per WS-C: `signex-distributor-<provider>`. No network or backend touch.
    let store = KeyringStore::for_provider("digikey", "user1");
    assert_eq!(store.service_name(), "signex-distributor-digikey");
    assert_eq!(store.username(), "user1");
}
