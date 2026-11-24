//! The shared dialog search / filter field: the search glyph, then the
//! input, on the field frame. Used by the component and primitive pickers
//! and the Net Colors palette (mockup row 71).

use iced::widget::{container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::Localized;
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::text_input;

use super::tokens::resolve;
use super::{ChromeMetrics, Spacing, TypeScale, field, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// A dialog's search field. `on_submit` is the Enter action, if any.
pub fn dialog_search_field<'a, M: Clone + 'a>(
    placeholder: Localized,
    value: &'a str,
    on_input: impl Fn(String) -> M + 'a,
    on_submit: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let r = resolve(tokens);
    let (ink, placeholder_ink, glyph_ink) = (ti(tokens.text), r.ink4, r.ink4);
    let selection = Color {
        a: 0.4,
        ..ti(tokens.accent)
    };
    let input = text_input(placeholder, value)
        .on_input(on_input)
        .on_submit_maybe(on_submit)
        .padding([SP.xs, SP.s])
        .size(TS.body)
        .width(Length::Fill)
        .style(move |_: &Theme, _| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: placeholder_ink,
            value: ink,
            selection,
        });
    let glyph = iced::widget::svg(crate::icons::icon_chrome_search(ThemeId::GraphiteDark))
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme, _| iced::widget::svg::Style {
            color: Some(glyph_ink),
            ..Default::default()
        });
    container(row![glyph, input].spacing(SP.s).align_y(Alignment::Center))
        .padding([0.0, SP.m])
        .width(Length::Fill)
        .center_y(Length::Fixed(ChromeMetrics::DEFAULT.field))
        .style(field(tokens, false))
        .boxed()
}
