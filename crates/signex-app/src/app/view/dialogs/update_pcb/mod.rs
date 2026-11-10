//! The Update PCB from Schematic dialog (#1312): summary, the stale or
//! blocked banner, the grouped rows with group checkboxes, notes and the
//! Close / Report... / Validate / Execute footer. State and rules:
//! `app/update_pcb/`.

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_engine::pcb::eco::{EcoChangeList, EcoGroup, EcoKey, EcoRow};
use signex_i18n::UserText;
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, Label, checkbox, text};

use super::sizes;
use crate::app::update_pcb::{BoardStatus, Phase, UpdatePcbState};
use crate::app::{Message, Signex, UpdatePcbMsg};
use crate::styles::{
    DialogButtonKind, Spacing, TypeScale, dialog_button, dialog_footer_split, ti, tokens,
};

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
fn line<'a>(label: impl Into<Label<'a>>, color: iced::Color) -> Element<'a, Message> {
    text(label).size(TS.body).color(color).boxed()
}

/// A small link-like button.
fn link<'a>(label: Localized, msg: Message) -> Element<'a, Message> {
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
        let mut body = Column::new()
            .spacing(SP.m)
            .push(self.update_pcb_banner(state, i18n, tokens));
        if let Some(list) = state.list.as_ref() {
            body = body
                .push(list_view(list, i18n, tokens))
                .push(notes_view(list, i18n, tokens));
        }
        if let Some(verdict) = &state.validation {
            body = body.push(match verdict {
                Ok(()) => line(t!(i18n, ids::UPDATE_PCB_VALID), tokens::accent(tokens)),
                Err(why) => line(
                    t!(i18n, ids::UPDATE_PCB_INVALID, "why" => why.as_str()),
                    tokens::danger(tokens),
                ),
            });
        }
        let body = scrollable(container(body).padding([SP.l, SP.xl])).height(Length::Fill);
        let footer = dialog_footer_split(Vec::new(), self.update_pcb_buttons(state, i18n, tokens));
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, sizes::UPDATE_PCB.0),
        )))
        .height(Length::Fixed(sizes::UPDATE_PCB.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }

    /// The summary, or what stands in the way: no board, opening, reading,
    /// a failure, a stale list, a parked or read-only board.
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
                let Some(list) = state.list.as_ref() else {
                    return Space::new().boxed();
                };
                let summary = if list.rows.is_empty() {
                    line(t!(i18n, ids::UPDATE_PCB_NOTHING), muted)
                } else {
                    line(
                        t!(
                            i18n,
                            ids::UPDATE_PCB_SUMMARY,
                            "ticked" => list.rows.iter().filter(|r| list.applies(r)).count(),
                            "total" => list.rows.len()
                        ),
                        ti(tokens.text),
                    )
                };
                let Some(why) = self.update_pcb_execute_block() else {
                    return summary;
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
                column![summary, banner(line(UserText::new(why), danger), action)]
                    .spacing(SP.s)
                    .boxed()
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
        let can_execute = ready.is_some() && self.update_pcb_execute_block().is_none();
        vec![
            dialog_button(
                &s(i18n, ids::UPDATE_PCB_CLOSE),
                DialogButtonKind::Cancel,
                Some(up(UpdatePcbMsg::CloseRequested)),
                tokens,
            ),
            dialog_button(
                &s(i18n, ids::UPDATE_PCB_REPORT),
                DialogButtonKind::Secondary,
                ready.map(|_| up(UpdatePcbMsg::ReportRequested)),
                tokens,
            ),
            dialog_button(
                &s(i18n, ids::UPDATE_PCB_VALIDATE),
                DialogButtonKind::Secondary,
                ready.map(|_| up(UpdatePcbMsg::ValidateRequested)),
                tokens,
            ),
            dialog_button(
                &s(i18n, ids::UPDATE_PCB_EXECUTE),
                super::graphite::call_to_action(tokens),
                can_execute.then(|| up(UpdatePcbMsg::ExecuteRequested)),
                tokens,
            ),
        ]
    }
}

/// A banner line with its action button on the right.
fn banner<'a>(
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

/// Every group in list order: its checkbox, then its rows.
fn list_view<'a>(list: &EcoChangeList, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let mut col = Column::new().spacing(SP.s);
    let mut shown: Option<EcoGroup> = None;
    for row in &list.rows {
        if shown != Some(row.group) {
            shown = Some(row.group);
            col = col.push(group_header(list, row.group, i18n, tokens));
        }
        col = col.push(row_view(list, row, i18n, tokens));
    }
    col.boxed()
}

/// A group's tri-state box: on when every open row is ticked, mixed when
/// some are; a click ticks them all unless all are ticked already.
fn group_header<'a>(
    list: &EcoChangeList,
    group: EcoGroup,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ticks = list.group_ticks(group);
    let target = !ticks.all();
    let mut boxed = checkbox(ticks.all() || ticks.mixed())
        .size(TS.title)
        .labelled(t!(i18n, texts::group_id(group)))
        .text_size(TS.label);
    boxed = if ticks.mixed() {
        boxed.style(crate::panels::form_look::mixed_checkbox_style)
    } else {
        boxed.style(crate::panels::form_look::checkbox_style)
    };
    if ticks.open > 0 {
        boxed = boxed.on_toggle(move |_| up(UpdatePcbMsg::GroupToggled { group, on: target }));
    }
    row![
        boxed,
        Space::new().width(Length::Fill),
        text(t!(
            i18n,
            ids::UPDATE_PCB_GROUP_COUNT,
            "on" => ticks.on,
            "open" => ticks.open
        ))
        .size(TS.body)
        .color(tokens::muted_text(tokens))
    ]
    .padding([SP.s, 0.0])
    .align_y(Alignment::Center)
    .boxed()
}

/// One row: its box (none to tick when blocked), what it does, and why it
/// is blocked with the links that fix it.
fn row_view<'a>(
    list: &EcoChangeList,
    row: &EcoRow,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut boxed = checkbox(row.on)
        .size(TS.body)
        .labelled(texts::change(i18n, &row.change))
        .text_size(TS.body)
        .style(crate::panels::form_look::checkbox_style);
    if row.blocked.is_none() {
        let key = row.key.clone();
        boxed = boxed.on_toggle(move |on| {
            up(UpdatePcbMsg::RowToggled {
                key: key.clone(),
                on,
            })
        });
    }
    let mut col = Column::new().spacing(SP.xs).push(boxed.boxed());
    if let Some(why) = &row.blocked {
        let mut fixes = row![line(texts::blocked(i18n, why), tokens::danger(tokens))]
            .spacing(SP.m)
            .align_y(Alignment::Center);
        if texts::needs_annotate(why) {
            fixes = fixes.push(link(
                t!(i18n, ids::UPDATE_PCB_FIX_ANNOTATE),
                up(UpdatePcbMsg::AnnotateRequested),
            ));
        }
        if matches!(row.key, EcoKey::AddFootprint { .. }) {
            fixes = fixes.push(link(
                t!(i18n, ids::UPDATE_PCB_FIX_CROSS_PROBE),
                up(UpdatePcbMsg::CrossProbeRequested(row.key.clone())),
            ));
        }
        col = col.push(fixes.boxed());
    } else if row.on && !list.applies(row) {
        col = col.push(line(
            t!(i18n, ids::UPDATE_PCB_ROW_SKIPPED),
            tokens::muted_text(tokens),
        ));
    }
    container(col).padding([0.0, SP.xl]).boxed()
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
