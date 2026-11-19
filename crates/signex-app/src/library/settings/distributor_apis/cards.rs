//! The distributor cards: a head row (name, status pill, action) over a
//! caption or the API key form.

use iced::widget::{Space, column, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, raw, text, text_input};

use crate::styles::{DialogButtonKind, PillTone, dialog_button, tokens::resolve};

use super::{
    SP, TS, caption, card, error_caption, field_style, mono, semibold, settings_msg, status_pill,
};
use crate::library::messages::{LibraryMessage, SettingsMsg};
use crate::library::state::DistributorSettings;

/// Width of the "API Key" label column of the Mouser form.
const KEY_LABEL_W: f32 = 64.0;

/// What the DigiKey card shows, from the settings alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DigiKeyCard<'a> {
    /// The OAuth flow is running: "Connecting…" and Cancel.
    Connecting,
    /// An account is connected: "Connected as …" and Disconnect.
    Connected(&'a str),
    /// No client id was set at start: the error state, Connect disabled.
    NotConfigured,
    /// Nothing connected: "Not connected" and Connect via OAuth.
    NotConnected,
}

impl<'a> DigiKeyCard<'a> {
    pub(super) fn of(settings: &'a DistributorSettings) -> Self {
        if settings.digikey_in_flight {
            Self::Connecting
        } else if let Some(account) = settings.digikey_account_email.as_deref() {
            Self::Connected(account)
        } else if !settings.digikey_client_configured {
            Self::NotConfigured
        } else {
            Self::NotConnected
        }
    }
}

/// The DigiKey card.
pub(super) fn digikey<'a>(
    settings: &'a DistributorSettings,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let help = || caption(t!(i18n, ids::LIBRARY_DIGIKEY_HELP), tokens);
    let status = || -> Option<Label<'a>> {
        let status = settings.digikey_status.as_deref()?;
        // raw: the OAuth status is set by its handler (#2061: handler-worded state text).
        Some(raw(status))
    };
    let (pill, action, lines): (_, _, Vec<Element<'a, LibraryMessage>>) =
        match DigiKeyCard::of(settings) {
            DigiKeyCard::Connecting => (
                status_pill(
                    t!(i18n, ids::LIBRARY_DIGIKEY_CONNECTING),
                    PillTone::Neutral,
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                    DialogButtonKind::Secondary,
                    Some(settings_msg(SettingsMsg::DigiKeyCancel)),
                    tokens,
                ),
                vec![status().map_or_else(help, |s| caption(s, tokens))],
            ),
            DigiKeyCard::Connected(account) => {
                let mut lines = vec![help()];
                if let Some(error) = settings.digikey_disconnect_error.as_deref() {
                    lines.push(error_caption(
                        t!(i18n, ids::LIBRARY_DIGIKEY_DISCONNECT_FAILED, "error" => error),
                        tokens,
                    ));
                }
                (
                    status_pill(
                        t!(i18n, ids::LIBRARY_DIGIKEY_CONNECTED_AS, "account" => account),
                        PillTone::Ok,
                        tokens,
                    ),
                    dialog_button(
                        t!(i18n, ids::LIBRARY_DIGIKEY_DISCONNECT),
                        DialogButtonKind::Secondary,
                        (!settings.digikey_worker_busy)
                            .then(|| settings_msg(SettingsMsg::DigiKeyDisconnectRequested)),
                        tokens,
                    ),
                    lines,
                )
            }
            DigiKeyCard::NotConfigured => (
                status_pill(
                    t!(i18n, ids::LIBRARY_DIGIKEY_NOT_CONFIGURED),
                    PillTone::Err,
                    tokens,
                ),
                connect_button(None, tokens, i18n),
                vec![error_caption(
                    t!(i18n, ids::LIBRARY_DIGIKEY_NOT_CONFIGURED_HELP),
                    tokens,
                )],
            ),
            DigiKeyCard::NotConnected => (
                status_pill(
                    t!(i18n, ids::LIBRARY_DIGIKEY_NOT_CONNECTED),
                    PillTone::Neutral,
                    tokens,
                ),
                connect_button(
                    Some(settings_msg(SettingsMsg::DigiKeyConnect)),
                    tokens,
                    i18n,
                ),
                vec![status().map_or_else(help, |s| caption(s, tokens))],
            ),
        };
    // raw: distributor names are product names, the same in every language.
    let mut body = column![head(raw("DigiKey"), pill, Some(action))].spacing(SP.s);
    for line in lines {
        body = body.push(line);
    }
    card(body, tokens)
}

