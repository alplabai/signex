//! The canvas of a board in sketch mode (#2234): the footprint editor's
//! sketch canvas over the board, with its DOF chip. The board's own bar
//! (Apply, Leave, Discard) is the app's, layered on top.

use iced::widget::Stack;
use iced::{Element, Widget as _};
use signex_i18n::I18n;
use signex_types::theme::ThemeTokens;

use crate::app::FootprintDocument;
use crate::library::messages::LibraryMessage;

/// The sketch canvas and DOF chip of `editor`, a board's sketch editor, on
/// the dark board ground.
pub fn view_board_sketch<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let look = crate::library::editor::footprint::canvas::graphite_look_for(tokens);
    Stack::new()
        .push(super::footprint::view_footprint_canvas(
            editor, tokens, look.bg, look.dot,
        ))
        .push(super::dof_chip::overlay(editor, tokens, i18n))
        .boxed()
}
