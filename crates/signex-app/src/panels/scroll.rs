//! The vertical scroller every dock panel shares.
//!
//! iced draws a default scrollbar over the right edge of the content, so a
//! row laid out at the full width ran under it: controls hidden, hints cut.
//! Setting a spacing makes the bar embedded: the content is laid out in the
//! width that is left, and long labels wrap or shrink inside it.

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{Scrollable, scrollable};
use iced::{Element, Length};

/// Space kept between the content and the embedded scrollbar, in pixels.
const SCROLLBAR_GAP: f32 = 2.0;

/// A panel body in a full-width vertical scroller with a scrollbar gutter.
pub(crate) fn panel_scrollable<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Scrollable<'a, M> {
    scrollable(content)
        .direction(Direction::Vertical(Scrollbar::new().spacing(SCROLLBAR_GAP)))
        .width(Length::Fill)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::{find, lay_out_texts};
    use iced::widget::text;

    /// The default scrollbar width; the gutter must be at least this.
    const BAR_WIDTH: f32 = 10.0;

    fn content_width(body: Element<'static, ()>) -> f32 {
        let texts = lay_out_texts(body, iced::Size::new(400.0, 300.0));
        find(&texts, "row").bounds.width
    }

    #[test]
    fn content_is_laid_out_beside_the_scrollbar_not_under_it() {
        // Taller than the 300 px window, so the scrollbar is showing. Laid
        // out twice inside: the gutter is decided after the first pass.
        let row = || {
            iced::widget::column![
                text("row").width(Length::Fill),
                iced::widget::Space::new().height(900)
            ]
        };
        let plain = content_width(scrollable(row()).width(Length::Fill).into());
        let panel = content_width(panel_scrollable(row()).into());
        assert!(
            plain > 390.0,
            "control case: the default bar floats, {plain}"
        );
        assert!(
            panel <= 400.0 - BAR_WIDTH,
            "content still reaches under the bar: {panel}"
        );
    }
}
