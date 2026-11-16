//! Board Reports (#2404) and Board Print / PDF (#2402). State and rules:
//! `app/board_output/`.

mod print;
mod reports;

use iced::widget::{Row, Space, column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::Localized;
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::sizes;
use crate::app::{BoardOutputDialog, BoardOutputMsg, Message, Signex};
use crate::styles::{Spacing, TypeScale, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn out(msg: BoardOutputMsg) -> Message {
    Message::from(msg)
}

impl Signex {
    pub(in crate::app::view) fn view_board_output_dialog(&self) -> Element<'_, Message> {
        let size = match self.ui_state.board_output.dialog.as_ref() {
            Some(BoardOutputDialog::Print(_)) => sizes::BOARD_PRINT,
            _ => sizes::BOARD_REPORTS,
        };
        self.wrap_centred_dialog(self.view_board_output_card(size), size)
    }

    fn view_board_output_card(&self, size: (f32, f32)) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let (header, body, footer) = match self.ui_state.board_output.dialog.as_ref() {
            Some(BoardOutputDialog::Reports(state)) => {
                let unit = crate::panels::pcb_board_properties::shown_unit(self.ui_state.unit);
                reports::parts(state, unit, i18n, tokens)
            }
            Some(BoardOutputDialog::Print(state)) => print::parts(state, i18n, tokens),
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

/// A row of toggle pills, the one for the current choice on.
fn pills<'a>(
    options: Vec<(Localized, bool, BoardOutputMsg)>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut row: Row<Element<'a, Message>> = Row::new().spacing(SP.s);
    for (label, on, msg) in options {
        row = row.push(
            button(container(text(label).size(TS.body)).padding([SP.s, 10.0]))
                .on_press(out(msg))
                .style(super::graphite_annotate::toggle_style(tokens, on))
                .boxed(),
        );
    }
    row.wrap().into()
}

/// A muted caption over a control.
fn captioned<'a>(
    caption: Localized,
    control: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    column![
        text(caption)
            .size(TS.label)
            .color(tokens::muted_text(tokens)),
        control,
    ]
    .spacing(SP.s)
    .boxed()
}
