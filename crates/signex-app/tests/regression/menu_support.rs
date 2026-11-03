//! Shared by the regression tests that press a main-menu row.

use signex_app::app::Message;
use signex_app::menu_bar::MenuMessage;

/// The message a main-menu row sends for command `id`.
pub fn menu_row(id: &str) -> Message {
    Message::Menu(MenuMessage::Command(
        signex_app::keymap::AppCommandId::new(id).expect("a command id"),
    ))
}
