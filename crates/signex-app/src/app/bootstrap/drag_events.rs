//! The pointer events a drag in flight is driven by, as the subscription
//! filters them. A plain drag takes only the events no widget captured, so a
//! widget that handles its own release keeps it. A document tab dragged over
//! the document area is the exception (mockup row 207): the canvas under the
//! pointer captures the moves and the release, and a tab let go there must
//! still drop on a pane edge, so those come through as well.

use crate::app::{ContextMenuMsg, Message, PaneMsg, UiMsg, WindowMsg};
use iced::event::Status;
use iced::mouse::{Button, Event as Mouse};

/// A drag in flight: the cursor's moves and the left release, with the
/// window they happened in, and a left press closes a context menu. Only
/// events no widget captured.
pub(super) fn drag_events(
    event: iced::Event,
    status: Status,
    window: iced::window::Id,
) -> Option<Message> {
    if status == Status::Captured {
        return None;
    }
    match event {
        iced::Event::Mouse(Mouse::CursorMoved { position }) => {
            Some(Message::Ui(UiMsg::DragMovedIn {
                window,
                x: position.x,
                y: position.y,
            }))
        }
        iced::Event::Mouse(Mouse::ButtonReleased(Button::Left)) => {
            Some(Message::Ui(UiMsg::DragEnd(window)))
        }
        iced::Event::Mouse(Mouse::ButtonPressed(Button::Left)) => {
            Some(Message::ContextMenu(ContextMenuMsg::Close))
        }
        // Window::Resized is carried, with its id, by the
        // `window::resize_events()` subscription.
        _ => None,
    }
}

/// A document tab dragged: [`drag_events`], and also the moves and the left
/// release a widget captured. A captured release is `PaneMsg::TabReleased`,
/// not `DragEnd`: the tab strip's own release (a reorder) is captured too,
/// and only a release over the document area ends the drag there.
pub(super) fn tab_drag_events(
    event: iced::Event,
    status: Status,
    window: iced::window::Id,
) -> Option<Message> {
    if status != Status::Captured {
        return drag_events(event, status, window);
    }
    match event {
        iced::Event::Mouse(Mouse::CursorMoved { .. }) => {
            drag_events(event, Status::Ignored, window)
        }
        iced::Event::Mouse(Mouse::ButtonReleased(Button::Left)) => Some(Message::Window(
            WindowMsg::Pane(PaneMsg::TabReleased(window)),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn released() -> iced::Event {
        iced::Event::Mouse(Mouse::ButtonReleased(Button::Left))
    }

    fn moved() -> iced::Event {
        iced::Event::Mouse(Mouse::CursorMoved {
            position: iced::Point::new(640.0, 300.0),
        })
    }

    /// A plain drag leaves a captured release to the widget that took it.
    #[test]
    fn a_plain_drag_ignores_what_a_widget_captured() {
        let window = iced::window::Id::unique();
        assert!(drag_events(released(), Status::Captured, window).is_none());
        assert!(matches!(
            drag_events(released(), Status::Ignored, window),
            Some(Message::Ui(UiMsg::DragEnd(id))) if id == window
        ));
    }

    /// Mockup row 207: a tab let go over the canvas, which captured the
    /// release, still reaches the drop; the moves over it still count.
    #[test]
    fn a_tab_drag_hears_the_release_and_the_moves_the_canvas_captured() {
        let window = iced::window::Id::unique();
        assert!(matches!(
            tab_drag_events(released(), Status::Captured, window),
            Some(Message::Window(WindowMsg::Pane(PaneMsg::TabReleased(id)))) if id == window
        ));
        assert!(matches!(
            tab_drag_events(moved(), Status::Captured, window),
            Some(Message::Ui(UiMsg::DragMovedIn { x, y, .. })) if x == 640.0 && y == 300.0
        ));
        assert!(matches!(
            tab_drag_events(released(), Status::Ignored, window),
            Some(Message::Ui(UiMsg::DragEnd(_)))
        ));
    }
}
