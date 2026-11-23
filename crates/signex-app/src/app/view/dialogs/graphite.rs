//! Graphite parts shared by the dialog bodies: the mono face, the subtitled
//! header, the call-to-action kind, the changed-designator ink and the
//! callout.

use iced::widget::{Row, container};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, text};

use crate::styles::{
    self, DialogButtonKind, DialogIcon, Radii, Spacing, TypeScale, chrome_px, tokens,
};

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

/// Ink of a proposed designator that changes.
pub(super) fn changed_ink(theme_tokens: &ThemeTokens) -> iced::Color {
    tokens::resolve(theme_tokens).accent_text
}

/// Horizontal padding of a [`callout`] (10 px at the 12 px base).
const CALLOUT_PAD_X: f32 = chrome_px(10.0 * 11.0 / 12.0);

/// A note in a dialog body: `icon`'s glyph in its tone's ink, then
/// `message` in the body ink, on the tone's soft fill. `DialogIcon::Info`
/// explains, `DialogIcon::Warn` warns.
pub(super) fn callout<'a, M: 'a>(
    icon: DialogIcon,
    message: impl Into<Label<'a>>,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    let (fill, ink) = styles::pill_colors(theme_tokens, icon.tone());
    // One body line tall, so the glyph sits on the first line.
    let glyph = TS.body * styles::LINE_HEIGHT;
    let mut line: Row<Element<'a, M>> = Row::new().spacing(SP.m).align_y(Alignment::Start);
    line = line.push(styles::tinted_glyph(icon.glyph_handle(), glyph, ink));
    line = line.push(
        text(message)
            .size(TS.body)
            .color(styles::ti(theme_tokens.text))
            .width(Length::Fill)
            .boxed(),
    );
    container(line)
        .padding([SP.m, CALLOUT_PAD_X])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
