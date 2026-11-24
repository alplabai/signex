//! The Back Annotate dialog (#1312, #2533): the Update PCB row and group
//! language with the board as the source. The intro, what stands in the way
//! or the duplicate-designator banner, the groups Designators, Values,
//! Footprints and Notes, and the footer: the ticked count and the
//! validation result on the left, Close, Validate and Execute on the right.
//! State and rules: `app/back_annotate/`.

use std::collections::BTreeMap;

use iced::widget::{Column, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_engine::pcb::eco::{
    BackBlock, BackChange, BackChangeList, BackGroup, BackKey, BackNote, BackRow,
};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, text};

use super::sizes;
use super::update_pcb::change_rows::{
    ChangeRow, GroupHeader, RowStatus, StatusGlyph, change_row, detail_text, group_header,
};
use super::update_pcb::{banner, line, link, list_frame};
use crate::app::back_annotate::BackAnnotateState;
use crate::app::back_annotate::duplicates::DuplicateRename;
use crate::app::update_pcb::Phase;
use crate::app::{BackAnnotateMsg, Message, Signex};
use crate::styles::{
    DialogButtonKind, PillTone, Spacing, TypeScale, dialog_button, dialog_footer_split, pill,
    pill_colors, ti, tokens,
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
        let flagged = state.duplicate_renames();
        let mut body = Column::new()
            .spacing(SP.m)
            .height(Length::Fill)
            .push(line(t!(i18n, ids::BACK_ANNOTATE_INTRO), muted))
            .push(self.back_annotate_banner(state, &flagged, i18n, tokens));
        if state.checking {
            body = body.push(line(t!(i18n, ids::BACK_ANNOTATE_CHECKING), muted));
        }
        match state
            .ready_list()
            .filter(|l| !l.rows.is_empty() || !l.notes.is_empty())
        {
            Some(list) => {
                body = body.push(list_frame(
                    list_view(state, list, &flagged, i18n, tokens),
                    tokens,
                ));
            }
            None => body = body.push(Space::new().height(Length::Fill).boxed()),
        }
        let ready = state.ready_list().is_some() && !state.checking;
        let count = state
            .ready_list()
            .map_or(0, |l| l.rows.iter().filter(|r| r.applies()).count());
        let can_execute = ready && !self.back_annotate_stale() && count > 0;
        let footer = dialog_footer_split(
            footer_left(state, i18n, tokens),
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
                    match state.ready_list() {
                        Some(_) => t!(i18n, ids::BACK_ANNOTATE_EXECUTE_COUNT, "count" => count),
                        None => t!(i18n, ids::BACK_ANNOTATE_EXECUTE),
                    },
                    super::graphite::call_to_action(tokens),
                    can_execute.then(|| ba(BackAnnotateMsg::ExecuteRequested)),
                    tokens,
                ),
            ],
        );
        let body = container(body).padding([SP.l, SP.xl]).height(Length::Fill);
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, sizes::BACK_ANNOTATE.0),
        )))
        .height(Length::Fixed(sizes::BACK_ANNOTATE.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }

    /// What stands in the way (no board, opening, reading, a failure, a
    /// stale list), else the banner on renames that would put a designator
    /// on two parts, with a link to the first such row.
    fn back_annotate_banner<'a>(
        &self,
        state: &BackAnnotateState,
        flagged: &BTreeMap<BackKey, DuplicateRename>,
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
                if self.back_annotate_stale() {
                    // No Refresh while Execute states the closed sheets.
                    let refresh = (!state.checking).then(|| {
                        link(
                            t!(i18n, ids::BACK_ANNOTATE_REFRESH),
                            ba(BackAnnotateMsg::RefreshRequested),
                        )
                    });
                    return callout(
                        banner(line(t!(i18n, ids::BACK_ANNOTATE_STALE), danger), refresh),
                        PillTone::Err,
                        tokens,
                    );
                }
                if list.rows.is_empty() {
                    return line(t!(i18n, ids::BACK_ANNOTATE_NOTHING), muted);
                }
                duplicate_banner(list, flagged, i18n, tokens)
            }
        }
    }
}

/// The banner on flagged renames: a warning while one is ticked (the
/// designator would be on several parts), else a note that they start
/// unticked. Its Show link opens and highlights the first flagged row.
fn duplicate_banner<'a>(
    list: &BackChangeList,
    flagged: &BTreeMap<BackKey, DuplicateRename>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let shared = list
        .notes
        .iter()
        .filter(|n| matches!(n, BackNote::SharedDesignator { .. }))
        .count();
    let ticked = list
        .rows
        .iter()
        .find(|r| r.applies() && flagged.contains_key(&r.key));
    let first = ticked
        .map(|r| r.key)
        .or_else(|| flagged.keys().next().copied());
    let Some(first) = first else {
        return Space::new().boxed();
    };
    let words = if shared > 0 {
        t!(i18n, ids::BACK_ANNOTATE_SHARED_WARNING, "count" => shared)
    } else {
        t!(i18n, ids::BACK_ANNOTATE_DUPLICATES_UNTICKED, "count" => flagged.len())
    };
    // Mockup row 82: a designator on two parts is a warning either way.
    let message = row![
        StatusGlyph::Warn.view(ti(tokens.warning)),
        text(words).size(TS.body).color(ti(tokens.text)),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed();
    let show = link(
        t!(i18n, ids::BACK_ANNOTATE_SHOW_ROW),
        ba(BackAnnotateMsg::DuplicateShown(first)),
    );
    callout(banner(message, Some(show)), PillTone::Warn, tokens)
}

