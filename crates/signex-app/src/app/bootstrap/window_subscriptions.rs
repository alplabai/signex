//! Subscriptions about windows and the app: which window has focus, files
//! dropped on a window, the macOS system menu bar's build failures, and
//! quit requests.

use iced::Subscription;

use crate::app::handlers::canvas::ends_held_resume_esc;
use crate::app::handlers::editing_commands::net_color_palette::is_pointer_press;
use crate::app::{GerberViewerMsg, InputMsg, Message, NetColorMsg, WindowMsg};

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

/// The end of a Components panel column resize, while one is in progress:
/// a release anywhere, captured or not, or the window losing the pointer
/// (#3236). The table's own `on_release` only sees releases over the
/// table.
pub(super) fn column_resize_end(is_resizing: bool) -> Subscription<Message> {
    if !is_resizing {
        return Subscription::none();
    }
    iced::event::listen_with(|event, _status, _window| {
        crate::panels::components_panel::table::ends_column_resize(&event).then_some(
            Message::Library(crate::library::LibraryMessage::ComponentsTable(
                crate::library::messages::ComponentsTableMsg::ResizeEnded,
            )),
        )
    })
}

/// The end of the Esc press that resumed a dimension, while it is held.
/// Captured events count: a widget taking the release must not leave it held.
pub(super) fn held_escape_end(is_held: bool) -> Subscription<Message> {
    if !is_held {
        return Subscription::none();
    }
    iced::event::listen_with(|event, _status, _window| {
        ends_held_resume_esc(&event).then_some(Message::Input(InputMsg::HeldEscapeEnded))
    })
}

/// Files dragged over and dropped on any window (mockup rows 193 and 193f):
/// the Gerber Viewer counts them while they hover and opens them once they
/// land. Captured events count: a widget under the pointer must not keep a
/// drop from the app.
pub(super) fn file_drops() -> Subscription<Message> {
    iced::event::listen_with(|event, _status, window| {
        let iced::Event::Window(event) = event else {
            return None;
        };
        let msg = match event {
            iced::window::Event::FileHovered(path) => GerberViewerMsg::FileHovered(path),
            iced::window::Event::FileDropped(path) => GerberViewerMsg::FileDropped { window, path },
            iced::window::Event::FilesHoveredLeft => GerberViewerMsg::FilesHoverLeft,
            _ => return None,
        };
        Some(Message::GerberViewer(msg))
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
