//! The measurement slot (mockup row 139): a mono number field and its prefix / unit
//! picker drawn as one control. The two share one border; the picker is
//! the right segment behind a hairline divider, and only the outer corners
//! are rounded. Without a picker the field keeps all four corners.

use iced::widget::{Space, container, row, text_input as text_input_mod};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::Localized;
use signex_types::theme::ThemeTokens;
use signex_widgets::{text_input, theme_ext};

use crate::panels::form_look::text_input_style;
use crate::styles::{HAIRLINE, Radii};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Corner radius of the joined control: a form field's.
const RADIUS: f32 = Radii::GRAPHITE.control;

/// Width shares of the number and the picker: a table laying the slot
/// across its Value and Unit columns uses them as the column shares.
pub(crate) const NUMBER_SHARE: u16 = 3;
pub(crate) const PICKER_SHARE: u16 = 2;

/// The slot: `value` in a mono input, with `picker` (a measurement menu or
/// a read-only label) as its right segment. `on_submit` is sent on Enter.
pub(crate) fn measurement_slot<'a, M: Clone + 'a>(
    value: &str,
    placeholder: Localized,
    on_input: impl Fn(String) -> M + 'a,
    on_submit: Option<M>,
    picker: Option<Element<'a, M>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let input = text_input(placeholder, value.to_string())
        .on_input(on_input)
        .on_submit_maybe(on_submit)
        .font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ))
        .padding([SP.xs + SP.xs, SP.s + SP.xs])
        .size(TS.body)
        .width(Length::Fill);
    let Some(picker) = picker else {
        return input
            .style(|theme: &Theme, status| {
                let mut style = text_input_style(theme, status);
                style.border.radius = RADIUS.into();
                style
            })
            .boxed();
    };
    // The outer frame draws the shared border; the input shows its own only
    // as the focus ring, on its rounded side.
    let input = input.style(|theme: &Theme, status| {
        let mut style = text_input_style(theme, status);
        style.border.radius = iced::border::left(RADIUS);
        if !matches!(status, text_input_mod::Status::Focused { .. }) {
            style.border.width = 0.0;
        }
        style
    });
    let line = theme_ext::border_color(tokens);
    let field = crate::styles::tokens::resolve(tokens).field;
    let input = input.width(Length::FillPortion(NUMBER_SHARE));
    let picker = container(picker).width(Length::FillPortion(PICKER_SHARE));
    let joined = row![input, segment_divider(tokens), picker]
        .height(Length::Shrink)
        .align_y(iced::Alignment::Center);
    container(joined)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(field)),
            border: Border {
                width: 1.0,
                color: line,
                radius: RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The hairline between two segments of a slot.
fn segment_divider<'a, M: 'a>(tokens: &ThemeTokens) -> Element<'a, M> {
    let line = theme_ext::border_color(tokens);
    container(Space::new())
        .width(HAIRLINE)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}
