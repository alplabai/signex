//! Icon buttons for toolbar strips.
//!
//! Themed icon buttons with tooltip, active/inactive state, separators,
//! and grouping — all built on stock Iced 0.14 primitives.

use iced::widget::{Row, button, container, text, tooltip};
use iced::{Border, Element, Length};
use signex_types::theme::ThemeTokens;

use crate::theme_ext;

// ---------------------------------------------------------------------------
// Icon button
// ---------------------------------------------------------------------------

/// Create a themed toolbar icon button.
///
/// * `icon`         — Unicode char or short text displayed on the button face.
/// * `tooltip_text` — Text shown on hover in a tooltip.
/// * `on_press`     — Message emitted when the button is pressed.
/// * `active`       — Whether this button is in the "active" / pressed state.
/// * `tokens`       — Theme tokens for colors.
pub fn icon_button<'a, M: Clone + 'a>(
    icon: &str,
    tooltip_text: &str,
    on_press: M,
    active: bool,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let text_color = if active {
        theme_ext::text_primary(tokens)
    } else {
        theme_ext::text_secondary(tokens)
    };

    let label = text(icon.to_owned()).size(13).color(text_color);
    let btn = button(label).padding([3, 7]).on_press(on_press);

    let styled_btn: Element<'a, M> = if active {
        let accent = theme_ext::accent(tokens);
        // Active button: accent-colored background
        container(btn.style(button::text))
            .style(move |_theme: &iced::Theme| container::Style {
                background: Some(accent.into()),
                border: Border {
                    width: 0.0,
                    radius: 2.0.into(),
                    color: iced::Color::TRANSPARENT,
                },
                ..container::Style::default()
            })
            .into()
    } else {
        btn.style(button::text).into()
    };

    let tip_text = tooltip_text.to_owned();
    tooltip(
        styled_btn,
        text(tip_text).size(11),
        tooltip::Position::Bottom,
    )
    .gap(4)
    .into()
}

// ---------------------------------------------------------------------------
// Separator
// ---------------------------------------------------------------------------

/// A thin vertical separator for toolbar button groups.
pub fn toolbar_separator<'a, M: 'a>(tokens: &ThemeTokens) -> Element<'a, M> {
    let border = theme_ext::border_color(tokens);
    container(
        container(text("").size(1))
            .width(1)
            .height(Length::Fixed(18.0))
            .style(move |_theme: &iced::Theme| container::Style {
                background: Some(border.into()),
                ..container::Style::default()
            }),
    )
    .padding([0, 3])
    .into()
}

// ---------------------------------------------------------------------------
// Button group
// ---------------------------------------------------------------------------

/// Group multiple button elements into a horizontal row with no spacing.
pub fn button_group<'a, M: 'a>(buttons: Vec<Element<'a, M>>) -> Element<'a, M> {
    let mut r: Row<'a, M> = Row::new().spacing(0).align_y(iced::Alignment::Center);
    for btn in buttons {
        r = r.push(btn);
    }
    r.into()
}
