//! The change table of the sync dialog (mockup row 79): one row per change
//! with its Change pill, the entry and its I/O type. On Symbol to Sheet each
//! removal has its own tick (#2831).

use iced::widget::{Column, Row, Space, column, container};
use iced::{Alignment, Border, Element, Length, Theme, Widget as _};
use signex_engine::{SheetEntryChange, SheetSyncEntry};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{checkbox, text};

use super::{SP, TS};
use crate::app::Message;
use crate::app::sheet_sync::{SheetSyncMsg, SheetSyncState};
use crate::panels::form_look::checkbox_style;
use crate::styles::{HAIRLINE, PillTone, Radii, pill, pill_colors, ti, tokens};

/// Widths of the Change and I/O type columns, in body ems (84 and 96 px).
const CHANGE_EM: f32 = 7.0;
const IO_EM: f32 = 8.0;
/// Width of the Remove column, in body ems (60 px).
const TICK_EM: f32 = 5.0;

/// The rows that change something, Added first, then Removed, then Renamed.
pub(super) fn changed_rows(entries: &[SheetSyncEntry]) -> Vec<&SheetSyncEntry> {
    let rank = |entry: &SheetSyncEntry| match entry.change {
        SheetEntryChange::Added => 0,
        SheetEntryChange::Removed => 1,
        SheetEntryChange::Renamed { .. } => 2,
        SheetEntryChange::Kept => 3,
    };
    let mut rows: Vec<&SheetSyncEntry> = entries
        .iter()
        .filter(|entry| entry.change != SheetEntryChange::Kept)
        .collect();
    rows.sort_by_key(|entry| rank(*entry));
    rows
}

/// The pill words and tone of a change.
pub(super) fn change_pill(change: &SheetEntryChange) -> (MessageId, PillTone) {
    match change {
        SheetEntryChange::Added => (ids::SHEET_SYNC_ADDED, PillTone::Ok),
        SheetEntryChange::Removed => (ids::SHEET_SYNC_REMOVED, PillTone::Err),
        SheetEntryChange::Renamed { .. } | SheetEntryChange::Kept => {
            (ids::SHEET_SYNC_RENAMED, PillTone::Info)
        }
    }
}

/// The catalog name of a normalised I/O type.
pub(super) fn io_name(direction: &str) -> MessageId {
    match direction {
        "input" => ids::SHEET_SYNC_IO_INPUT,
        "output" => ids::SHEET_SYNC_IO_OUTPUT,
        "tri_state" => ids::SHEET_SYNC_IO_TRI_STATE,
        "passive" => ids::SHEET_SYNC_IO_PASSIVE,
        _ => ids::SHEET_SYNC_IO_BIDIRECTIONAL,
    }
}

/// A row of the columns; `tick` is the Remove cell, when the table has one.
fn line<'a>(
    tick: Option<Element<'a, Message>>,
    change: Element<'a, Message>,
    entry: Element<'a, Message>,
    io: Element<'a, Message>,
) -> Element<'a, Message> {
    let mut cells: Row<Element<'a, Message>> = Row::new().spacing(SP.s).align_y(Alignment::Center);
    if let Some(tick) = tick {
        cells = cells.push(
            container(tick)
                .width(Length::Fixed(TS.body * TICK_EM))
                .boxed(),
        );
    }
    cells
        .push(
            container(change)
                .width(Length::Fixed(TS.body * CHANGE_EM))
                .boxed(),
        )
        .push(container(entry).width(Length::Fill).boxed())
        .push(container(io).width(Length::Fixed(TS.body * IO_EM)).boxed())
        .boxed()
}

/// The Remove cell: a tick on a removal, empty on any other change. Drawn
/// disabled while Apply waits on the file check.
fn removal_tick<'a>(entry: &SheetSyncEntry, state: &SheetSyncState) -> Element<'a, Message> {
    if entry.change != SheetEntryChange::Removed {
        return container(Space::new()).boxed();
    }
    let mut tick = checkbox(!state.is_left_out(entry))
        .size(TS.body)
        .style(checkbox_style);
    if !state.checking {
        let port = entry.name.clone();
        tick = tick.on_toggle(move |ticked| {
            Message::from(SheetSyncMsg::RemovalTicked {
                port: port.clone(),
                ticked,
            })
        });
    }
    container(tick).boxed()
}

