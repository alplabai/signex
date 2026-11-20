//! The shell's toolbar row: the caller's items from the left, wrapping onto
//! more lines when the tab is narrow, and an end group (totals, pills)
//! pushed to the right edge.

use iced::widget::{Column, Row, Space, container};
use iced::{Alignment, Element, Length, Padding, Theme, Widget};

use super::{DocTabShellStyle, filled, rule};
use crate::theme_ext::HAIRLINE;

/// One entry of the toolbar's start group.
pub enum ToolbarItem<'a, M> {
    /// A caller widget: a button, a field, a toggle, a label.
    Item(Element<'a, M>),
    /// A short vertical line between two groups of items.
    Separator,
}

impl<M> ToolbarItem<'_, M> {
    pub fn is_separator(&self) -> bool {
        matches!(self, Self::Separator)
    }
}

/// The toolbar. Items are added left to right with [`push`](Toolbar::push)
/// and [`separator`](Toolbar::separator); [`push_end`](Toolbar::push_end)
/// adds to the group at the right edge.
pub struct Toolbar<'a, M> {
    start: Vec<ToolbarItem<'a, M>>,
    end: Vec<Element<'a, M>>,
}

impl<M> Default for Toolbar<'_, M> {
    fn default() -> Self {
        Self {
            start: Vec::new(),
            end: Vec::new(),
        }
    }
}

impl<'a, M: 'a> Toolbar<'a, M> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an item to the start group.
    pub fn push(mut self, item: Element<'a, M>) -> Self {
        self.start.push(ToolbarItem::Item(item));
        self
    }

    /// Add a separator to the start group. A separator first, last or next
    /// to another one separates nothing and is not drawn.
    pub fn separator(mut self) -> Self {
        self.start.push(ToolbarItem::Separator);
        self
    }

    /// Add an item to the end group, after the flexible space.
    pub fn push_end(mut self, item: Element<'a, M>) -> Self {
        self.end.push(item);
        self
    }

    /// Which start-group entries are drawn: every item, and a separator
    /// only between two items.
    fn drawn(&self) -> Vec<bool> {
        let start = &self.start;
        let mut mask = vec![false; start.len()];
        let mut last_kept: Option<usize> = None;
        for (i, item) in start.iter().enumerate() {
            let after_item = last_kept.is_some_and(|j| !start[j].is_separator());
            if !item.is_separator() || after_item {
                mask[i] = true;
                last_kept = Some(i);
            }
        }
        if let Some(j) = last_kept.filter(|&j| start[j].is_separator()) {
            mask[j] = false;
        }
        mask
    }

    /// Items drawn in the start group, separators included.
    pub fn start_len(&self) -> usize {
        self.drawn().into_iter().filter(|&d| d).count()
    }

    /// Separators drawn in the start group.
    pub fn separator_count(&self) -> usize {
        self.start
            .iter()
            .zip(self.drawn())
            .filter(|(item, drawn)| *drawn && item.is_separator())
            .count()
    }

    /// Items in the end group.
    pub fn end_len(&self) -> usize {
        self.end.len()
    }

    pub(super) fn view(self, s: &DocTabShellStyle) -> Element<'a, M> {
        let m = s.metrics;
        let keep = self.drawn();
        let mut start: Row<Element<'a, M>> = Row::new()
            .spacing(m.toolbar_gap)
            .align_y(Alignment::Center)
            .width(Length::Fill);
        for (item, keep) in self.start.into_iter().zip(keep) {
            if !keep {
                continue;
            }
            start = start.push(match item {
                ToolbarItem::Item(element) => element,
                ToolbarItem::Separator => separator(s),
            });
        }
        let mut line: Row<Element<'a, M>> = Row::new()
            .spacing(m.toolbar_gap)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .push(start.wrap().vertical_spacing(m.toolbar_gap).boxed());
        if !self.end.is_empty() {
            let end: Row<Element<'a, M>> = Row::with_children(self.end)
                .spacing(m.toolbar_gap)
                .align_y(Alignment::Center);
            line = line.push(end.boxed());
        }
        let chrome = s.chrome;
        let band = container(line)
            .width(Length::Fill)
            .padding(Padding::from([m.toolbar_pad_y, m.pad_x]))
            .style(move |_: &Theme| filled(chrome));
        Column::new()
            .width(Length::Fill)
            .push(band.boxed())
            .push(rule(s.line_soft))
            .boxed()
    }
}

/// The toolbar separator: a short vertical line with clear space each side.
/// [`Toolbar::separator`] draws it; it is public for an end group that needs
/// one.
pub fn separator<'a, M: 'a>(s: &DocTabShellStyle) -> Element<'a, M> {
    let m = s.metrics;
    let line = s.border;
    container(
        container(Space::new().width(HAIRLINE).height(m.separator_height))
            .style(move |_: &Theme| filled(line)),
    )
    .padding(Padding::from([0.0, m.separator_margin]))
    .boxed()
}
