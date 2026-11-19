//! Graphite parts shared by the dialog bodies: the subtitled header, the
//! changed-row tint and the recovery icon tile.

use iced::widget::container;
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text};

use crate::styles::{self, DialogButtonKind, PillTone, Radii, Spacing, TypeScale, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The chrome mono face for designators and numbers.
pub(super) fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// Dialog header: title plus optional subtitle.
pub(super) fn titled_header<'a, M: Clone + 'a>(
    title: impl Into<String>,
    subtitle: Option<String>,
    on_close: M,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    super::header::graphite_header(
        title,
        subtitle,
        None,
        on_close,
        super::header::DIALOG_HEADER_H,
        theme_tokens,
    )
}

/// The button kind of a dialog's call to action: copper.
pub(super) fn call_to_action(_theme_tokens: &ThemeTokens) -> DialogButtonKind {
    DialogButtonKind::Accent
}

/// Row tint of an Annotate row whose designator changes: the soft accent
/// fill. Unchanged rows stay unfilled.
pub(super) fn changed_row(
    theme_tokens: &ThemeTokens,
    changing: bool,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let fill = changing.then(|| tokens::resolve(theme_tokens).accent_soft);
    move |_| container::Style {
        background: fill.map(Background::Color),
        ..container::Style::default()
    }
}

/// Ink of a proposed designator that changes.
pub(super) fn changed_ink(theme_tokens: &ThemeTokens) -> iced::Color {
    tokens::resolve(theme_tokens).accent_text
}

/// Soft-warning icon tile of the recovery prompt.
pub(super) fn recovery_icon_tile<'a, M: 'a>(theme_tokens: &ThemeTokens) -> Element<'a, M> {
    let (fill, ink) = styles::pill_colors(theme_tokens, PillTone::Warn);
    let side = TS.glyph + 2.0 * SP.m;
    container(
        // raw: a restore glyph, not language.
        text(raw("\u{21BA}"))
            .size(TS.glyph)
            .color(ink)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fixed(side))
    .height(Length::Fixed(side))
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}
