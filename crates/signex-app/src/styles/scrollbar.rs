//! The thin themed scrollbar of every Graphite list, pane and panel: no
//! visible rail, a pill thumb in `line_strong` that turns `ink4` while it is
//! hovered or dragged. Geometry and status logic live in
//! `signex_widgets::theme_ext`; this file picks the theme colours.

use iced::Theme;
use iced::widget::scrollable;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::metrics::ChromeMetrics;
use super::tokens::resolve;

/// `(thumb, thumb_hot)` colours of the thin scrollbar.
pub fn scrollbar_colors(tokens: &ThemeTokens) -> (iced::Color, iced::Color) {
    let r = resolve(tokens);
    (r.line_strong, r.ink4)
}

/// Style of the thin scrollbar for `scrollable(..).style(..)`.
pub fn scrollbar_style(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style + 'static {
    let (thumb, thumb_hot) = scrollbar_colors(tokens);
    theme_ext::thin_scrollbar_style(thumb, thumb_hot)
}

/// Geometry of the thin scrollbar at the default chrome metrics.
pub fn thin_scrollbar() -> scrollable::Scrollbar {
    theme_ext::thin_scrollbar(&ChromeMetrics::DEFAULT)
}

/// Vertical scrolling with the thin scrollbar, for `.direction(..)`.
pub fn thin_vertical() -> scrollable::Direction {
    scrollable::Direction::Vertical(thin_scrollbar())
}

/// Scrolling both ways with the thin scrollbar, for `.direction(..)`.
pub fn thin_both() -> scrollable::Direction {
    scrollable::Direction::Both {
        vertical: thin_scrollbar(),
        horizontal: thin_scrollbar(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Background;
    use signex_types::theme::{ThemeId, theme_tokens};

    /// On both themes the thumb is `line_strong` at rest and `ink4` while
    /// hovered, over a transparent rail.
    #[test]
    fn the_thumb_follows_the_theme() {
        for &id in ThemeId::BUILTINS {
            let t = theme_tokens(id);
            let r = resolve(&t);
            let style = scrollbar_style(&t);
            let idle = style(
                &Theme::Dark,
                scrollable::Status::Active {
                    is_horizontal_scrollbar_disabled: false,
                    is_vertical_scrollbar_disabled: false,
                },
            );
            assert!(idle.vertical_rail.background.is_none(), "{id:?}");
            assert_eq!(
                idle.vertical_rail.scroller.background,
                Background::Color(r.line_strong),
                "{id:?}"
            );
            let hot = style(
                &Theme::Dark,
                scrollable::Status::Hovered {
                    is_horizontal_scrollbar_hovered: false,
                    is_vertical_scrollbar_hovered: true,
                    is_horizontal_scrollbar_disabled: false,
                    is_vertical_scrollbar_disabled: false,
                },
            );
            assert_eq!(
                hot.vertical_rail.scroller.background,
                Background::Color(r.ink4),
                "{id:?}"
            );
        }
    }
}
