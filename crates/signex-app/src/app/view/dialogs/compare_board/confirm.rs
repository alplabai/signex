//! The confirmation before Apply when a chosen row deletes from the board
//! (mockup row 162): "Apply N changes?", one line per deletion ("Removes TP3 from
//! the board"), Cancel / Apply. Cancel applies nothing.

use iced::widget::{Column, Space, column, container};
use iced::{Element, Length, Widget as _};
use signex_engine::pcb::eco::{CompareObject, CompareRow};
use signex_i18n::{I18n, Localized, ids, t};
use signex_widgets::text;

use super::{SP, TS, cb};
use crate::app::{CompareBoardMsg, Message, Signex};
use crate::styles::{DialogButtonKind, dialog_button, dialog_footer, ti};

/// Design size of the confirmation card.
const SIZE: (f32, f32) = (440.0, 260.0);

fn deletion_text(i18n: &I18n, row: &CompareRow) -> Localized {
    match &row.object {
        CompareObject::Net(net) => {
            t!(i18n, ids::COMPARE_BOARD_CONFIRM_REMOVES_NET, "net" => net.clone())
        }
        CompareObject::Designator(object) => {
            t!(i18n, ids::COMPARE_BOARD_CONFIRM_REMOVES, "object" => object.clone())
        }
        CompareObject::Pin { reference, pad } => t!(
            i18n,
            ids::COMPARE_BOARD_CONFIRM_REMOVES,
            "object" => t!(i18n, ids::COMPARE_BOARD_PIN, "part" => reference.clone(), "pin" => pad.clone())
                .into_string()
        ),
    }
}

impl Signex {
    pub(in crate::app::view) fn view_compare_board_confirm(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_compare_board_confirm_card(), SIZE)
    }

    fn view_compare_board_confirm_card(&self) -> Element<'_, Message> {
        let Some(report) = self
            .ui_state
            .compare_board
            .as_ref()
            .and_then(|state| state.report())
        else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_view.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let count = report.counts().applied();
        let mut lines = Column::new().spacing(SP.xs);
        for row in report.board_deletions() {
            lines = lines.push(
                text(deletion_text(i18n, row))
                    .size(TS.body)
                    .color(ti(tokens.text)),
            );
        }
        let title = text(t!(i18n, ids::COMPARE_BOARD_CONFIRM_TITLE, "count" => count))
            .size(TS.title)
            .color(ti(tokens.text));
        let body = container(column![title, lines].spacing(SP.m))
            .padding([SP.l, SP.xl])
            .width(Length::Fill)
            .height(Length::Fill);
        let footer = dialog_footer(vec![
            dialog_button(
                t!(i18n, ids::COMPARE_BOARD_CONFIRM_CANCEL),
                DialogButtonKind::Cancel,
                Some(cb(CompareBoardMsg::ApplyCancelled)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::COMPARE_BOARD_CONFIRM_APPLY),
                DialogButtonKind::Primary,
                Some(cb(CompareBoardMsg::ApplyConfirmed)),
                tokens,
            ),
        ]);
        container(column![body, footer].width(Length::Fixed(
            super::super::widgets::modal_card_width(self.ui_state.window_size, SIZE.0),
        )))
        .height(Length::Fixed(SIZE.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}
