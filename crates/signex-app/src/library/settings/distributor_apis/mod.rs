//! Preferences ▸ Library ▸ Distributor APIs.
//!
//! Two columns: one card per distributor account (`cards`) and the order of
//! preference with drag handles (`order`). DigiKey connects through OAuth2
//! PKCE (`digikey_oauth`), Mouser takes an API key and a Test, LCSC and
//! JLCPCB need nothing. Tokens and keys live in the OS keyring; the order is
//! saved to `distributors.toml` (`persistence`).

mod cards;
mod order;

pub(crate) use order::grip_handle;

use iced::widget::{column, container, mouse_area, row};
use iced::{Background, Border, Element, Font, Length, Theme, Widget as _};
use signex_i18n::I18n;
use signex_library::DistributorSource;
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, text, text_input};

use crate::styles::{PillTone, Radii, Spacing, TypeScale, ti, tokens::resolve};

use super::super::messages::{LibraryMessage, SettingsMsg};
use super::super::state::DistributorSettings;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the order-of-preference column beside the cards.
const ORDER_COLUMN_W: f32 = 280.0;

/// Render the Distributor APIs page body: the cards beside the order list.
pub fn view<'a>(
    settings: &'a DistributorSettings,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let cards = column![
        cards::digikey(settings, tokens, i18n),
        cards::mouser(settings, tokens, i18n),
        cards::no_key(tokens, i18n),
    ]
    .spacing(SP.m + SP.xs)
    .width(Length::Fill);
    let side = container(order::order_column(settings, tokens, i18n)).width(ORDER_COLUMN_W);
    let body = row![cards, side]
        .spacing(SP.xl)
        .align_y(iced::Alignment::Start)
        .width(Length::Fill);
    // A release anywhere on the page ends a drag of the order list; one
    // over a row has already dropped it there. Always a mouse area, so the
    // tree keeps its shape (and the key field its focus) when a drag starts.
    let mut area = mouse_area(body);
    if settings.preferred_order_drag.is_some() {
        area = area.on_release(settings_msg(SettingsMsg::PreferenceDragEnded));
    }
    area.boxed()
}

fn settings_msg(msg: SettingsMsg) -> LibraryMessage {
    LibraryMessage::Settings(msg)
}

/// The distributor's product name; `None` for "Other", which is a word.
fn distributor_label(src: DistributorSource) -> Option<&'static str> {
    match src {
        DistributorSource::DigiKey => Some("DigiKey"),
        DistributorSource::Mouser => Some("Mouser"),
        DistributorSource::Lcsc => Some("LCSC"),
        DistributorSource::Jlcpcb => Some("JLCPCB"),
        DistributorSource::Octopart => Some("Octopart"),
        DistributorSource::Oemsecrets => Some("OEMsecrets"),
        DistributorSource::Other => None,
    }
}

/// The semibold face of a card's name.
fn semibold() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..Font::DEFAULT
    }
}

/// The chrome mono face (API key, rank, config path).
fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// A rounded status chip: soft fill, toned ink.
fn status_pill<'a>(
    label: impl Into<Label<'a>>,
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    container(
        text(label)
            .size(TS.caption)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([SP.xs / 2.0, SP.s + SP.xs])
    .style(crate::styles::pill(tokens, tone))
    .boxed()
}

/// A muted caption line.
fn caption<'a>(label: impl Into<Label<'a>>, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    text(label)
        .size(TS.caption)
        .color(resolve(tokens).ink3)
        .boxed()
}

/// A caption line in the error colour.
fn error_caption<'a>(
    label: impl Into<Label<'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    text(label).size(TS.caption).color(ti(tokens.error)).boxed()
}

/// A distributor card: the panel fill, a hairline border and the card radius.
fn card<'a>(
    content: impl Into<Element<'a, LibraryMessage>>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let (fill, line) = (ti(tokens.panel_bg), ti(tokens.border));
    container(content)
        .padding([SP.l, SP.l + SP.xs])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A Graphite text field: field fill, hairline, accent border when focused.
fn field_style(tokens: &ThemeTokens) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    let r = resolve(tokens);
    let (fill, line, focus, ink, hint) = (
        r.field,
        ti(tokens.border),
        ti(tokens.accent),
        ti(tokens.text),
        r.ink4,
    );
    move |_: &Theme, status: text_input::Status| text_input::Style {
        background: Background::Color(fill),
        border: Border {
            width: 1.0,
            color: if matches!(status, text_input::Status::Focused { .. }) {
                focus
            } else {
                line
            },
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        placeholder: hint,
        value: ink,
        selection: iced::Color { a: 0.3, ..focus },
    }
}
