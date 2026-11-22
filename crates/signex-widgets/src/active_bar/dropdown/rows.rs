//! The rows of a dropdown, without the card around them.

use iced::widget::{Column, Space, container};
use iced::{Background, Color, Element, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;

use super::{DropdownEntry, Look, MenuMetrics, RowShape, item_row, metrics};
use crate::{raw, text};

/// `entries` as a column as wide as its parent, for a caller that draws the
/// card itself (one column of a two-column card).
pub fn rows<'a, M>(entries: Vec<DropdownEntry<M>>, tokens: &'a ThemeTokens) -> Element<'a, M>
where
    M: 'static + Clone,
{
    let line = super::super::skin::Skin::of(tokens).line_soft;
    entry_column(entries, Look::of(tokens), line, true).boxed()
}

/// One line per entry. `fixed`: the parent has a fixed width, so the column
/// fills it; otherwise it fits its content.
pub(super) fn entry_column<'a, M>(
    entries: Vec<DropdownEntry<M>>,
    look: Look,
    line: Color,
    fixed: bool,
) -> Column<Element<'a, M>>
where
    M: 'static + Clone,
{
    let metrics = MenuMetrics::GRAPHITE;
    // Fill children inside a Shrink column lay out at zero width in the
    // fork, so a fixed-width panel gets a Fill column (buttons stay clickable)
    // and an unsized panel fits its content.
    let width = if fixed { Length::Fill } else { Length::Fit };
    let shape = RowShape {
        has_icons: metrics::has_icons(&entries),
        has_chevrons: metrics::has_chevrons(&entries),
        fixed,
    };
    let lines = entries.into_iter().map(|entry| match entry {
        DropdownEntry::Header(label) => header(label, &metrics, look),
        DropdownEntry::Separator => separator(&metrics, line),
        DropdownEntry::Custom(element) => element,
        DropdownEntry::Item(item) => item_row(item, &metrics, look, shape),
    });
    Column::with_children(lines).spacing(0).width(width)
}

fn header<'a, M: 'a>(label: String, metrics: &MenuMetrics, look: Look) -> Element<'a, M> {
    container(
        text(raw(label))
            .size(metrics.header_size)
            .color(look.muted)
            .wrapping(text::Wrapping::None),
    )
    .padding([0.0, metrics.row_pad_x])
    .height(metrics.header_row_height())
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .boxed()
}

fn separator<'a, M: 'a>(metrics: &MenuMetrics, line: Color) -> Element<'a, M> {
    container(
        container(Space::new())
            .height(1.0)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(line)),
                ..container::Style::default()
            }),
    )
    .padding([metrics.separator_margin, 0.0])
    .width(Length::Fill)
    .boxed()
}
