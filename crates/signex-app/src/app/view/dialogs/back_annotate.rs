//! The Back Annotate dialog (#1312): the Update PCB dialog's look with the
//! board as the source. Summary or what stands in the way, grouped rows with
//! a tri-state box per group, read-only rows with their reason, notes, and
//! Close / Validate / Execute. State and rules: `app/back_annotate/`.

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_engine::pcb::eco::{
    BackBlock, BackChange, BackChangeList, BackGroup, BackNote, BackRow,
};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, checkbox, text};

use super::sizes;
use super::update_pcb::{banner, line, link};
use crate::app::back_annotate::BackAnnotateState;
use crate::app::update_pcb::Phase;
use crate::app::{BackAnnotateMsg, Message, Signex};
use crate::styles::{
    DialogButtonKind, Spacing, TypeScale, dialog_button, dialog_footer_split, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn ba(msg: BackAnnotateMsg) -> Message {
    Message::from(msg)
}

fn s(i18n: &I18n, id: MessageId) -> String {
    t!(i18n, id).into_string()
}

impl Signex {
    pub(in crate::app::view) fn view_back_annotate_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_back_annotate_card(), sizes::BACK_ANNOTATE)
    }

    fn view_back_annotate_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.back_annotate.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let subtitle = t!(
            i18n,
            ids::BACK_ANNOTATE_SUBTITLE,
            "board" => state.board_name(),
            "project" => state.project_name.clone()
        )
        .into_string();
        let close = Some(ba(BackAnnotateMsg::CloseRequested));
        let header = super::header::graphite_header_closing(
            s(i18n, ids::BACK_ANNOTATE_TITLE),
            Some(subtitle),
            None,
            close.clone(),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let muted = tokens::muted_text(tokens);
        let mut body = Column::new()
            .spacing(SP.m)
            .push(line(t!(i18n, ids::BACK_ANNOTATE_INTRO), muted))
            .push(self.back_annotate_banner(state, i18n, tokens));
        if let Some(list) = state.ready_list() {
            body = body
                .push(list_view(list, state.checking, i18n, tokens))
                .push(notes_view(list, i18n, tokens));
        }
        if state.checking {
            body = body.push(line(t!(i18n, ids::BACK_ANNOTATE_CHECKING), muted));
        }
        if let Some(verdict) = state.validation() {
            body = body.push(match verdict {
                Ok(()) => line(t!(i18n, ids::BACK_ANNOTATE_VALID), tokens::accent(tokens)),
                Err(why) => line(
                    signex_i18n::UserText::new(why.clone()),
                    tokens::danger(tokens),
                ),
            });
        }
        let body = scrollable(container(body).padding([SP.l, SP.xl])).height(Length::Fill);
        let ready = state.ready_list().is_some() && !state.checking;
        let can_execute = ready
            && !self.back_annotate_stale()
            && state
                .ready_list()
                .is_some_and(|l| l.rows.iter().any(BackRow::applies));
        // A designator the ticked rows would leave on several parts is not
        // blocked (Caner, #1312), so it is said next to Execute too.
        let shared = state.ready_list().map_or(0, |list| {
            list.notes
                .iter()
                .filter(|n| matches!(n, BackNote::SharedDesignator { .. }))
                .count()
        });
        let warning: Vec<Element<'_, Message>> = (shared > 0)
            .then(|| {
                line(
                    t!(i18n, ids::BACK_ANNOTATE_SHARED_WARNING, "count" => shared),
                    crate::styles::pill_colors(tokens, crate::styles::PillTone::Warn).1,
                )
            })
            .into_iter()
            .collect();
        let footer = dialog_footer_split(
            warning,
            vec![
                dialog_button(
                    t!(i18n, ids::BACK_ANNOTATE_CLOSE),
                    DialogButtonKind::Cancel,
                    close,
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::BACK_ANNOTATE_VALIDATE),
                    DialogButtonKind::Secondary,
                    ready.then(|| ba(BackAnnotateMsg::ValidateRequested)),
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::BACK_ANNOTATE_EXECUTE),
                    super::graphite::call_to_action(tokens),
                    can_execute.then(|| ba(BackAnnotateMsg::ExecuteRequested)),
                    tokens,
                ),
            ],
        );
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, sizes::BACK_ANNOTATE.0),
        )))
        .height(Length::Fixed(sizes::BACK_ANNOTATE.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }

    /// The summary, or what stands in the way: no board, opening, reading,
    /// a failure, a stale list.
    fn back_annotate_banner<'a>(
        &self,
        state: &BackAnnotateState,
        i18n: &I18n,
        tokens: &ThemeTokens,
    ) -> Element<'a, Message> {
        let muted = tokens::muted_text(tokens);
        let danger = tokens::danger(tokens);
        match &state.phase {
            Phase::NoBoard => line(t!(i18n, ids::BACK_ANNOTATE_NO_BOARD), muted),
            Phase::Opening => line(
                t!(i18n, ids::BACK_ANNOTATE_OPENING, "board" => state.board_name()),
                muted,
            ),
            Phase::Computing => line(t!(i18n, ids::BACK_ANNOTATE_COMPUTING), muted),
            Phase::Failed(why) => line(
                t!(i18n, ids::BACK_ANNOTATE_FAILED, "why" => why.as_str()),
                danger,
            ),
            Phase::Ready => {
                let Some(list) = state.list() else {
                    return Space::new().boxed();
                };
                let summary = if list.rows.is_empty() {
                    line(t!(i18n, ids::BACK_ANNOTATE_NOTHING), muted)
                } else {
                    line(
                        t!(
                            i18n,
                            ids::BACK_ANNOTATE_SUMMARY,
                            "ticked" => list.rows.iter().filter(|r| r.applies()).count(),
                            "total" => list.rows.len()
                        ),
                        ti(tokens.text),
                    )
                };
                if !self.back_annotate_stale() {
                    return summary;
                }
                // No Refresh while Execute states the closed sheets.
                let refresh = (!state.checking).then(|| {
                    link(
                        t!(i18n, ids::BACK_ANNOTATE_REFRESH),
                        ba(BackAnnotateMsg::RefreshRequested),
                    )
                });
                column![
                    summary,
                    banner(line(t!(i18n, ids::BACK_ANNOTATE_STALE), danger), refresh)
                ]
                .spacing(SP.s)
                .boxed()
            }
        }
    }
}

