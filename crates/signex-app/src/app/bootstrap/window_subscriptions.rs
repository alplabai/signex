//! Subscriptions about windows and the app: which window has focus, the
//! macOS system menu bar's build failures, and quit requests.

use iced::Subscription;

use crate::app::{Message, WindowMsg};

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
