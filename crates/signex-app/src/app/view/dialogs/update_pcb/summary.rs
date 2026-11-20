//! The Update PCB summary (#2371, #2533): four count tiles (Add, Change,
//! Remove, Problems) that filter the list, the filter field, and under it
//! the hint or, while a filter is on, Clear filter.

use iced::widget::{Column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_engine::pcb::eco::EcoChangeList;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::stat_tiles::{Density, StatTile, Tone, stat_tiles};
use signex_widgets::{text, text_input};

use super::{SP, TS, link, up};
use crate::app::update_pcb::table::{Summary, TableState};
use crate::app::update_pcb::{RowAction, RowFilter};
use crate::app::{Message, UpdatePcbMsg};
use crate::styles::tokens;

/// The tiles take this share of the width and the filter column the rest:
/// four tiles beside a filter about 1.4 tiles wide.
const TILES_PART: u16 = 20;
const FILTER_PART: u16 = 7;

/// The tile row with the filter field beside it.
pub(super) fn bar<'a>(
    list: &EcoChangeList,
    table: &'a TableState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let summary = Summary::of(list);
    let tile = |filter: RowFilter, count: usize, label: MessageId, tone: Tone| {
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        StatTile::count(count, t!(i18n, label))
            .tone(tone)
            .on(table.filter == Some(filter))
            .on_press(up(UpdatePcbMsg::FilterChipClicked(filter)))
    };
    let tiles = stat_tiles(
        vec![
            tile(
                RowFilter::Action(RowAction::Add),
                summary.add,
                ids::UPDATE_PCB_ACTION_ADD,
                Tone::Ok,
            ),
            tile(
                RowFilter::Action(RowAction::Change),
                summary.change,
                ids::UPDATE_PCB_ACTION_CHANGE,
                Tone::Info,
            ),
            tile(
                RowFilter::Action(RowAction::Remove),
                summary.remove,
                ids::UPDATE_PCB_ACTION_REMOVE,
                Tone::Neutral,
            ),
            tile(
                RowFilter::Problems,
                summary.problems,
                ids::UPDATE_PCB_TILE_PROBLEMS,
                Tone::Err,
            ),
        ],
        Density::Regular,
        tokens,
        TS.body,
        i18n,
    );
    let field = text_input(t!(i18n, ids::UPDATE_PCB_FILTER_PLACEHOLDER), &table.query)
        .style(crate::panels::form_look::text_input_style)
        .on_input(|query| up(UpdatePcbMsg::FilterQueryChanged(query)))
        .size(TS.body)
        .padding([SP.xs, SP.m])
        .width(Length::Fill);
    let under: Element<'a, Message> = if table.is_filtered() {
        link(
            t!(i18n, ids::UPDATE_PCB_CLEAR_FILTER),
            up(UpdatePcbMsg::FilterCleared),
        )
    } else {
        text(t!(i18n, ids::UPDATE_PCB_TILE_HINT))
            .size(TS.caption)
            .color(tokens::muted_text(tokens))
            .boxed()
    };
    let filter = Column::new()
        .spacing(SP.xs)
        .push(field.boxed())
        .push(under)
        .width(Length::FillPortion(FILTER_PART));
    row![
        container(tiles).width(Length::FillPortion(TILES_PART)),
        filter
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}
