//! Settings → Library → Distributor APIs panel.
//!
//! Spec (v0.9-library-plan.md §14a.2):
//!
//! - DigiKey: OAuth2 PKCE — Phase 1 stubs the connect button.
//! - Mouser: API key in keyring — Phase 1 takes the user's key in
//!   memory + a Test button.
//! - LCSC, JLCPCB: no key required.
//! - Order-of-preference list — Phase 1 holds it in memory only.

use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_library::DistributorSource;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{LibraryMessage, SettingsMsg};
use super::super::state::DistributorSettings;

/// Render the Distributor APIs panel. Mounts inside the existing
/// Preferences modal under a Library section.
///
/// Phase 1 ships this panel; Phase 2 wires it into the
/// `crate::preferences` modal as a dedicated pref pane.
#[allow(dead_code)]
pub fn view<'a>(
    settings: &'a DistributorSettings,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let hover = crate::styles::ti(tokens.hover);

    let section_title = |title: &'static str| -> Element<'_, LibraryMessage> {
        text(title).size(11).color(muted).into()
    };

    // ── DigiKey ──────────────────────────────────────────────
    let digikey_status = match (
        settings.digikey_in_flight,
        settings.digikey_status.as_deref(),
        settings.digikey_account_email.as_deref(),
    ) {
        (true, Some(s), _) => s.to_string(),
        (false, _, Some(label)) => format!("Connected as {label}"),
        (false, Some(s), _) => s.to_string(),
        _ => "Not connected".to_string(),
    };
    let mut digikey_actions =
        row![text(digikey_status).size(11).color(text_c)].align_y(iced::Alignment::Center);
    digikey_actions = digikey_actions.push(Space::new().width(Length::Fill));
    if settings.digikey_in_flight {
        digikey_actions = digikey_actions.push(secondary_btn(
            "Cancel",
            LibraryMessage::Settings(SettingsMsg::DigiKeyCancel),
            text_c,
            border,
        ));
    } else {
        digikey_actions = digikey_actions.push(primary_btn(
            "Connect via OAuth",
            LibraryMessage::Settings(SettingsMsg::DigiKeyConnect),
        ));
    }
    let digikey_section = column![
        section_title("DigiKey"),
        Space::new().height(4),
        digikey_actions,
        Space::new().height(4),
        text(
            "OAuth2 PKCE flow — opens your browser, then stores the refresh token in the OS \
             keyring. Set SIGNEX_DIGIKEY_CLIENT_ID + SIGNEX_DIGIKEY_CLIENT_SECRET first."
        )
        .size(10)
        .color(muted),
    ]
    .spacing(2);

    // ── Mouser ──────────────────────────────────────────────
    let masked: String = if settings.mouser_api_key_buf.is_empty() {
        String::new()
    } else {
        "\u{2022}".repeat(settings.mouser_api_key_buf.len().min(32))
    };
    let mouser_test_status = settings
        .mouser_status
        .clone()
        .unwrap_or_else(|| "Not tested".to_string());
    let mouser_section = column![
        section_title("Mouser"),
        Space::new().height(4),
        row![
            text("API Key")
                .size(11)
                .color(text_c)
                .width(Length::Fixed(96.0)),
            text_input("Paste API key…", &masked)
                .on_input(|s| LibraryMessage::Settings(SettingsMsg::MouserApiKeyChanged(s)))
                .padding([4, 8])
                .size(12),
            Space::new().width(8),
            primary_btn("Test", LibraryMessage::Settings(SettingsMsg::MouserTest)),
        ]
        .align_y(iced::Alignment::Center),
        Space::new().height(4),
        text(mouser_test_status).size(10).color(muted),
    ]
    .spacing(2);

    // ── LCSC + JLCPCB ──────────────────────────────────────
    let no_key_section = column![
        section_title("LCSC, JLCPCB"),
        Space::new().height(4),
        text("(no key required)").size(11).color(text_c),
    ]
    .spacing(2);

    // ── Preference order ───────────────────────────────────
    let mut order_col =
        column![section_title("Order of Preference"), Space::new().height(4)].spacing(2);
    let len = settings.preferred_order.len();
    for (i, src) in settings.preferred_order.iter().enumerate() {
        let label = distributor_label(*src);
        let up_enabled = i > 0;
        let down_enabled = i + 1 < len;
        let mut up_btn = button(container(text("▲").size(10).color(text_c)).padding([1, 6]))
            .padding(0)
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
                text_color: text_c,
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            });
        if up_enabled {
            up_btn = up_btn.on_press(LibraryMessage::Settings(SettingsMsg::PreferenceUp(*src)));
        }
        let mut down_btn = button(container(text("▼").size(10).color(text_c)).padding([1, 6]))
            .padding(0)
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
                text_color: text_c,
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            });
        if down_enabled {
            down_btn =
                down_btn.on_press(LibraryMessage::Settings(SettingsMsg::PreferenceDown(*src)));
        }
        let row_widget = row![
            text(format!("{}.  {}", i + 1, label))
                .size(11)
                .color(text_c)
                .width(Length::Fill),
            up_btn,
            Space::new().width(4),
            down_btn,
        ]
        .padding([3, 6])
        .align_y(iced::Alignment::Center);
        let bg_row = container(row_widget).style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                hover.r, hover.g, hover.b, 0.06,
            ))),
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
        .size(10)
        .color(muted),
    );

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
        .spacing(12)
        .width(Length::Fill),
    )
    .padding(14)
    .style(crate::styles::modal_card(tokens))
    .into()
}

#[allow(dead_code)]
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

#[allow(dead_code)]
fn divider(color: iced::Color) -> Element<'static, LibraryMessage> {
    container(Space::new().height(1).width(Length::Fill))
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(color)),
            ..Default::default()
        })
        .into()
}

#[allow(dead_code)]
fn primary_btn<'a>(label: &'a str, message: LibraryMessage) -> Element<'a, LibraryMessage> {
    button(container(text(label.to_string()).size(11).color(iced::Color::WHITE)).padding([4, 12]))
        .on_press(message)
        .style(|_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(
                0.00, 0.47, 0.84,
            ))),
            text_color: iced::Color::WHITE,
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        })
        .into()
}

#[allow(dead_code)]
fn secondary_btn<'a>(
    label: &'a str,
    message: LibraryMessage,
    text_c: iced::Color,
    border: iced::Color,
) -> Element<'a, LibraryMessage> {
    button(container(text(label.to_string()).size(11).color(text_c)).padding([4, 12]))
        .on_press(message)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: text_c,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        })
        .into()
}
