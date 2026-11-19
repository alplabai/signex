//! The Active layer popover (m164): the layers the next-layer key steps
//! through, each with its swatch and name, the active one checked; a row
//! makes that layer active. The footer opens the Layers panel.

use iced::Element;
use iced::widget::Widget;
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::super::graphite::TS;
use super::counters::layer_swatch;
use super::kit::{
    Glyph, caption, card, card_width, check, cycles, footer, header, list_row, small_button,
};
use super::{BoardPopoverEvent, BoardPopoverKind, LayerList};
use crate::styles::ti;

pub(super) fn view<'a>(
    list: &LayerList,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    let keys = cycles(&list.next_key, &list.prev_key, i18n).map(|keys| caption(keys, tokens));
    let mut parts = vec![header(
        t!(i18n, ids::BOARD_STATUS_LAYER_TITLE),
        keys,
        tokens,
    )];
    parts.extend(list.rows.iter().map(|row| {
        let name = text(row.name.clone())
            .size(TS.body)
            .color(ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None);
        list_row(
            Some(layer_swatch(row.colour, tokens)),
            name.boxed(),
            Some(check(row.active, tokens)),
            Some(BoardPopoverEvent::LayerPicked(row.layer)),
            tokens,
        )
    }));
    parts.push(footer(
        vec![small_button(
            t!(i18n, ids::BOARD_STATUS_LAYERS_PANEL),
            Some(Glyph::Layers),
            false,
            Some(BoardPopoverEvent::LayersPanelOpened),
            tokens,
        )],
        tokens,
    ));
    card(card_width(BoardPopoverKind::Layer), parts, tokens)
}