fn group_id(group: BackGroup) -> MessageId {
    match group {
        BackGroup::Designators => ids::BACK_ANNOTATE_GROUP_DESIGNATORS,
        BackGroup::Values => ids::BACK_ANNOTATE_GROUP_VALUES,
        BackGroup::Footprints => ids::BACK_ANNOTATE_GROUP_FOOTPRINTS,
    }
}

/// Every group in list order: its checkbox, then its rows.
/// `locked`: Execute is stating the closed sheets, so no box can be ticked.
fn list_view<'a>(
    list: &BackChangeList,
    locked: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut col = Column::new().spacing(SP.s);
    let mut shown: Option<BackGroup> = None;
    for row in &list.rows {
        if shown != Some(row.group) {
            shown = Some(row.group);
            col = col.push(group_header(list, row.group, locked, i18n, tokens));
        }
        col = col.push(row_view(row, locked, i18n, tokens));
    }
    col.boxed()
}

/// A group's tri-state box: a click ticks every open row unless all are.
fn group_header<'a>(
    list: &BackChangeList,
    group: BackGroup,
    locked: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ticks = list.group_ticks(group);
    let target = !ticks.all();
    let mut boxed = checkbox(ticks.all() || ticks.mixed())
        .size(TS.title)
        .labelled(t!(i18n, group_id(group)))
        .text_size(TS.label);
    boxed = if ticks.mixed() {
        boxed.style(crate::panels::form_look::mixed_checkbox_style)
    } else {
        boxed.style(crate::panels::form_look::checkbox_style)
    };
    if ticks.open > 0 && !locked {
        boxed = boxed.on_toggle(move |_| ba(BackAnnotateMsg::GroupToggled { group, on: target }));
    }
    row![
        boxed,
        Space::new().width(Length::Fill),
        text(t!(
            i18n,
            ids::BACK_ANNOTATE_GROUP_COUNT,
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

/// One row: its box (none to tick when blocked), and why it is blocked.
fn row_view<'a>(
    row: &BackRow,
    locked: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut boxed = checkbox(row.on)
        .size(TS.body)
        .labelled(change_text(i18n, &row.change))
        .text_size(TS.body)
        .style(crate::panels::form_look::checkbox_style);
    if row.blocked.is_none() && !locked {
        let key = row.key;
        boxed = boxed.on_toggle(move |on| ba(BackAnnotateMsg::RowToggled { key, on }));
    }
    let mut col = Column::new().spacing(SP.xs).push(boxed.boxed());
    if let Some(why) = &row.blocked {
        col = col.push(line(block_text(i18n, why), tokens::danger(tokens)));
    }
    container(col).padding([0.0, SP.xl]).boxed()
}

fn change_text(i18n: &I18n, change: &BackChange) -> Localized {
    match change {
        BackChange::Designator { from, to, .. } => t!(
            i18n,
            ids::BACK_ANNOTATE_ROW_DESIGNATOR,
            "from" => from.as_str(),
            "to" => to.as_str()
        ),
        BackChange::Value {
            reference,
            from,
            to,
            ..
        } => t!(
            i18n,
            ids::BACK_ANNOTATE_ROW_VALUE,
            "reference" => reference.as_str(),
            "from" => from.as_str(),
            "to" => to.as_str()
        ),
        BackChange::Footprint {
            reference,
            board,
            schematic,
            ..
        } => t!(
            i18n,
            ids::BACK_ANNOTATE_ROW_FOOTPRINT,
            "reference" => reference.as_str(),
            "board" => board.as_str(),
            "schematic" => schematic.as_str()
        ),
    }
}

fn block_text(i18n: &I18n, why: &BackBlock) -> Localized {
    match why {
        BackBlock::NeedsAnnotation => t!(i18n, ids::BACK_ANNOTATE_BLOCK_NEEDS_ANNOTATION),
        BackBlock::DuplicateDesignator(designator) => t!(
            i18n,
            ids::BACK_ANNOTATE_BLOCK_DUPLICATE,
            "designator" => designator.as_str()
        ),
        BackBlock::BoardNotAnnotated => t!(i18n, ids::BACK_ANNOTATE_BLOCK_BOARD_NOT_ANNOTATED),
        BackBlock::SharedSheet => t!(i18n, ids::BACK_ANNOTATE_BLOCK_SHARED_SHEET),
        BackBlock::FootprintFromBoard => t!(i18n, ids::BACK_ANNOTATE_BLOCK_FOOTPRINT),
    }
}

fn note_text(i18n: &I18n, note: &BackNote) -> Localized {
    match note {
        BackNote::UnreadableSheet(_) => t!(i18n, ids::BACK_ANNOTATE_NOTE_UNREADABLE),
        BackNote::Unlinked { reference } => t!(
            i18n,
            ids::BACK_ANNOTATE_NOTE_UNLINKED,
            "reference" => reference.as_str()
        ),
        BackNote::PartGone { reference } => t!(
            i18n,
            ids::BACK_ANNOTATE_NOTE_PART_GONE,
            "reference" => reference.as_str()
        ),
        BackNote::PartNotRead { designator } => t!(
            i18n,
            ids::BACK_ANNOTATE_NOTE_PART_NOT_READ,
            "designator" => designator.as_str()
        ),
        BackNote::SharedDesignator { designator, count } => t!(
            i18n,
            ids::BACK_ANNOTATE_NOTE_SHARED,
            "designator" => designator.as_str(),
            "count" => *count
        ),
        BackNote::ExtraLink {
            reference,
            designator,
        } => t!(
            i18n,
            ids::BACK_ANNOTATE_NOTE_EXTRA_LINK,
            "reference" => reference.as_str(),
            "designator" => designator.as_str()
        ),
    }
}

fn notes_view<'a>(
    list: &BackChangeList,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    if list.notes.is_empty() {
        return Space::new().boxed();
    }
    let muted = tokens::muted_text(tokens);
    let mut col = Column::new().spacing(SP.xs).push(line(
        t!(i18n, ids::BACK_ANNOTATE_NOTES_HEADING),
        ti(tokens.text),
    ));
    for note in &list.notes {
        col = col.push(line(note_text(i18n, note), muted));
    }
    col.boxed()
}