/// A banner on the soft fill of its tone.
fn callout<'a>(
    content: Element<'a, Message>,
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (fill, edge) = pill_colors(tokens, tone);
    container(content)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: edge,
                radius: crate::styles::Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The ticked count as a pill and, beside it, the last Validate's verdict.
pub(super) fn footer_left<'a>(
    state: &BackAnnotateState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Vec<Element<'a, Message>> {
    let mut left = Vec::new();
    if let Some(list) = state.ready_list().filter(|l| !l.rows.is_empty()) {
        let ink = pill_colors(tokens, PillTone::Accent).1;
        left.push(
            container(
                text(t!(
                    i18n,
                    ids::BACK_ANNOTATE_SUMMARY,
                    "ticked" => list.rows.iter().filter(|r| r.applies()).count(),
                    "total" => list.rows.len()
                ))
                .size(TS.caption)
                .color(ink),
            )
            .padding([0.0, SP.s])
            .style(pill(tokens, PillTone::Accent))
            .boxed(),
        );
    }
    if let Some(verdict) = state.validation() {
        let (glyph, words, ink): (StatusGlyph, Label<'a>, iced::Color) = match verdict {
            Ok(()) => (
                StatusGlyph::Check,
                t!(i18n, ids::BACK_ANNOTATE_VALID).into(),
                ti(tokens.success),
            ),
            Err(why) => (
                StatusGlyph::Blocked,
                UserText::new(why.clone()).into(),
                tokens::danger(tokens),
            ),
        };
        left.push(
            row![glyph.view(ink), text(words).size(TS.body).color(ink)]
                .spacing(SP.s)
                .align_y(Alignment::Center)
                .boxed(),
        );
    }
    left
}

fn group_id(group: BackGroup) -> MessageId {
    match group {
        BackGroup::Designators => ids::BACK_ANNOTATE_GROUP_DESIGNATORS,
        BackGroup::Values => ids::BACK_ANNOTATE_GROUP_VALUES,
        BackGroup::Footprints => ids::BACK_ANNOTATE_GROUP_FOOTPRINTS,
    }
}

/// Every group in list order: its header, then its rows unless folded; the
/// Notes group last. While Execute states the closed sheets
/// (`state.checking`) no box can be ticked.
fn list_view<'a>(
    state: &BackAnnotateState,
    list: &BackChangeList,
    flagged: &BTreeMap<BackKey, DuplicateRename>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut col = Column::new();
    let mut shown: Option<BackGroup> = None;
    for row in &list.rows {
        let folded = state.is_folded(row.group);
        if shown != Some(row.group) {
            shown = Some(row.group);
            col = col.push(heading(
                list,
                row.group,
                folded,
                state.checking,
                i18n,
                tokens,
            ));
        }
        if !folded {
            col = col.push(row_view(state, row, flagged.get(&row.key), i18n, tokens));
        }
    }
    if !list.notes.is_empty() {
        col = col.push(notes_view(state, list, i18n, tokens));
    }
    col.width(Length::Fill).boxed()
}

/// A group's header: a click on its box ticks every open row unless all
/// are.
fn heading<'a>(
    list: &BackChangeList,
    group: BackGroup,
    folded: bool,
    locked: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ticks = list.group_ticks(group);
    let target = !ticks.all();
    let total = list.rows.iter().filter(|r| r.group == group).count();
    group_header(
        GroupHeader {
            title: t!(i18n, group_id(group)),
            ticks: Some(ticks),
            on_tick: (ticks.open > 0 && !locked)
                .then(|| ba(BackAnnotateMsg::GroupToggled { group, on: target })),
            folded,
            on_fold: ba(BackAnnotateMsg::GroupFolded {
                group,
                folded: !folded,
            }),
            count: t!(
                i18n,
                ids::BACK_ANNOTATE_GROUP_COUNT,
                "on" => ticks.on,
                "open" => total
            ),
            split: None,
            mark: None,
        },
        i18n,
        tokens,
    )
}

/// One row: its box (none to tick when blocked or busy), the change, and
/// under it why it is blocked (red), why it is listed only (grey) or what
/// the rename would duplicate (amber).
fn row_view<'a>(
    state: &BackAnnotateState,
    row: &BackRow,
    flag: Option<&DuplicateRename>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let locked = state.checking;
    let (status, caption) = row_status(state, row, flag, i18n, tokens);
    let on_toggle: Option<Box<dyn Fn(bool) -> Message + 'a>> = if row.blocked.is_none() && !locked {
        let key = row.key;
        Some(Box::new(move |on| {
            ba(BackAnnotateMsg::RowToggled { key, on })
        }))
    } else {
        None
    };
    let status_tip = caption.as_ref().map(|(words, _)| words.clone());
    change_row(
        ChangeRow {
            on: row.on,
            on_toggle,
            pill: None,
            object: None,
            detail: detail_text(
                change_text(i18n, &row.change),
                row.blocked.is_some(),
                tokens,
            ),
            caption,
            status,
            status_tip,
            sentence: None,
            focused: state.focused == Some(row.key),
            on_press: None,
            dimmed: row.blocked.is_some(),
        },
        tokens,
    )
}

