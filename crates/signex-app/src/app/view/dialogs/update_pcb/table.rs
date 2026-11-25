//! The Update PCB list (#2371, #2533): the groups Components, Nets, Pad
//! nets and Notes, each with its header, then its dense rows unless folded.
//! The header and row builders are shared with Back Annotate
//! (`change_rows.rs`).

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use signex_engine::pcb::eco::{EcoChangeList, EcoRow};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::change_rows::{
    ChangeRow, GroupHeader, RowStatus, StatusGlyph, change_row, detail_text, group_header,
};
use super::{SP, TS, texts, up};
use crate::app::update_pcb::table::{Summary, group_ticks, object, rows_of};
use crate::app::update_pcb::{RowAction, TableGroup, UpdatePcbState};
use crate::app::{Message, UpdatePcbMsg};
use crate::styles::{PillTone, tokens};

/// Every group with a shown row: its header, then its rows unless folded;
/// the Notes group last.
pub(super) fn body<'a>(
    state: &UpdatePcbState,
    list: &EcoChangeList,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut col = Column::new();
    let mut any = false;
    for group in TableGroup::ALL {
        let shown: Vec<&EcoRow> = rows_of(list, group)
            .filter(|r| state.table.shows(r))
            .collect();
        if shown.is_empty() {
            continue;
        }
        any = true;
        let folded = state.table.is_folded(list, group);
        col = col.push(heading(list, group, folded, i18n, tokens));
        if folded {
            continue;
        }
        for r in shown {
            col = col.push(row_view(state, list, r, i18n, tokens));
        }
    }
    if !any && !list.rows.is_empty() {
        col = col.push(
            container(
                text(t!(i18n, ids::UPDATE_PCB_FILTERED_EMPTY))
                    .size(TS.body)
                    .color(tokens::muted_text(tokens)),
            )
            .padding([SP.m, SP.m])
            .boxed(),
        );
    }
    if !list.notes.is_empty() {
        col = col.push(notes(state, list, i18n, tokens));
    }
    col.width(Length::Fill).boxed()
}

/// A group's header: fold, the tri-state tick (on when every open row is
/// ticked, mixed when some are; a click ticks them all unless all are),
/// `n of m` and the split of its rows.
fn heading<'a>(
    list: &EcoChangeList,
    group: TableGroup,
    folded: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ticks = group_ticks(list, group);
    let rows: Vec<&EcoRow> = rows_of(list, group).collect();
    let target = !ticks.all();
    group_header(
        GroupHeader {
            title: t!(i18n, texts::group_id(group)),
            ticks: Some(ticks),
            on_tick: (ticks.open > 0).then(|| up(UpdatePcbMsg::GroupToggled { group, on: target })),
            folded,
            on_fold: up(UpdatePcbMsg::GroupFolded {
                group,
                folded: !folded,
            }),
            count: t!(
                i18n,
                ids::UPDATE_PCB_GROUP_COUNT,
                "on" => ticks.on,
                "open" => rows.len()
            ),
            split: Some(Summary::of_rows(rows)),
            mark: None,
        },
        i18n,
        tokens,
    )
}

/// The Notes group: a list group like the others, folded on open.
fn notes<'a>(
    state: &UpdatePcbState,
    list: &EcoChangeList,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let folded = !state.table.notes_open;
    let header = group_header(
        GroupHeader {
            title: t!(i18n, ids::UPDATE_PCB_NOTES_HEADING),
            ticks: None,
            on_tick: None,
            folded,
            on_fold: up(UpdatePcbMsg::NotesFolded(!folded)),
            count: t!(i18n, ids::CHANGE_ROW_NOTES_COUNT, "count" => list.notes.len()),
            split: None,
            mark: Some((StatusGlyph::Info, tokens::resolve(tokens).info)),
        },
        i18n,
        tokens,
    );
    let mut col = Column::new().push(header);
    if !folded {
        for note in &list.notes {
            col = col.push(
                container(
                    text(texts::note(i18n, note))
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

/// One dense row. A blocked row stays in its group unticked, with the red
/// status; a clicked row takes the focus Space ticks.
fn row_view<'a>(
    state: &UpdatePcbState,
    list: &EcoChangeList,
    r: &EcoRow,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let problem = r.blocked.is_some();
    let action = RowAction::of(&r.change);
    let (status, status_tip) = status(state, list, r, i18n);
    let on_toggle: Option<Box<dyn Fn(bool) -> Message + 'a>> = if problem {
        None
    } else {
        let key = r.key.clone();
        Some(Box::new(move |on| {
            up(UpdatePcbMsg::RowToggled {
                key: key.clone(),
                on,
            })
        }))
    };
    let detail = texts::detail(i18n, &r.change);
    change_row(
        ChangeRow {
            on: r.on,
            on_toggle,
            pill: Some((t!(i18n, texts::action(action)), action_tone(action))),
            object: Some(object(&r.change)),
            detail: detail_text(UserText::new(detail), problem, tokens),
            caption: None,
            status,
            status_tip: Some(status_tip),
            sentence: Some(texts::change(i18n, &r.change)),
            focused: state.table.focused.as_ref() == Some(&r.key),
            on_press: Some(up(UpdatePcbMsg::RowFocused(r.key.clone()))),
            dimmed: problem,
        },
        tokens,
    )
}

/// The status icon and its words: the problem in full, Skipped when a row
/// it needs is unticked, ✓ after a passing Validate, else quiet.
fn status(
    state: &UpdatePcbState,
    list: &EcoChangeList,
    r: &EcoRow,
    i18n: &I18n,
) -> (RowStatus, Localized) {
    if let Some(why) = &r.blocked {
        return (RowStatus::Blocked, texts::blocked(i18n, why));
    }
    if r.on && !list.applies(r) {
        return (RowStatus::Skipped, t!(i18n, ids::UPDATE_PCB_ROW_SKIPPED));
    }
    if matches!(state.validation, Some(Ok(()))) && list.applies(r) {
        return (
            RowStatus::Applies,
            t!(i18n, ids::UPDATE_PCB_STATUS_OK_TOOLTIP),
        );
    }
    let words = if r.on {
        ids::CHANGE_ROW_STATUS_TICKED
    } else {
        ids::CHANGE_ROW_STATUS_UNTICKED
    };
    (RowStatus::Quiet, t!(i18n, words))
}

/// The pill tone of an action.
fn action_tone(action: RowAction) -> PillTone {
    match action {
        RowAction::Add => PillTone::Ok,
        RowAction::Change => PillTone::Info,
        RowAction::Remove => PillTone::Err,
    }
}
