//! Subscriptions about windows and the app: which window has focus, the
//! macOS system menu bar's build failures, and quit requests.

use iced::Subscription;

use crate::app::handlers::editing_commands::net_color_palette::is_pointer_press;
use crate::app::{Message, NetColorMsg, WindowMsg};

/// Which window has focus: the system menu bar acts on it.
pub(super) fn window_focus() -> Subscription<Message> {
    iced::event::listen_with(|event, _status, window| match event {
        iced::Event::Window(iced::window::Event::Focused) => {
            Some(Message::Window(WindowMsg::WindowFocused(window)))
        }
        iced::Event::Window(iced::window::Event::Unfocused) => {
            Some(Message::Window(WindowMsg::WindowUnfocused(window)))
        }
        _ => None,
    })
}

/// Presses in any window while a net colour chooser is open, so one outside
/// the palette window closes it. Captured presses count.
pub(super) fn net_color_chooser_presses(is_open: bool) -> Subscription<Message> {
    if !is_open {
        return Subscription::none();
    }
    iced::event::listen_with(|event, _status, window| {
        is_pointer_press(&event).then_some(Message::NetColor(NetColorMsg::PressedInWindow(window)))
    })
}

/// A system menu bar that could not be built, with the reason; never fires
/// off macOS.
pub(super) fn native_menu_failures() -> Subscription<Message> {
    iced::system::menu_failures().map(|reason| Message::Window(WindowMsg::NativeMenuFailed(reason)))
}

/// Quit requests from the system (macOS only). Always subscribed: with no
/// listener the runtime answers a request "quit" at once, which would skip
/// the unsaved-changes guard.
pub(super) fn quit_requests() -> Subscription<Message> {
    iced::system::quit_requests().map(|()| Message::Window(WindowMsg::QuitRequested))
}
