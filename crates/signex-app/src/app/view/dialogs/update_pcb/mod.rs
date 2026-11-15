//! The Update PCB from Schematic dialog (#1312, #2371): header, the summary
//! chips and the stale or blocked banner, the table (`table.rs`) scrolling
//! between the fixed column headings and the footer, the notes, and the
//! Close / Report... / Validate / Execute footer. State and rules:
//! `app/update_pcb/`.

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_engine::pcb::eco::EcoChangeList;
use signex_i18n::UserText;
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, text};

use super::sizes;
use crate::app::update_pcb::{BoardStatus, Phase, UpdatePcbState};
use crate::app::{Message, Signex, UpdatePcbMsg};
use crate::styles::{
    DialogButtonKind, Spacing, TypeScale, dialog_button, dialog_footer_split, ti, tokens,
};

mod summary;
mod table;
#[cfg(test)]
mod tests;
mod texts;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn up(msg: UpdatePcbMsg) -> Message {
    Message::from(msg)
}

fn s(i18n: &I18n, id: signex_i18n::MessageId) -> String {
    t!(i18n, id).into_string()
}

/// A line of text in `color`.
pub(super) fn line<'a>(label: impl Into<Label<'a>>, color: iced::Color) -> Element<'a, Message> {
    text(label).size(TS.body).color(color).boxed()
}

