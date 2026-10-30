//! The net hover card: what the cursor rests on, as the net's name, class and
//! pins with their sheets (#1270). The card is built in `update`; this only
//! draws it.

use super::*;
use crate::app::state::net_nav::HoverCard;

/// Card width and the room reserved when clamping it inside the window.
const CARD_W: f32 = 240.0;
const ROW_H: f32 = 16.0;
const CARD_PAD_H: f32 = 40.0;

impl Signex {
    pub(super) fn view_net_hover_card(&self, card: &HoverCard) -> Element<'_, Message> {
        use iced::widget::{column, container, row, text};
        use iced::{Background, Border};

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let muted_c = crate::styles::ti(tokens.text_secondary);
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let border_c = crate::styles::ti(tokens.border);

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(card.pins.len() + 3);
        rows.push(text(card.name.clone()).size(TS.label).color(text_c).into());
        if let Some(class) = &card.class {
            rows.push(
                text(format!("Class: {class}"))
                    .size(TS.caption)
                    .color(muted_c)
                    .into(),
            );
        }
        for pin in &card.pins {
            rows.push(
                row![
                    text(pin.pin.clone()).size(TS.caption).color(text_c),
                    text(pin.sheet.clone()).size(TS.caption).color(muted_c),
                ]
                .spacing(SP.s)
                .into(),
            );
        }
        if card.hidden > 0 {
            rows.push(
                text(format!("+{} more", card.hidden))
                    .size(TS.caption)
                    .color(muted_c)
                    .into(),
            );
        }
        let height = CARD_PAD_H + ROW_H * rows.len() as f32;

        let body = container(column(rows).spacing(2))
            .padding(iced::Padding {
                top: 8.0,
                right: 14.0,
                bottom: 8.0,
                left: 10.0,
            })
            .width(Length::Fixed(CARD_W))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                border: Border {
                    color: border_c,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            });

        let (x, y) = canvas_region::place_card(
            (card.screen.0 as f32, card.screen.1 as f32),
            (CARD_W, height),
            self.canvas_bounds(),
        );
        super::super::view::translate::Translate::new(body, (x, y)).into()
    }
}
