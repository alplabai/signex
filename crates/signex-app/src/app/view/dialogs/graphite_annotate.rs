//! Graphite controls of the Annotate dialog: the checkbox pip, the order
//! and lock toggles, the field box and the order-preview cell. Callers use
//! them only when `is_graphite(tokens)`.

use iced::widget::{button, container, text};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;

use crate::styles::{Radii, TypeScale, tokens};

const TS: TypeScale = TypeScale::DEFAULT;

/// Read-only checkbox pip: accent fill with a check when on, an empty
/// field box with a strong hairline when off.
pub(super) fn checkbox_pip<'a, M: 'a>(theme_tokens: &ThemeTokens, on: bool) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let accent = tokens::accent_fill(theme_tokens);
    let side = TS.body + 2.0;
    let mark = if on { "\u{2713}" } else { " " };
    let (fill, edge) = if on {
        (accent, accent)
    } else {
        (r.field, r.line_strong)
    };
    container(
        text(mark)
            .size(TS.caption)
            .color(tokens::on_accent(theme_tokens)),
    )
    .width(Length::Fixed(side))
    .height(Length::Fixed(side))
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: 1.0,
            color: edge,
            radius: 4.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// Toggle-pill style (order choice, lock): selected takes the soft accent
/// fill, its ink and an accent hairline; unselected is a field box.
pub(super) fn toggle_style(
    theme_tokens: &ThemeTokens,
    selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let r = tokens::resolve(theme_tokens);
    let accent = tokens::accent(theme_tokens);
    let ink = crate::styles::ti(theme_tokens.text);
    let hover = tokens::hover_overlay(theme_tokens);
    move |_: &Theme, status: button::Status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let (fill, edge, fg) = if selected {
            (r.accent_soft, accent, r.accent_soft_ink)
        } else if hovered {
            (hover, r.line_strong, ink)
        } else {
            (r.field, r.line_soft, ink)
        };
        button::Style {
            background: Some(Background::Color(fill)),
            text_color: fg,
            border: Border {
                width: 1.0,
                color: edge,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    }
}

/// Field box around a read-only value or a list: field fill, hairline.
pub(super) fn field_box(
    theme_tokens: &ThemeTokens,
) -> impl Fn(&Theme) -> container::Style + 'static {
    crate::styles::field(theme_tokens, false)
}

/// One cell of the order-preview legend: sunken fill, hairline, mono ink.
pub(super) fn preview_cell(
    theme_tokens: &ThemeTokens,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let r = tokens::resolve(theme_tokens);
    let (fill, edge) = (r.sunken, r.line_soft);
    move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: 1.0,
            color: edge,
            radius: 4.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Mono face of designators (the preview cells).
pub(super) fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}