fn connect_button<'a>(
    on_press: Option<LibraryMessage>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    dialog_button(
        t!(i18n, ids::LIBRARY_DISTRIBUTOR_CONNECT_OAUTH),
        DialogButtonKind::Primary,
        on_press,
        tokens,
    )
}

/// The Mouser card: the key's test state and the API key form.
pub(super) fn mouser<'a>(
    settings: &'a DistributorSettings,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let (pill_label, tone) = match (settings.mouser_in_flight, settings.mouser_key_verified) {
        (true, _) => (ids::LIBRARY_MOUSER_TESTING, PillTone::Neutral),
        (false, Some(true)) => (ids::LIBRARY_MOUSER_VERIFIED, PillTone::Ok),
        (false, Some(false)) => (ids::LIBRARY_MOUSER_TEST_FAILED, PillTone::Err),
        (false, None) => (ids::LIBRARY_MOUSER_NOT_TESTED, PillTone::Neutral),
    };
    // The input is bound to the real buffer; `secure(true)` only masks the
    // glyphs, so a keystroke never writes the bullets back into the key.
    let key_field = text_input(
        t!(i18n, ids::LIBRARY_MOUSER_KEY_PLACEHOLDER),
        &settings.mouser_api_key_buf,
    )
    .secure(true)
    .on_input(|s| settings_msg(SettingsMsg::MouserApiKeyChanged(s)))
    .font(mono())
    .size(TS.body)
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .style(field_style(tokens));
    let form = row![
        text(t!(i18n, ids::LIBRARY_MOUSER_API_KEY))
            .size(TS.body)
            .color(resolve(tokens).ink3)
            .width(KEY_LABEL_W),
        key_field,
        dialog_button(
            t!(i18n, ids::LIBRARY_DISTRIBUTOR_TEST),
            DialogButtonKind::Secondary,
            Some(settings_msg(SettingsMsg::MouserTest)),
            tokens,
        ),
    ]
    .spacing(SP.m + SP.xs)
    .align_y(Alignment::Center);
    // raw: distributor names are product names, the same in every language.
    let mut body = column![
        head(
            raw("Mouser"),
            status_pill(t!(i18n, pill_label), tone, tokens),
            None
        ),
        form
    ]
    .spacing(SP.s);
    if let Some(status) = settings.mouser_status.as_deref() {
        // raw: the key-test status is set by its handler (#2061: handler-worded state text).
        body = body.push(caption(raw(status), tokens));
    }
    card(body, tokens)
}

/// The LCSC and JLCPCB card: public lookups, nothing to set up.
pub(super) fn no_key<'a>(tokens: &'a ThemeTokens, i18n: &I18n) -> Element<'a, LibraryMessage> {
    // raw: distributor names are product names, the same in every language.
    let body = column![
        head(
            raw("LCSC, JLCPCB"),
            status_pill(
                t!(i18n, ids::LIBRARY_NO_KEY_REQUIRED),
                PillTone::Info,
                tokens
            ),
            None
        ),
        caption(t!(i18n, ids::LIBRARY_NO_KEY_HELP), tokens),
    ]
    .spacing(SP.s);
    card(body, tokens)
}

/// A card's head row: the name in semibold, its pill, then the action at
/// the right end.
fn head<'a>(
    name: Label<'a>,
    pill: Element<'a, LibraryMessage>,
    action: Option<Element<'a, LibraryMessage>>,
) -> Element<'a, LibraryMessage> {
    let mut head = row![text(name).size(TS.label).font(semibold()), pill]
        .spacing(SP.m)
        .align_y(Alignment::Center);
    if let Some(action) = action {
        head = head
            .push(Space::new().width(Length::Fill).boxed())
            .push(action);
    }
    head.boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> DistributorSettings {
        DistributorSettings {
            digikey_client_configured: true,
            ..DistributorSettings::default()
        }
    }

    #[test]
    fn the_digikey_card_follows_the_flow_then_the_account_then_the_client_id() {
        let mut s = settings();
        assert_eq!(DigiKeyCard::of(&s), DigiKeyCard::NotConnected);

        // The card and the OAuth flow share one test of the client id.
        assert!(!crate::library::settings::digikey_oauth::client_id_configured(""));
        assert!(crate::library::settings::digikey_oauth::client_id_configured("id"));
        s.digikey_client_configured = false;
        assert_eq!(
            DigiKeyCard::of(&s),
            DigiKeyCard::NotConfigured,
            "a missing client id is the card's error state"
        );

        s.digikey_account_email = Some("me@example.com".into());
        assert_eq!(
            DigiKeyCard::of(&s),
            DigiKeyCard::Connected("me@example.com")
        );

        s.digikey_in_flight = true;
        assert_eq!(DigiKeyCard::of(&s), DigiKeyCard::Connecting);
    }
}
