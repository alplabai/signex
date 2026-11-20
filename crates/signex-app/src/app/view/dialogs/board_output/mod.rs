//! Board Reports (#2404; laid out as mockup row 117) and Board Print / PDF (#2402;
//! laid out as mockup row 116). State and rules: `app/board_output/`.

pub(super) mod kit;
mod print;
mod print_plan;
mod reports;
mod reports_table;

#[cfg(test)]
mod tests_print;
#[cfg(test)]
mod tests_reports;

use iced::widget::{Space, column, container};
use iced::{Element, Length, Widget as _};

use super::sizes;
use crate::app::{BoardOutputDialog, BoardOutputMsg, Message, Signex};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn out(msg: BoardOutputMsg) -> Message {
    Message::from(msg)
}

impl Signex {
    pub(in crate::app::view) fn view_board_output_dialog(&self) -> Element<'_, Message> {
        let size = match self.ui_state.board_output.dialog.as_deref() {
            Some(BoardOutputDialog::Print(_)) => sizes::BOARD_PRINT,
            _ => sizes::BOARD_REPORTS,
        };
        self.wrap_centred_dialog(self.view_board_output_card(size), size)
    }

    fn view_board_output_card(&self, size: (f32, f32)) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let (header, body, footer) = match self.ui_state.board_output.dialog.as_deref() {
            Some(BoardOutputDialog::Reports(state)) => {
                let unit = crate::panels::pcb_board_properties::shown_unit(self.ui_state.unit);
                reports::parts(state, unit, i18n, tokens)
            }
            Some(BoardOutputDialog::Print(state)) => {
                let board = self.document_state.pcb_board(&state.board);
                print::parts(state, board, i18n, tokens)
            }
            None => return container(Space::new()).boxed(),
        };
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, size.0),
        )))
        .height(Length::Fixed(size.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}
