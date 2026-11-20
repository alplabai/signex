//! The DRC popover (mockup row 164): when the last batch run landed and how many edits
//! the board had since, its violations worded as the DRC panel words them
//! with a Zoom each, then Run DRC and Open Messages.

use iced::Element;
use iced::widget::{Widget, row};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_types::violation::Severity;
use signex_widgets::text;

use super::super::graphite::TS;
use super::kit::{
    Glyph, PX, ROW_GLYPH, caption, card, card_width, detail, footer, glyph, header, list_row,
    scrolling, small_button, two_line_row, zoom_link,
};
use super::{BoardPopoverEvent, BoardPopoverKind, DrcList};
use crate::panels::DrcRow;
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// The header's caption: the last run's time and the edits since.
pub(super) fn last_run_text(list: &DrcList, i18n: &I18n) -> Localized {
    match &list.last_run {
        Some((time, edits)) => t!(
            i18n,
            ids::BOARD_STATUS_DRC_LAST_RUN,
            "time" => time.clone(),
            "edits" => *edits
        ),
        None => t!(i18n, ids::BOARD_STATUS_DRC_NEVER),
    }
}

/// A violation's second line: its objects, where it is and its layer.
fn place_text(i18n: &I18n, row: &DrcRow, place: Localized) -> Localized {
    let objects = t!(i18n, ids::DRC_PANEL_OBJECTS, "count" => row.objects).to_string();
    match row.layer {
        Some(layer) => t!(
            i18n,
            ids::BOARD_STATUS_DRC_DETAIL_LAYER,
            "objects" => objects,
            "place" => place.to_string(),
            "layer" => crate::pcb_canvas::layer_style::copper_name(i18n, layer).to_string()
        ),
        None => t!(
            i18n,
            ids::BOARD_STATUS_DRC_DETAIL,
            "objects" => objects,
            "place" => place.to_string()
        ),
    }
}

/// The glyph and ink of a severity: an error in the error ink, a warning in
/// the warning ink, anything else muted.
fn severity_glyph<'a>(
    severity: Option<Severity>,
    tokens: &ThemeTokens,
) -> Element<'a, BoardPopoverEvent> {
    match severity {
        Some(Severity::Error) => glyph(Glyph::Error, ROW_GLYPH, ti(tokens.error)),
        Some(Severity::Warning) => glyph(Glyph::Warning, ROW_GLYPH, ti(tokens.warning)),
        _ => glyph(Glyph::Warning, ROW_GLYPH, resolve(tokens).ink3),
    }
}

fn violation_row<'a>(
    row_data: &DrcRow,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    let (kind, broke, place) = crate::panels::drc_row_text(i18n, row_data);
    let first = row![
        text(kind)
            .size(TS.body)
            .color(ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None),
        detail(broke, tokens),
    ]
    .spacing(4.0 * PX)
    .align_y(iced::Alignment::Center);
    two_line_row(
        severity_glyph(row_data.severity, tokens),
        first.boxed(),
        detail(place_text(i18n, row_data, place), tokens),
        zoom_link(
            BoardPopoverEvent::ViolationZoomed(row_data.key.clone()),
            tokens,
            i18n,
        ),
    )
}

pub(super) fn view<'a>(
    list: &DrcList,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    let mut parts = vec![header(
        t!(i18n, ids::BOARD_STATUS_DRC_TITLE),
        Some(caption(last_run_text(list, i18n), tokens)),
        tokens,
    )];
    let note = if list.running {
        Some(t!(i18n, ids::BOARD_STATUS_DRC_RUNNING))
    } else if list.last_run.is_some() && list.rows.is_empty() {
        Some(t!(i18n, ids::BOARD_STATUS_DRC_CLEAN))
    } else {
        None
    };
    if let Some(note) = note {
        parts.push(list_row(None, caption(note, tokens), None, None, tokens));
    }
    let rows = list
        .rows
        .iter()
        .map(|row_data| violation_row(row_data, tokens, i18n))
        .collect();
    parts.push(scrolling(rows));
    if list.more > 0 {
        parts.push(list_row(
            None,
            caption(
                t!(i18n, ids::BOARD_STATUS_DRC_MORE, "count" => list.more),
                tokens,
            ),
            None,
            None,
            tokens,
        ));
    }
    parts.push(footer(
        vec![
            small_button(
                t!(i18n, ids::BOARD_STATUS_RUN_DRC),
                None,
                true,
                (!list.running).then_some(BoardPopoverEvent::DrcRunRequested),
                tokens,
            ),
            small_button(
                t!(i18n, ids::BOARD_STATUS_OPEN_MESSAGES),
                None,
                false,
                Some(BoardPopoverEvent::MessagesOpened),
                tokens,
            ),
        ],
        tokens,
    ));
    card(card_width(BoardPopoverKind::Drc), parts, tokens)
}