fn header_cell<'a>(id: MessageId, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(t!(i18n, id))
        .size(TS.caption)
        .color(tokens::resolve(tokens).ink3)
        .boxed()
}

/// The entry cell: the name, or old → new with the new name in the accent.
fn entry_cell<'a>(
    entry: &SheetSyncEntry,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mono = super::super::graphite::mono();
    let ink = ti(tokens.text);
    match &entry.change {
        SheetEntryChange::Renamed { from } => {
            let words: Localized = t!(
                i18n,
                ids::SHEET_SYNC_RENAME_ROW,
                "from" => from.as_str(),
                "to" => entry.name.as_str()
            );
            text(words).size(TS.body).font(mono).color(ink).boxed()
        }
        _ => text(UserText::new(entry.name.clone()))
            .size(TS.body)
            .font(mono)
            .color(ink)
            .boxed(),
    }
}

fn change_row<'a>(
    entry: &SheetSyncEntry,
    state: &SheetSyncState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (words, tone) = if state.is_left_out(entry) {
        (ids::SHEET_SYNC_LEFT_OUT, PillTone::Neutral)
    } else {
        change_pill(&entry.change)
    };
    let tick = state
        .shows_removal_ticks()
        .then(|| removal_tick(entry, state));
    let ink = pill_colors(tokens, tone).1;
    let badge = container(text(t!(i18n, words)).size(TS.caption).color(ink))
        .padding([0.0, SP.m * 0.75])
        .style(pill(tokens, tone))
        .boxed();
    let io = text(t!(i18n, io_name(entry.direction)))
        .size(TS.body)
        .color(tokens::muted_text(tokens))
        .boxed();
    container(line(tick, badge, entry_cell(entry, i18n, tokens), io))
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// The table of `rows`: a header strip, then one row per change.
pub(super) fn view<'a>(
    rows: &[&SheetSyncEntry],
    state: &SheetSyncState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let (strip, edge) = (r.sunken, r.line_soft);
    let remove_head = state
        .shows_removal_ticks()
        .then(|| header_cell(ids::SHEET_SYNC_COLUMN_REMOVE, i18n, tokens));
    let header = container(line(
        remove_head,
        header_cell(ids::SHEET_SYNC_COLUMN_CHANGE, i18n, tokens),
        header_cell(ids::SHEET_SYNC_COLUMN_ENTRY, i18n, tokens),
        header_cell(ids::SHEET_SYNC_COLUMN_IO, i18n, tokens),
    ))
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(strip.into()),
        ..container::Style::default()
    });
    let mut list: Column<Element<'a, Message>> = column![header.boxed()];
    for entry in rows {
        list = list.push(change_row(entry, state, i18n, tokens));
    }
    let fill = r.field;
    container(list)
        .width(Length::Fill)
        .padding(HAIRLINE)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(fill.into()),
            border: Border {
                width: HAIRLINE,
                color: edge,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, change: SheetEntryChange) -> SheetSyncEntry {
        SheetSyncEntry {
            name: name.to_string(),
            direction: "input",
            change,
        }
    }

    /// Mockup row 79: the table lists what changes, in pill order.
    #[test]
    fn the_table_lists_only_changes_in_order() {
        let entries = [
            entry(
                "B",
                SheetEntryChange::Renamed {
                    from: "A".to_string(),
                },
            ),
            entry("KEEP", SheetEntryChange::Kept),
            entry("GONE", SheetEntryChange::Removed),
            entry("NEW", SheetEntryChange::Added),
        ];
        let names: Vec<&str> = changed_rows(&entries)
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(names, ["NEW", "GONE", "B"]);
    }

    #[test]
    fn every_io_type_has_its_own_name() {
        let en = I18n::en_us_ref();
        for (direction, words) in [
            ("input", "Input"),
            ("output", "Output"),
            ("bidirectional", "Bidirectional"),
            ("tri_state", "Tri-state"),
            ("passive", "Passive"),
        ] {
            assert_eq!(en.message_plain(io_name(direction), &[]), words);
        }
    }
}