/// A small link-like button.
pub(super) fn link<'a>(label: Localized, msg: Message) -> Element<'a, Message> {
    signex_widgets::button(text(label).size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press(msg)
        .style(crate::panels::form_look::ghost_button)
        .boxed()
}

impl Signex {
    pub(in crate::app::view) fn view_update_pcb_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_update_pcb_card(), sizes::UPDATE_PCB)
    }

    fn view_update_pcb_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.update_pcb.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let subtitle = t!(
            i18n,
            ids::UPDATE_PCB_SUBTITLE,
            "project" => state.project_name.clone(),
            "board" => state.board_name()
        )
        .into_string();
        let header = super::header::graphite_header_closing(
            s(i18n, ids::UPDATE_PCB_TITLE),
            Some(subtitle),
            None,
            Some(up(UpdatePcbMsg::CloseRequested)),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let mut top = Column::new().spacing(SP.m);
        if let Some(list) = state.ready_list().filter(|l| !l.rows.is_empty()) {
            top = top.push(summary::bar(list, &state.table, i18n, tokens));
        }
        top = top.push(self.update_pcb_banner(state, i18n, tokens));
        let mut card = Column::new()
            .push(header)
            .push(container(top).padding([SP.l, SP.xl]).boxed());
        if let Some(list) = state.list.as_ref().filter(|l| !l.rows.is_empty()) {
            let scrolled = column![
                table::body(state, list, i18n, tokens),
                container(notes_view(list, i18n, tokens)).padding([SP.m, SP.xl]),
            ];
            card = card
                .push(table::headings(i18n, tokens))
                .push(scrollable(scrolled).height(Length::Fill).boxed());
        } else {
            card = card.push(Space::new().height(Length::Fill).boxed());
        }
        let footer = dialog_footer_split(
            validation_line(state, i18n, tokens).into_iter().collect(),
            self.update_pcb_buttons(state, i18n, tokens),
        );
        let (width, height) =
            super::widgets::fit_modal_size(self.ui_state.window_size, sizes::UPDATE_PCB);
        container(card.push(footer).width(Length::Fixed(width)))
            .height(Length::Fixed(height))
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .boxed()
    }

    /// What stands in the way of a list: no board, opening, reading, a
    /// failure, an empty list, a stale list, a parked or read-only board.
    fn update_pcb_banner<'a>(
        &self,
        state: &UpdatePcbState,
        i18n: &I18n,
        tokens: &ThemeTokens,
    ) -> Element<'a, Message> {
        let muted = tokens::muted_text(tokens);
        let danger = tokens::danger(tokens);
        let board = state.board_name();
        match &state.phase {
            Phase::NoBoard => banner(
                line(t!(i18n, ids::UPDATE_PCB_NO_BOARD), muted),
                Some(link(
                    t!(i18n, ids::UPDATE_PCB_CREATE_PCB),
                    up(UpdatePcbMsg::CreatePcbRequested),
                )),
            ),
            Phase::Opening => line(t!(i18n, ids::UPDATE_PCB_OPENING, "board" => board), muted),
            Phase::Computing => line(t!(i18n, ids::UPDATE_PCB_COMPUTING), muted),
            Phase::Failed(why) => line(
                t!(i18n, ids::UPDATE_PCB_FAILED, "why" => why.as_str()),
                danger,
            ),
            Phase::Ready => {
                if state.list.as_ref().is_some_and(|l| l.rows.is_empty()) {
                    return line(t!(i18n, ids::UPDATE_PCB_NOTHING), muted);
                }
                let Some(why) = self.update_pcb_execute_block() else {
                    return Space::new().boxed();
                };
                let action = match self.update_pcb_board_status() {
                    Some(BoardStatus::Parked) => Some(link(
                        t!(i18n, ids::UPDATE_PCB_REOPEN),
                        up(UpdatePcbMsg::ReopenRequested),
                    )),
                    Some(BoardStatus::Editable) => Some(link(
                        t!(i18n, ids::UPDATE_PCB_REFRESH),
                        up(UpdatePcbMsg::RefreshRequested),
                    )),
                    Some(BoardStatus::ReadOnly) | None => None,
                };
                banner(line(UserText::new(why), danger), action)
            }
        }
    }

    fn update_pcb_buttons<'a>(
        &self,
        state: &UpdatePcbState,
        i18n: &I18n,
        tokens: &ThemeTokens,
    ) -> Vec<Element<'a, Message>> {
        let ready = state.ready_list();
        let count = state.execute_count();
        let can_execute = ready.is_some() && count > 0 && self.update_pcb_execute_block().is_none();
        let execute = match ready {
            Some(_) => t!(i18n, ids::UPDATE_PCB_EXECUTE_COUNT, "count" => count),
            None => t!(i18n, ids::UPDATE_PCB_EXECUTE),
        };
        vec![
            dialog_button(
                t!(i18n, ids::UPDATE_PCB_CLOSE),
                DialogButtonKind::Cancel,
                Some(up(UpdatePcbMsg::CloseRequested)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::UPDATE_PCB_REPORT),
                DialogButtonKind::Secondary,
                ready.map(|_| up(UpdatePcbMsg::ReportRequested)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::UPDATE_PCB_VALIDATE),
                DialogButtonKind::Secondary,
                ready.map(|_| up(UpdatePcbMsg::ValidateRequested)),
                tokens,
            ),
            dialog_button(
                execute,
                super::graphite::call_to_action(tokens),
                can_execute.then(|| up(UpdatePcbMsg::ExecuteRequested)),
                tokens,
            ),
        ]
    }
}

/// The last Validate's verdict, left of the footer buttons.
fn validation_line<'a>(
    state: &UpdatePcbState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Option<Element<'a, Message>> {
    Some(match state.validation.as_ref()? {
        Ok(()) => line(t!(i18n, ids::UPDATE_PCB_VALID), tokens::accent(tokens)),
        Err(why) => line(
            t!(i18n, ids::UPDATE_PCB_INVALID, "why" => why.as_str()),
            tokens::danger(tokens),
        ),
    })
}

/// A banner line with its action button on the right.
pub(super) fn banner<'a>(
    message: Element<'a, Message>,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut bar = row![message, Space::new().width(Length::Fill)]
        .spacing(SP.m)
        .align_y(Alignment::Center);
    if let Some(action) = action {
        bar = bar.push(action);
    }
    bar.boxed()
}

fn notes_view<'a>(list: &EcoChangeList, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    if list.notes.is_empty() {
        return Space::new().boxed();
    }
    let muted = tokens::muted_text(tokens);
    let mut col = Column::new().spacing(SP.xs).push(line(
        t!(i18n, ids::UPDATE_PCB_NOTES_HEADING),
        ti(tokens.text),
    ));
    for note in &list.notes {
        col = col.push(line(texts::note(i18n, note), muted));
    }
    col.boxed()
}
