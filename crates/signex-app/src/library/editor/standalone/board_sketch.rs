//! The canvas of a board in sketch mode (#2234): the footprint editor's
//! sketch canvas over the board, with its chrome row. The board's own bar
//! (Apply, Leave, Discard) is the app's, layered on top.

use iced::widget::Stack;
use iced::{Element, Widget as _};
use signex_i18n::I18n;
use signex_types::theme::ThemeTokens;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_mode::mirror_card::{
    MirrorCard, overlay as mirror_overlay,
};
use crate::library::messages::LibraryMessage;

/// The sketch canvas and chrome row of `editor`, a board's sketch editor,
/// on the dark board ground, with the Mirror card (mockup row 191) while
/// Mirror is armed; `mirror_card` is the editor's open card.
pub fn view_board_sketch<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    mirror_card: Option<&MirrorCard>,
) -> Element<'a, LibraryMessage> {
    let look = crate::library::editor::footprint::graphite_look_for(tokens);
    Stack::new()
        .push(super::footprint::view_footprint_canvas(
            editor,
            tokens,
            (look.bg, look.dot),
            mirror_card,
        ))
        .push(super::dof_chip::chrome_row(
            editor,
            tokens,
            i18n,
            super::sketch_info_bar::board_bar(editor, tokens, i18n),
        ))
        .push(
            crate::library::editor::footprint::parameters_table::view::overlay(
                editor, tokens, i18n,
            ),
        )
        .push(mirror_overlay(editor, mirror_card, tokens, i18n))
        .boxed()
}

#[cfg(test)]
#[path = "board_sketch_tests.rs"]
mod tests;