/// A row's status and caption: blocked (red), listed only (grey), a
/// flagged rename (amber), the green tick once Validate passed with it
/// ticked, else quiet.
pub(super) fn row_status(
    state: &BackAnnotateState,
    row: &BackRow,
    flag: Option<&DuplicateRename>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> (RowStatus, Option<(Localized, iced::Color)>) {
    let info = matches!(row.blocked, Some(BackBlock::FootprintFromBoard));
    match (&row.blocked, flag) {
        (Some(why), _) if info => (
            RowStatus::Info,
            Some((block_text(i18n, why), tokens::muted_text(tokens))),
        ),
        (Some(why), _) => (
            RowStatus::Blocked,
            Some((block_text(i18n, why), tokens::danger(tokens))),
        ),
        (None, Some(flag)) => (
            RowStatus::Warn,
            Some((duplicate_text(state, flag, i18n), ti(tokens.warning))),
        ),
        (None, None) if matches!(state.validation(), Some(Ok(()))) && row.applies() => {
            (RowStatus::Applies, None)
        }
        (None, None) => (RowStatus::Quiet, None),
    }
}

/// The caption of a flagged rename: where its new designator already is.
fn duplicate_text(state: &BackAnnotateState, flag: &DuplicateRename, i18n: &I18n) -> Localized {
    if flag.contested {
        return t!(
            i18n,
            ids::BACK_ANNOTATE_DUPLICATE_CONTESTED,
            "designator" => flag.designator.as_str()
        );
    }
    match state.part_sheet_name(&flag.holder) {
        Some(sheet) => t!(
            i18n,
            ids::BACK_ANNOTATE_DUPLICATE_ON_SHEET,
            "designator" => flag.designator.as_str(),
            "sheet" => sheet
        ),
        None => t!(
            i18n,
            ids::BACK_ANNOTATE_DUPLICATE_EXISTS,
            "designator" => flag.designator.as_str()
        ),
    }
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

/// Why a row cannot be ticked. Compare Board with Schematic says it too.
pub(super) fn block_text(i18n: &I18n, why: &BackBlock) -> Localized {
    match why {
        BackBlock::NeedsAnnotation => t!(i18n, ids::BACK_ANNOTATE_BLOCK_NEEDS_ANNOTATION),
        BackBlock::DuplicateDesignator(designator) => t!(
            i18n,
            ids::BACK_ANNOTATE_BLOCK_DUPLICATE,
            "designator" => designator.as_str()
        ),
        BackBlock::BoardNotAnnotated => t!(i18n, ids::BACK_ANNOTATE_BLOCK_BOARD_NOT_ANNOTATED),
        BackBlock::SharedSheet => t!(i18n, ids::BACK_ANNOTATE_BLOCK_SHARED_SHEET),
        BackBlock::ChannelDesignator => t!(i18n, ids::BACK_ANNOTATE_BLOCK_CHANNEL),
        BackBlock::FootprintFromBoard => t!(i18n, ids::BACK_ANNOTATE_BLOCK_FOOTPRINT),
    }
}

/// One note under the rows. Compare Board with Schematic shows it too.
pub(super) fn note_text(i18n: &I18n, note: &BackNote) -> Localized {
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

/// The Notes group, last in the list; its mark turns amber while a note
/// says a designator would be on several parts.
fn notes_view<'a>(
    state: &BackAnnotateState,
    list: &BackChangeList,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let warns = list
        .notes
        .iter()
        .any(|n| matches!(n, BackNote::SharedDesignator { .. }));
    let mark = if warns {
        (StatusGlyph::Warn, ti(tokens.warning))
    } else {
        (StatusGlyph::Info, tokens::resolve(tokens).info)
    };
    let folded = state.notes_folded;
    let header = group_header(
        GroupHeader {
            title: t!(i18n, ids::BACK_ANNOTATE_NOTES_HEADING),
            ticks: None,
            on_tick: None,
            folded,
            on_fold: ba(BackAnnotateMsg::NotesFolded(!folded)),
            count: t!(i18n, ids::CHANGE_ROW_NOTES_COUNT, "count" => list.notes.len()),
            split: None,
            mark: Some(mark),
        },
        i18n,
        tokens,
    );
    let mut col = Column::new().push(header);
    if !folded {
        for note in &list.notes {
            col = col.push(
                container(
                    text(note_text(i18n, note))
                        .size(TS.body)
                        .color(tokens::muted_text(tokens)),
                )
                .padding([SP.s, SP.m])
                .width(Length::Fill)
                .boxed(),
            );
        }
    }
    col.boxed()
}
