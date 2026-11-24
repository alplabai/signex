//! The Update PCB from Schematic dialog (#1312, #2371, #2533): the header,
//! the count tiles and filter field (`summary.rs`), the stale or blocked
//! banner, the Problems card (`problems.rs`), the grouped list
//! (`table.rs`, rows from `change_rows.rs`) and the footer: Report... with
//! the validation result on the left, Close, Validate and Execute on the
//! right. State and rules: `app/update_pcb/`.

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::UserText;
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, text};

use self::change_rows::StatusGlyph;
use super::sizes;
use crate::app::update_pcb::{BoardStatus, Phase, UpdatePcbState};
use crate::app::{Message, Signex, UpdatePcbMsg};
use crate::styles::{
    DialogButtonKind, Radii, Spacing, TypeScale, dialog_button, dialog_footer_split, ti, tokens,
};

pub(super) mod change_rows;
mod problems;
mod schematic_rules;
mod summary;
mod table;
#[cfg(test)]
mod tests;
pub(super) mod texts;

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

/// The project's name for the subtitle: its file name without `.snxprj`.
fn project_stem(name: &str) -> String {
    std::path::Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string())
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
            "project" => project_stem(&state.project_name),
            "board" => state.board_name()
        )
        .into_string();
        // Mockup row 81: the subtitle on a second line, in the taller header.
        let header = super::header::graphite_header_stacked(
            s(i18n, ids::UPDATE_PCB_TITLE),
            subtitle,
            Some(up(UpdatePcbMsg::CloseRequested)),
            super::header::PREFERENCES_HEADER_H,
            tokens,
        );
        let mut body = Column::new().spacing(SP.m).height(Length::Fill);
        let ready = state.ready_list().filter(|l| !l.rows.is_empty());
        if let Some(list) = ready {
            body = body.push(summary::bar(list, &state.table, i18n, tokens));
        }
        body = body.push(self.update_pcb_banner(state, i18n, tokens));
        // #1489: the rules the schematic asks for, below the board's rows.
        let rules = schematic_rules::section(&self.update_pcb_rule_rows(state), i18n, tokens);
        if let Some(list) = state.list.as_ref().filter(|l| !l.rows.is_empty()) {
            if let Some(card) = problems::card(list, i18n, tokens) {
                body = body.push(card);
            }
            let rows = table::body(state, list, i18n, tokens);
            let framed = match rules {
                Some(rules) => column![rows, rules].boxed(),
                None => rows,
            };
            body = body.push(list_frame(framed, tokens));
        } else if let Some(rules) = rules {
            body = body.push(list_frame(rules, tokens));
        } else {
            body = body.push(Space::new().height(Length::Fill).boxed());
        }
        let footer = dialog_footer_split(
            self.update_pcb_footer_left(state, i18n, tokens),
            self.update_pcb_buttons(state, i18n, tokens),
        );
        let (width, height) =
            super::widgets::fit_modal_size(self.ui_state.window_size, sizes::UPDATE_PCB);
        let card = Column::new()
            .push(header)
            .push(
                container(body)
                    .padding([SP.l, SP.xl])
                    .height(Length::Fill)
                    .boxed(),
            )
            .push(footer)
            .width(Length::Fixed(width));
        container(card)
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

    /// Report... and, beside it, the last Validate's verdict.
    fn update_pcb_footer_left<'a>(
        &self,
        state: &UpdatePcbState,
        i18n: &I18n,
        tokens: &ThemeTokens,
    ) -> Vec<Element<'a, Message>> {
        let ready = state.ready_list();
        let mut left = vec![dialog_button(
            t!(i18n, ids::UPDATE_PCB_REPORT),
            DialogButtonKind::Secondary,
            ready.map(|_| up(UpdatePcbMsg::ReportRequested)),
            tokens,
        )];
        if let Some(verdict) = validation_line(state, i18n, tokens) {
            left.push(verdict);
        }
        left
    }

    fn update_pcb_buttons<'a>(
        &self,
        state: &UpdatePcbState,
        i18n: &I18n,
        tokens: &ThemeTokens,
    ) -> Vec<Element<'a, Message>> {
        let ready = state.ready_list();
        let count = state.execute_count()
            + crate::app::update_pcb::rules_from_schematic::pending(
                &self.update_pcb_rule_rows(state),
            );
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

/// The list in its hairline frame, scrolling inside the remaining height.
/// Back Annotate frames its list the same way.
pub(super) fn list_frame<'a>(
    list: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let edge = tokens::resolve(tokens).line_soft;
    let fill = tokens::resolve(tokens).field;
    container(
        scrollable(list)
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(tokens))
            .height(Length::Fill),
    )
    .height(Length::Fill)
    .clip(true)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: 1.0,
            color: edge,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The last Validate's verdict: its glyph, its words and whether it passed
/// ("Validated: ..." with the tick, mockup row 81, or the reason).
fn validation_verdict(
    state: &UpdatePcbState,
    i18n: &I18n,
) -> Option<(StatusGlyph, Localized, bool)> {
    Some(match state.validation.as_ref()? {
        Ok(()) => (
            StatusGlyph::Check,
            t!(i18n, ids::UPDATE_PCB_VALIDATED),
            true,
        ),
        Err(why) => (
            StatusGlyph::Blocked,
            t!(i18n, ids::UPDATE_PCB_INVALID, "why" => why.as_str()),
            false,
        ),
    })
}

/// The last Validate's verdict: green with the tick, or red with the reason.
fn validation_line<'a>(
    state: &UpdatePcbState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Option<Element<'a, Message>> {
    let (glyph, words, ok) = validation_verdict(state, i18n)?;
    let ink = if ok {
        ti(tokens.success)
    } else {
        tokens::danger(tokens)
    };
    Some(
        row![glyph.view(ink), text(words).size(TS.body).color(ink)]
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .boxed(),
    )
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
