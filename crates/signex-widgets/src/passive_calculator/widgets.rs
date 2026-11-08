//! Row builders the passive-calculator tabs share: a captioned control, a
//! labelled read-out, and the theme colour conversion. Each tab passes the
//! text sizes it draws at.

use crate::{raw, text};
use iced::widget::column;
use iced::{Element, Widget};
use signex_types::theme::ThemeTokens;

/// Text sizes of a read-out: the muted `label` over the `value`.
#[derive(Debug, Clone, Copy)]
pub(super) struct MetricSizes {
    pub label: f32,
    pub value: f32,
}

/// The read-out of the calculator and component-card tabs.
pub(super) const COMPACT_METRIC: MetricSizes = MetricSizes {
    label: 10.0,
    value: 11.0,
};

/// A control under a caption of `caption_size`.
pub(super) fn labeled<'a, M: 'a>(
    caption_size: f32,
    label: &'a str,
    control: impl Widget<M> + 'a,
) -> Element<'a, M> {
    column![text(raw(label)).size(caption_size), control]
        .spacing(4)
        .boxed()
}

/// A muted `label` over its `value`, at `sizes`.
pub(super) fn metric<'a, M: 'a>(
    sizes: MetricSizes,
    label: &'a str,
    value: String,
    tokens: &'a ThemeTokens,
) -> Element<'a, M> {
    column![
        text(raw(label))
            .size(sizes.label)
            .color(token_color(tokens.text_secondary)),
        text(raw(value))
            .size(sizes.value)
            .color(token_color(tokens.text)),
    ]
    .spacing(2)
    .boxed()
}

pub(super) fn token_color(color: signex_types::theme::Color) -> iced::Color {
    iced::Color::from_rgba8(color.r, color.g, color.b, f32::from(color.a) / 255.0)
}
