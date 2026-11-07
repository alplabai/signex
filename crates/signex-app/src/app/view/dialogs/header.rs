//! Graphite dialog shell parts: the themed scrim, the corner radius, the
//! 44 px / 52 px header with a subtitle slot, and the neutral close button.
//! Legacy themes keep the old strip and the red-hover X (`widgets.rs`).

use iced::widget::{Space, button, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;

use crate::styles::{
    self, MODAL_CORNER_RADIUS, Radii, Spacing, TypeScale, chrome_px, is_graphite, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Header height of a Graphite dialog (44 px at the default base).
pub(crate) const DIALOG_HEADER_H: f32 = chrome_px(44.0 * 11.0 / 12.0);
/// Header height of the Preferences dialog (52 px at the default base).
pub(crate) const PREFERENCES_HEADER_H: f32 = chrome_px(52.0 * 11.0 / 12.0);

/// The one modal scrim: the theme's scrim token on Graphite, black at
/// [`tokens::BACKDROP_ALPHA`] on every other theme.
pub(crate) fn dialog_scrim(theme_tokens: &ThemeTokens) -> Color {
    if is_graphite(theme_tokens) {
        tokens::resolve(theme_tokens).scrim
    } else {
        tokens::scrim(tokens::BACKDROP_ALPHA)
    }
}

/// Container style of a full-window scrim.
pub(crate) fn scrim_style(
    theme_tokens: &ThemeTokens,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let colour = dialog_scrim(theme_tokens);
    move |_| container::Style {
        background: Some(Background::Color(colour)),
        ..container::Style::default()
    }
}

/// Outer corner radius of a dialog card.
pub(crate) fn dialog_corner(theme_tokens: &ThemeTokens) -> f32 {
    if is_graphite(theme_tokens) {
        Radii::GRAPHITE.dialog
    } else {
        MODAL_CORNER_RADIUS
    }
}

/// Neutral close button: a quiet 28 px square with a cross, ghost hover.
pub(crate) fn neutral_close_button<'a, M: Clone + 'a>(
    theme_tokens: &ThemeTokens,
    message: M,
) -> Element<'a, M> {
    let side = chrome_px(28.0 * 11.0 / 12.0);
    button(
        text("\u{00D7}")
            .size(TS.heading)
            .color(tokens::muted_text(theme_tokens))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .padding(0)
    .width(Length::Fixed(side))
    .height(Length::Fixed(side))
    .on_press(message)
    .style(styles::ghost_button(theme_tokens))
    .boxed()
}

/// Graphite dialog header: title, optional subtitle, free space, an
/// optional trailing control, then the neutral close. `height` is
/// [`DIALOG_HEADER_H`] or [`PREFERENCES_HEADER_H`].
pub(crate) fn graphite_header<'a, M: Clone + 'a>(
    title: impl Into<String>,
    subtitle: Option<String>,
    trailing: Option<Element<'a, M>>,
    on_close: M,
    height: f32,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let mut line = row![
        text(title.into())
            .size(TS.heading)
            .color(ti(theme_tokens.text))
            .boxed()
    ]
    .spacing(SP.l)
    .align_y(Alignment::Center);
    if let Some(sub) = subtitle {
        line = line.push(text(sub).size(TS.label).color(r.ink3).boxed());
    }
    line = line.push(Space::new().width(Length::Fill).boxed());
    if let Some(extra) = trailing {
        line = line.push(extra);
    }
    line = line.push(neutral_close_button(theme_tokens, on_close));
    let (paper, rule, corner) = (
        ti(theme_tokens.paper),
        r.line_soft,
        dialog_corner(theme_tokens),
    );
    container(line)
        .padding([0.0, SP.l])
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .align_y(iced::alignment::Vertical::Center)
        .style(move |theme: &Theme| {
            let flat = styles::is_dialog_window(theme);
            let mut border = Border {
                color: rule,
                radius: if flat {
                    iced::border::Radius::default()
                } else {
                    iced::border::Radius::default()
                        .top_left(corner)
                        .top_right(corner)
                },
                ..Border::default()
            };
            border.bottom = iced::border::Side::default().width(1.0);
            container::Style {
                background: Some(Background::Color(paper)),
                border,
                ..container::Style::default()
            }
        })
        .boxed()
}
