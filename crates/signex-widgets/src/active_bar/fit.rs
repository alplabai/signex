//! The Active Bar in a window narrower than its slots: one run of slots
//! scrolls sideways inside the bar and every other slot stays in reach.
//! The renderer and the dropdown anchors read the same width from here.

use std::ops::Range;

use iced::widget::scrollable::Direction;
use iced::widget::{Row, scrollable};
use iced::{Alignment, Element, Length, Widget};
use signex_types::theme::ThemeTokens;

use super::skin::Skin;
use super::{ActiveBarItem, Metrics, bar_shell, offsets_for_widths, slot_offsets, view_item};
use crate::theme_ext::{ChromeMetrics, thin_scrollbar, thin_scrollbar_style};

/// The run never narrows below this many buttons.
const MIN_RUN_BUTTONS: usize = 3;

/// The width `run` is drawn at inside `max_width`; `None` when the whole
/// bar fits, or `run` is not a run of `items`.
fn run_width<M: 'static + Clone>(
    items: &[ActiveBarItem<M>],
    run: &Range<usize>,
    max_width: f32,
    m: &Metrics,
) -> Option<f32> {
    if run.is_empty() || run.end > items.len() {
        return None;
    }
    let (_, total) = slot_offsets(items, m);
    if total <= max_width {
        return None;
    }
    let slots = items[run.clone()].iter().map(|i| i.width(m)).sum::<f32>();
    let full = slots + m.spacing * (run.len() - 1) as f32;
    let floor = MIN_RUN_BUTTONS as f32 * m.btn + (MIN_RUN_BUTTONS - 1) as f32 * m.spacing;
    Some((max_width - (total - full)).max(floor.min(full)).min(full))
}

/// [`slot_offsets`] of the bar [`view_within`] draws. A slot inside a
/// scrolling run reports the run's left edge.
pub fn slot_offsets_within<M: 'static + Clone>(
    items: &[ActiveBarItem<M>],
    run: Range<usize>,
    max_width: f32,
    m: &Metrics,
) -> (Vec<f32>, f32) {
    let Some(width) = run_width(items, &run, max_width, m) else {
        return slot_offsets(items, m);
    };
    let mut widths: Vec<f32> = items[..run.start].iter().map(|i| i.width(m)).collect();
    widths.push(width);
    widths.extend(items[run.end..].iter().map(|i| i.width(m)));
    let (collapsed, total) = offsets_for_widths(&widths, m);
    let mut offsets = Vec::with_capacity(items.len());
    offsets.extend_from_slice(&collapsed[..run.start]);
    offsets.extend(std::iter::repeat_n(collapsed[run.start], run.len()));
    offsets.extend_from_slice(&collapsed[run.start + 1..]);
    (offsets, total)
}

/// [`super::view`] inside `max_width`: when the bar does not fit, the
/// slots of `run` scroll sideways at the width that makes it fit.
pub fn view_within<'a, M>(
    mut items: Vec<ActiveBarItem<M>>,
    run: Range<usize>,
    max_width: f32,
    tokens: &'a ThemeTokens,
) -> Element<'a, M>
where
    M: 'static + Clone,
{
    let m = Metrics::GRAPHITE;
    let Some(width) = run_width(&items, &run, max_width, &m) else {
        return super::view(items, tokens);
    };
    let tail = items.split_off(run.end);
    let middle = items.split_off(run.start);
    let slots = |list: Vec<ActiveBarItem<M>>| -> Row<Element<'a, M>> {
        list.into_iter().fold(
            Row::new().spacing(m.spacing).align_y(Alignment::Center),
            |row, item| row.push(view_item(item, tokens, &m)),
        )
    };
    // A visible thumb: a plain wheel does not move a horizontal scrollable,
    // so a hidden one would leave the far buttons out of a mouse's reach.
    // No margin, so the thumb's hit band takes the least of the buttons.
    let skin = Skin::of(tokens);
    let strip = scrollable(slots(middle))
        .direction(Direction::Horizontal(
            thin_scrollbar(&ChromeMetrics::DEFAULT).margin(0.0),
        ))
        .style(thin_scrollbar_style(skin.border, skin.icon_ink))
        .width(Length::Fixed(width))
        .height(Length::Fixed(m.btn));
    let bar = slots(items).push(strip.boxed());
    let bar = tail
        .into_iter()
        .fold(bar, |row, item| row.push(view_item(item, tokens, &m)));
    bar_shell(bar.boxed(), tokens)
}

#[cfg(test)]
mod tests {
    use super::super::ActiveBarButton;
    use super::*;

    fn bar() -> Vec<ActiveBarItem<()>> {
        let button = || ActiveBarItem::Button(ActiveBarButton::default());
        let mut items = vec![button(), ActiveBarItem::Separator];
        items.extend((0..10).map(|_| button()));
        items.extend([ActiveBarItem::Separator, button()]);
        items
    }

    const RUN: Range<usize> = 2..12;

    #[test]
    fn a_bar_that_fits_keeps_its_offsets() {
        let (items, m) = (bar(), Metrics::GRAPHITE);
        let (_, total) = slot_offsets(&items, &m);
        assert_eq!(
            slot_offsets_within(&items, RUN, total, &m),
            slot_offsets(&items, &m)
        );
    }

    #[test]
    fn a_narrow_bar_fits_and_keeps_the_slots_after_the_run_in_reach() {
        let (items, m) = (bar(), Metrics::GRAPHITE);
        let (full, total) = slot_offsets(&items, &m);
        let max = total - 100.0;
        let (offsets, fitted) = slot_offsets_within(&items, RUN, max, &m);
        assert_eq!(offsets.len(), items.len());
        assert!((fitted - max).abs() < 0.01, "{fitted} vs {max}");
        assert_eq!(offsets[..=RUN.start], full[..=RUN.start]);
        assert!(offsets[RUN].iter().all(|x| *x == full[RUN.start]));
        let last = items.len() - 1;
        assert!((offsets[last] - (full[last] - 100.0)).abs() < 0.01);
        assert!(offsets[last] + m.btn <= max);
    }

    #[test]
    fn the_run_keeps_three_buttons_in_a_tiny_window() {
        let (items, m) = (bar(), Metrics::GRAPHITE);
        let width = run_width(&items, &RUN, 1.0, &m).expect("the bar overflows");
        assert_eq!(width, 3.0 * m.btn + 2.0 * m.spacing);
    }

    #[test]
    fn a_range_outside_the_bar_scrolls_nothing() {
        let (items, m) = (bar(), Metrics::GRAPHITE);
        assert_eq!(run_width(&items, &(0..0), 1.0, &m), None);
        assert_eq!(run_width(&items, &(2..99), 1.0, &m), None);
    }
}
