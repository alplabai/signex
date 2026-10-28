//! Settings → Library → Distributor APIs panel.
//!
//! - DigiKey: OAuth2 PKCE via the Connect button (see `digikey_oauth`);
//!   client id/secret come from `SIGNEX_DIGIKEY_CLIENT_ID` /
//!   `SIGNEX_DIGIKEY_CLIENT_SECRET`, tokens are kept in the keyring.
//! - Mouser: API key field + a Test button; a verified key is saved to the keyring.
//! - LCSC, JLCPCB: no key required.
//! - Order-of-preference list — persisted to `distributors.toml`
//!   (see `persistence`).

use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_library::DistributorSource;
use signex_types::theme::ThemeTokens;

use crate::styles::{HAIRLINE, Spacing, TypeScale};
use signex_widgets::theme_ext;

use super::super::messages::{LibraryMessage, SettingsMsg};
use super::super::state::DistributorSettings;

/// Render the Distributor APIs panel. Mounts inside the existing
/// Preferences modal under a Library section.
pub fn view<'a>(
    settings: &'a DistributorSettings,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let hover_bg = crate::styles::tokens::hover_overlay(tokens);

    let section_title = |title: &'static str| -> Element<'_, LibraryMessage> {
        text(title).size(ts.body).color(muted).into()
    };

    // ── DigiKey ──────────────────────────────────────────────
    let digikey_status = match (
        settings.digikey_in_flight,
        settings.digikey_status.as_deref(),
        settings.digikey_account_email.as_deref(),
    ) {
        (true, Some(s), _) => s.to_string(),
        // In-flight with no status set yet — never let "Not connected"
        // render while the Cancel button is also visible.
        (true, None, _) => "Connecting…".to_string(),
        (false, _, Some(label)) => format!("Connected as {label}"),
        (false, Some(s), _) => s.to_string(),
        _ => "Not connected".to_string(),
    };
    let mut digikey_actions =
        row![text(digikey_status).size(ts.body).color(text_c)].align_y(iced::Alignment::Center);
    digikey_actions = digikey_actions.push(Space::new().width(Length::Fill));
    if settings.digikey_in_flight {
        digikey_actions = digikey_actions.push(secondary_btn(
            "Cancel",
            LibraryMessage::Settings(SettingsMsg::DigiKeyCancel),
            tokens,
        ));
    } else {
        digikey_actions = digikey_actions.push(primary_btn(
            "Connect via OAuth",
            LibraryMessage::Settings(SettingsMsg::DigiKeyConnect),
            tokens,
        ));
    }
    let digikey_section = column![
        section_title("DigiKey"),
        Space::new().height(sp.s),
        digikey_actions,
        Space::new().height(sp.s),
        text(
            "OAuth2 PKCE flow — opens your browser, then stores the refresh token in the OS \
             keyring. Set SIGNEX_DIGIKEY_CLIENT_ID + SIGNEX_DIGIKEY_CLIENT_SECRET first."
        )
        .size(ts.caption)
        .color(muted),
    ]
    .spacing(sp.xs);

    // ── Mouser ──────────────────────────────────────────────
    // The text_input must be bound to the real `mouser_api_key_buf` —
    // the previous version bound it to a `•`-only `masked` string,
    // which meant the first keystroke fired `MouserApiKeyChanged`
    // with the bullet string and permanently overwrote the real key.
    // `secure(true)` is the iced-supplied mask — the buffer stays
    // cleartext, only the rendered glyphs are bullets.
    let mouser_test_status = settings
        .mouser_status
        .clone()
        .unwrap_or_else(|| "Not tested".to_string());
    let mouser_section = column![
        section_title("Mouser"),
        Space::new().height(sp.s),
        row![
            text("API Key")
                .size(ts.body)
                .color(text_c)
                .width(Length::Fixed(96.0)),
            text_input("Paste API key…", &settings.mouser_api_key_buf)
                .secure(true)
                .on_input(|s| LibraryMessage::Settings(SettingsMsg::MouserApiKeyChanged(s)))
                .padding([sp.s, sp.m])
                .size(ts.label),
            Space::new().width(sp.m),
            primary_btn(
                "Test",
                LibraryMessage::Settings(SettingsMsg::MouserTest),
                tokens
            ),
        ]
        .align_y(iced::Alignment::Center),
        Space::new().height(sp.s),
        text(mouser_test_status).size(ts.caption).color(muted),
    ]
    .spacing(sp.xs);

    // ── LCSC + JLCPCB ──────────────────────────────────────
    let no_key_section = column![
        section_title("LCSC, JLCPCB"),
        Space::new().height(sp.s),
        text("(no key required)").size(ts.body).color(text_c),
    ]
    .spacing(sp.xs);

    // ── Preference order ───────────────────────────────────
    let mut order_col = column![
        section_title("Order of Preference"),
        Space::new().height(sp.s)
    ]
    .spacing(sp.xs);
    let len = settings.preferred_order.len();
    for (i, src) in settings.preferred_order.iter().enumerate() {
        let label = distributor_label(*src);
        let up_enabled = i > 0;
        let down_enabled = i + 1 < len;
        let mut up_btn =
            button(container(text("▲").size(ts.caption).color(text_c)).padding([sp.xs, sp.s]))
                .padding(0)
                .style(crate::styles::outline_button(tokens));
        if up_enabled {
            up_btn = up_btn.on_press(LibraryMessage::Settings(SettingsMsg::PreferenceUp(*src)));
        }
        let mut down_btn =
            button(container(text("▼").size(ts.caption).color(text_c)).padding([sp.xs, sp.s]))
                .padding(0)
                .style(crate::styles::outline_button(tokens));
        if down_enabled {
            down_btn =
                down_btn.on_press(LibraryMessage::Settings(SettingsMsg::PreferenceDown(*src)));
        }
        let row_widget = row![
            text(format!("{}.  {}", i + 1, label))
                .size(ts.body)
                .color(text_c)
                .width(Length::Fill),
            up_btn,
            Space::new().width(sp.s),
            down_btn,
        ]
        .padding([sp.xs, sp.s])
        .align_y(iced::Alignment::Center);
        let bg_row = container(row_widget).style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(hover_bg)),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..Default::default()
        });
        order_col = order_col.push(bg_row);
    }
    order_col = order_col.push(
        text(
            "Saved to <config_dir>/signex/distributors.toml. The first matching adapter is \
             tried first when you paste a distributor URL into the Supply tab.",
        )
        .size(ts.caption)
        .color(muted),
    );
    // A failed save must be visible here and now — otherwise the reorder
    // looks applied and silently reverts on next launch.
    if let Some(error) = settings.preferred_order_error.as_deref() {
        order_col = order_col.push(
            text(format!("Could not save the order: {error}"))
                .size(ts.caption)
                .color(crate::styles::tokens::danger(tokens)),
        );
    }

    container(
        column![
            digikey_section,
            divider(border),
            mouser_section,
            divider(border),
            no_key_section,
            divider(border),
            order_col,
        ]
        .spacing(sp.l)
        .width(Length::Fill),
    )
    .padding(sp.l)
    .style(crate::styles::modal_card(tokens))
    .into()
}

