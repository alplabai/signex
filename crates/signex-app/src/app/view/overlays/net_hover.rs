//! The net hover card: what the cursor rests on, as the net's name, class and
//! pins with their sheets (#1270). The card is built in `update`; this only
//! draws it.

use super::*;
use crate::app::state::net_nav::HoverCard;

/// Widest the card grows; below that it is as wide as its text (#2092).
const CARD_MAX_W: f32 = 320.0;
/// Average glyph advance per em the card is sized by: a slight overestimate
/// of the UI face, so the text never clips.
const GLYPH_ADVANCE_EM: f32 = 0.6;
/// Left plus right padding of the card body.
const CARD_PAD_W: f32 = 24.0;
const ROW_H: f32 = 16.0;
const CARD_PAD_H: f32 = 40.0;

impl Signex {
    pub(super) fn view_net_hover_card(
        &self,
        card: &HoverCard,
        window_id: iced::window::Id,
    ) -> Element<'_, Message> {
        use iced::widget::{Widget as _, column, container, row, text};
        use iced::{Background, Border};

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let muted_c = crate::styles::ti(tokens.text_secondary);
        let panel_bg = crate::styles::ti(tokens.panel_bg);
        let border_c = crate::styles::ti(tokens.border);

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(card.pins.len() + 3);
        rows.push(text(card.name.clone()).size(TS.label).color(text_c).boxed());
        if let Some(class) = &card.class {
            rows.push(
                text(format!("Class: {class}"))
                    .size(TS.caption)
                    .color(muted_c)
                    .boxed(),
            );
        }
        for pin in &card.pins {
            rows.push(
                row![
                    text(pin.pin.clone()).size(TS.caption).color(text_c),
                    text(pin.sheet.clone()).size(TS.caption).color(muted_c),
                ]
                .spacing(SP.s)
                .boxed(),
            );
        }
        if card.hidden > 0 {
            rows.push(
                text(format!("+{} more", card.hidden))
                    .size(TS.caption)
                    .color(muted_c)
                    .boxed(),
            );
        }
        let height = CARD_PAD_H + ROW_H * rows.len() as f32;
        let width = card_width_estimate(card);

        let body = container(column(rows).spacing(2).boxed())
            .padding(iced::Padding {
                top: 8.0,
                right: 14.0,
                bottom: 8.0,
                left: 10.0,
            })
            .width(Length::Fixed(width))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                border: Border {
                    color: border_c,
                    width: 1.0,
                    radius: 4.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });

        let (x, y) = canvas_region::place_card(
            (card.screen.0 as f32, card.screen.1 as f32),
            (width, height),
            self.card_bounds(window_id),
        );
        super::super::translate::Translate::new(body.boxed(), (x, y)).boxed()
    }
}

/// The room the card needs, for keeping it inside the window: the widest of
/// its lines at [`GLYPH_ADVANCE_EM`], plus the padding, at most
/// [`CARD_MAX_W`].
fn card_width_estimate(card: &HoverCard) -> f32 {
    let line = |chars: usize, size: f32| chars as f32 * size * GLYPH_ADVANCE_EM;
    let mut widest = line(card.name.chars().count(), TS.label);
    if let Some(class) = &card.class {
        widest = widest.max(line("Class: ".len() + class.chars().count(), TS.caption));
    }
    for pin in &card.pins {
        let chars = pin.pin.chars().count() + pin.sheet.chars().count();
        widest = widest.max(line(chars, TS.caption) + SP.s);
    }
    (widest + CARD_PAD_W + 2.0).min(CARD_MAX_W)
}
