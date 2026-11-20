//! Target band samples on both Graphite themes: a full bar in band and one
//! out of band over their axis caption, then the small bars of the dock
//! panel. Static samples.

use iced::widget::{Column, column, container, row};
use iced::{Background, Element, Length, Theme, Widget};
use signex_types::coord::Unit;
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::LengthFormat;
use signex_widgets::target_band::{BandSize, TargetBandStyle, axis_row, target_band};
use signex_widgets::theme_ext::{Spacing, TypeScale};

use crate::catalog::Message;
use crate::theme;

const BODY: f32 = TypeScale::DEFAULT.body;
const SP: Spacing = Spacing::DEFAULT;
/// Width of one theme's sample column, in em: the tab's bar column.
const COLUMN_EM: f32 = 26.0;
/// Target and tolerance of every sample, in mm.
const TARGET: f64 = 28.5;
const TOLERANCE: f64 = 0.25;

fn format() -> LengthFormat<'static> {
    LengthFormat::new(crate::locale::en_us(), Unit::Mm)
}

fn samples<'a>(theme_id: ThemeId) -> Element<'a, Message> {
    let tokens = theme_tokens(theme_id);
    let style = TargetBandStyle::from_tokens(&tokens, BODY);
    let full = |value: f64| target_band(value, TARGET, TOLERANCE, BandSize::Full, style);
    let small = |value: f64| target_band(value, TARGET, TOLERANCE, BandSize::Small, style);
    let body: Column<Element<'a, Message>> = column![
        axis_row(&format(), TARGET, TOLERANCE, style),
        full(28.40),
        full(28.91),
        full(26.00),
        small(28.52),
        small(27.70),
    ]
    .spacing(SP.s)
    .width(BODY * COLUMN_EM);
    let ground = theme::color(tokens.panel_bg);
    container(body)
        .padding(SP.l)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(ground)),
            ..container::Style::default()
        })
        .boxed()
}

pub(crate) fn view<'a>() -> Element<'a, Message> {
    row![
        samples(ThemeId::GraphiteLight),
        samples(ThemeId::GraphiteDark)
    ]
    .spacing(SP.xl)
    .width(Length::Shrink)
    .boxed()
}
