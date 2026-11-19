//! The Selection popover (m164): what is selected on the board, a part by
//! its designator, value and footprint and anything else by its kind, each
//! with a Zoom; then Properties, Select in schematic (greyed for a board in
//! no project) and Clear. The card sits right-aligned to its segment, its
//! buttons wrapping on a narrow window.

use iced::Element;
use iced::widget::{Widget, row};
use pcb_geom::PickKind;
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::super::graphite::TS;
use super::kit::{
    Glyph, PX, ROW_GLYPH, caption, card, card_width, footer, glyph, header, list_row, mono_weight,
    scrolling, small_button, zoom_link,
};
use super::{BoardPopoverEvent, BoardPopoverKind, SelectedRow, SelectedWhat, SelectionList};
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// The name of an object of `kind`, in the singular.
pub(super) fn kind_name(kind: PickKind) -> MessageId {
    match kind {
        PickKind::Components => ids::BOARD_STATUS_KIND_COMPONENT,
        PickKind::Bodies3d => ids::BOARD_STATUS_KIND_BODY,
        PickKind::Keepouts => ids::BOARD_STATUS_KIND_KEEPOUT,
        PickKind::Tracks => ids::BOARD_STATUS_KIND_TRACK,
        PickKind::Arcs => ids::BOARD_STATUS_KIND_ARC,
        PickKind::Pads => ids::BOARD_STATUS_KIND_PAD,
        PickKind::Vias => ids::BOARD_STATUS_KIND_VIA,
        PickKind::Regions => ids::BOARD_STATUS_KIND_REGION,
        PickKind::Polygons => ids::BOARD_STATUS_KIND_POLYGON,
        PickKind::Fills => ids::BOARD_STATUS_KIND_FILL,
        PickKind::Texts => ids::BOARD_STATUS_KIND_TEXT,
        PickKind::Rooms => ids::BOARD_STATUS_KIND_ROOM,
        PickKind::Other => ids::BOARD_STATUS_KIND_OTHER,
    }
}

fn selected_row<'a>(
    selected: &SelectedRow,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    let ink = ti(tokens.text);
    let (lead, body) = match &selected.what {
        SelectedWhat::Part {
            designator,
            value,
            footprint,
        } => {
            let name = text(UserText::new(designator.clone()))
                .size(TS.body)
                .font(mono_weight(iced::font::Weight::Semibold))
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::None);
            let about = t!(
                i18n,
                ids::BOARD_STATUS_PART_CAPTION,
                "value" => value.clone(),
                "footprint" => footprint.clone()
            );
            let body = row![name, caption(about, tokens)]
                .spacing(4.0 * PX)
                .align_y(iced::Alignment::Center);
            (Glyph::Part, body.boxed())
        }
        SelectedWhat::Kind(kind) => {
            let name = text(t!(i18n, kind_name(*kind)))
                .size(TS.body)
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::None);
            (Glyph::Selection, name.boxed())
        }
    };
    list_row(
        Some(glyph(lead, ROW_GLYPH, resolve(tokens).ink3)),
        body,
        Some(zoom_link(
            BoardPopoverEvent::SelectedItemZoomed(selected.item),
            tokens,
            i18n,
        )),
        None,
        tokens,
    )
}

pub(super) fn view<'a>(
    list: &SelectionList,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    let count = t!(i18n, ids::BOARD_STATUS_SELECTION_OBJECTS, "count" => list.count);
    let mut parts = vec![header(
        t!(i18n, ids::BOARD_STATUS_SELECTION_TITLE),
        Some(caption(count, tokens)),
        tokens,
    )];
    let rows = list
        .rows
        .iter()
        .map(|row_data| selected_row(row_data, tokens, i18n))
        .collect();
    parts.push(scrolling(rows));
    let more = list.count.saturating_sub(list.rows.len());
    if more > 0 {
        parts.push(list_row(
            None,
            caption(
                t!(i18n, ids::BOARD_STATUS_SELECTION_MORE, "count" => more),
                tokens,
            ),
            None,
            None,
            tokens,
        ));
    }
    let any = list.count > 0;
    parts.push(footer(
        vec![
            small_button(
                t!(i18n, ids::BOARD_STATUS_PROPERTIES),
                None,
                false,
                Some(BoardPopoverEvent::PropertiesOpened),
                tokens,
            ),
            small_button(
                t!(i18n, ids::BOARD_STATUS_SELECT_IN_SCHEMATIC),
                None,
                false,
                (any && list.linked).then_some(BoardPopoverEvent::SchematicSelectRequested),
                tokens,
            ),
            small_button(
                t!(i18n, ids::BOARD_STATUS_CLEAR),
                None,
                false,
                any.then_some(BoardPopoverEvent::SelectionCleared),
                tokens,
            ),
        ],
        tokens,
    ));
    card(card_width(BoardPopoverKind::Selection), parts, tokens)
}
