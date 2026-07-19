//! Library settings dispatcher — distributor / API credential
//! settings (`SettingsMsg`), including DigiKey OAuth and persistence.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;

impl Signex {
    pub(super) fn handle_library_settings_message(&mut self, msg: SettingsMsg) -> Task<Message> {
        use crate::library::settings::digikey_oauth;
        use signex_library::distributor::DistributorAdapter;
        use signex_library::distributors::digikey::{DIGIKEY_AUTH_URL, DIGIKEY_TOKEN_URL};
        use signex_library::distributors::keyring::KeyringStore;
        use signex_library::distributors::mouser::MouserAdapter;

        match msg {
            SettingsMsg::DigiKeyConnect => {
                if self.library.settings.digikey_in_flight {
                    return Task::none();
                }
                // Bump the generation BEFORE spawning so the worker
                // captures the current value. `DigiKeyOAuthResult`
                // discards messages whose generation is stale — i.e.
                // belonged to a cancelled flow that's only now winding
                // down. Without this, Cancel + reconnect lets the
                // first worker's outcome stomp on the second flow's
                // state.
                self.library.settings.digikey_flow_generation = self
                    .library
                    .settings
                    .digikey_flow_generation
                    .wrapping_add(1);
                let generation = self.library.settings.digikey_flow_generation;
                self.library.settings.digikey_in_flight = true;
                self.library.settings.digikey_status = Some("Waiting for browser…".to_string());
                let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                self.library.settings.digikey_cancel = Some(cancel_flag.clone());
                let (client_id, client_secret) = digikey_oauth::read_env_credentials();
                let auth_url = DIGIKEY_AUTH_URL.to_string();
                let token_url = DIGIKEY_TOKEN_URL.to_string();
                return Task::perform(
                    async move {
                        let cancel = digikey_oauth::CancelHandle::from_flag(cancel_flag);
                        tokio::task::spawn_blocking(move || {
                            digikey_oauth::run_blocking(
                                client_id,
                                client_secret,
                                auth_url,
                                token_url,
                                cancel,
                                true,
                            )
                        })
                        .await
                        .unwrap_or(digikey_oauth::Outcome::Failed {
                            reason: "worker thread panicked".into(),
                        })
                    },
                    move |outcome| {
                        let (label, err) = match outcome {
                            digikey_oauth::Outcome::Connected { account_label } => {
                                (Some(account_label), None)
                            }
                            digikey_oauth::Outcome::Failed { reason } => (None, Some(reason)),
                            digikey_oauth::Outcome::Cancelled => (None, None),
                        };
                        Message::Library(LibraryMessage::Settings(
                            SettingsMsg::DigiKeyOAuthResult {
                                generation,
                                connected_label: label,
                                error: err,
                            },
                        ))
                    },
                );
            }
            SettingsMsg::DigiKeyCancel => {
                if let Some(flag) = self.library.settings.digikey_cancel.as_ref() {
                    flag.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                // Bump the generation so any in-flight worker's eventual
                // result is treated as stale by the result handler.
                // Then clear the in-flight flag — the user is now free
                // to start a fresh OAuth attempt without the old
                // worker's outcome leaking into the new flow.
                self.library.settings.digikey_flow_generation = self
                    .library
                    .settings
                    .digikey_flow_generation
                    .wrapping_add(1);
                self.library.settings.digikey_cancel = None;
                self.library.settings.digikey_in_flight = false;
                self.library.settings.digikey_status = Some("Cancelled".to_string());
            }
            SettingsMsg::DigiKeyOAuthResult {
                generation,
                connected_label,
                error,
            } => {
                // Drop stale results from a cancelled flow — see the
                // comment on `DigiKeyConnect` for why.
                if generation != self.library.settings.digikey_flow_generation {
                    return Task::none();
                }
                self.library.settings.digikey_in_flight = false;
                self.library.settings.digikey_cancel = None;
                match (connected_label, error) {
                    (Some(label), _) => {
                        self.library.settings.digikey_account_email = Some(label.clone());
                        self.library.settings.digikey_status =
                            Some(format!("Connected as {label}"));
                    }
                    (_, Some(reason)) => {
                        self.library.settings.digikey_status = Some(format!("Failed: {reason}"));
                    }
                    (None, None) => {
                        self.library.settings.digikey_status = Some("Cancelled".to_string());
                    }
                }
            }
            SettingsMsg::MouserApiKeyChanged(s) => {
                self.library.settings.mouser_api_key_buf = s;
            }
            SettingsMsg::MouserTest => {
                if self.library.settings.mouser_in_flight {
                    return Task::none();
                }
                let key = self.library.settings.mouser_api_key_buf.clone();
                if key.is_empty() {
                    self.library.settings.mouser_status =
                        Some("Cannot test — paste an API key first.".to_string());
                    return Task::none();
                }
                self.library.settings.mouser_in_flight = true;
                self.library.settings.mouser_status = Some("Testing…".to_string());
                let key_for_writeback = key.clone();
                return Task::perform(
                    async move {
                        let key_for_test = key.clone();
                        tokio::task::spawn_blocking(move || {
                            const SENTINEL_MPN: &str = "RC0805FR-0710KL";
                            let adapter = MouserAdapter::with_api_key(
                                "https://api.mouser.com/api/v1/search/keyword",
                                key_for_test,
                                None,
                            );
                            adapter
                                .lookup_by_mpn(SENTINEL_MPN)
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("worker thread panicked: {e}")))
                    },
                    move |result| {
                        let result = match result {
                            Ok(()) => {
                                // MD-17: surface keyring backend
                                // unavailability instead of panicking
                                // — the dialog will tell the user to
                                // install libsecret or run with the
                                // env-var auth flow.
                                match KeyringStore::for_provider("mouser", "default") {
                                    Ok(store) => {
                                        if let Err(e) = store.set_secret(&key_for_writeback) {
                                            Err(format!(
                                                "API key valid, but keyring write failed: {e}"
                                            ))
                                        } else {
                                            Ok(())
                                        }
                                    }
                                    Err(e) => Err(format!(
                                        "API key valid, but OS keychain unavailable: {e}"
                                    )),
                                }
                            }
                            Err(e) => Err(e),
                        };
                        Message::Library(LibraryMessage::Settings(SettingsMsg::MouserTestResult(
                            result,
                        )))
                    },
                );
            }
            SettingsMsg::MouserTestResult(result) => {
                self.library.settings.mouser_in_flight = false;
                self.library.settings.mouser_status = Some(match result {
                    Ok(()) => "\u{2713} Connected & key saved to keyring.".to_string(),
                    Err(e) => format!("Failed: {e}"),
                });
            }
            SettingsMsg::PreferenceUp(src) => self.swap_preferred_order(src, true),
            SettingsMsg::PreferenceDown(src) => self.swap_preferred_order(src, false),
        }
        Task::none()
    }

    /// Move `src` one slot up (or down) the preferred-order list and
    /// persist. A save failure lands in `preferred_order_error` so the
    /// panel can show it inline — silently swallowing it left the user
    /// to discover the reverted order on next launch.
    fn swap_preferred_order(&mut self, src: signex_library::DistributorSource, up: bool) {
        let order = &mut self.library.settings.preferred_order;
        let Some(i) = order.iter().position(|s| *s == src) else {
            return;
        };
        let target = if up {
            if i == 0 {
                return;
            }
            i - 1
        } else {
            if i + 1 >= order.len() {
                return;
            }
            i + 1
        };
        order.swap(i, target);
        let saved = crate::library::settings::persistence::save_preferred_order(order);
        self.library.settings.preferred_order_error = saved.err();
    }
}
