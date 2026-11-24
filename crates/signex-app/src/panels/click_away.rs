//! The clear backdrop behind a menu open inside a panel: a press closes the
//! menu, and neither the press nor the wheel reaches the panel under it.

use iced::widget::{Space, mouse_area, opaque};
use iced::{Element, Length, Widget as _};

use super::PanelMsg;

pub(crate) fn click_away_backdrop<'a>(on_press: PanelMsg) -> Element<'a, PanelMsg> {
    let area = mouse_area(Space::new().width(Length::Fill).height(Length::Fill)).on_press(on_press);
    opaque(area).capture_scroll(true).boxed()
}

#[cfg(test)]
mod tests {
    use iced::mouse::ScrollDelta;
    use iced::widget::{Space, Stack, mouse_area};
    use iced::{Element, Length, Point, Size, Widget as _};

    use super::click_away_backdrop;
    use crate::layout_probe::{messages_on_click_at, messages_on_scroll_at};
    use crate::panels::{LayersPanelMsg, PanelMsg};

    const WINDOW: Size = Size::new(200.0, 200.0);
    const INSIDE: Point = Point::new(100.0, 100.0);

    /// A body that reports the wheel, under the backdrop.
    fn over_a_body<'a>() -> Element<'a, PanelMsg> {
        let body = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
            .on_scroll(|_| PanelMsg::LayersPanel(LayersPanelMsg::MoreMenuToggled));
        Stack::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .push(body.boxed())
            .push(click_away_backdrop(PanelMsg::LayersPanel(
                LayersPanelMsg::MoreMenuDismissed,
            )))
            .boxed()
    }

    #[test]
    fn the_wheel_stops_at_the_backdrop() {
        let delta = ScrollDelta::Lines { x: 0.0, y: 1.0 };
        let sent = messages_on_scroll_at(over_a_body(), WINDOW, INSIDE, delta);
        assert!(sent.is_empty(), "the body scrolled: {sent:?}");
    }

    #[test]
    fn a_press_on_the_backdrop_closes_the_menu() {
        let sent = messages_on_click_at(over_a_body(), WINDOW, None, INSIDE);
        assert!(
            matches!(
                sent.as_slice(),
                [PanelMsg::LayersPanel(LayersPanelMsg::MoreMenuDismissed)]
            ),
            "{sent:?}"
        );
    }
}