fn distributor_label(src: DistributorSource) -> &'static str {
    match src {
        DistributorSource::DigiKey => "DigiKey",
        DistributorSource::Mouser => "Mouser",
        DistributorSource::Lcsc => "LCSC",
        DistributorSource::Jlcpcb => "JLCPCB",
        DistributorSource::Octopart => "Octopart",
        DistributorSource::Oemsecrets => "OEMsecrets",
        DistributorSource::Other => "Other",
    }
}

fn divider(color: iced::Color) -> Element<'static, LibraryMessage> {
    container(Space::new().height(HAIRLINE).width(Length::Fill))
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(color)),
            ..Default::default()
        })
        .into()
}

fn primary_btn<'a>(
    label: &'a str,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let on_accent = crate::styles::tokens::on_accent(tokens);
    button(container(text(label.to_string()).size(ts.body).color(on_accent)).padding([sp.s, sp.l]))
        .on_press(message)
        .style(crate::styles::accent_button(tokens))
        .into()
}

fn secondary_btn<'a>(
    label: &'a str,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let text_c = crate::styles::ti(tokens.text);
    button(container(text(label.to_string()).size(ts.body).color(text_c)).padding([sp.s, sp.l]))
        .on_press(message)
        .style(crate::styles::outline_button(tokens))
        .into()
}
