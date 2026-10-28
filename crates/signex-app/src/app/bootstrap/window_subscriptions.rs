//! Subscriptions about windows: which one has focus, and the macOS system
//! menu bar's row activations.

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

/// Row activations of the macOS system menu bar; empty elsewhere.
#[cfg(target_os = "macos")]
pub(super) fn native_menu_activations() -> Subscription<Message> {
    use iced::futures::StreamExt;
    fn stream() -> impl iced::futures::Stream<Item = Message> {
        crate::menu_bar::native::activations()
            .map(|id| Message::Window(WindowMsg::NativeMenuItemActivated(id)))
    }
    Subscription::run(stream)
}

#[cfg(not(target_os = "macos"))]
pub(super) fn native_menu_activations() -> Subscription<Message> {
    Subscription::none()
}
