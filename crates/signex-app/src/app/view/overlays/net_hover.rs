//! The net hover card (Graphite mockup row 10): the net's name with a pin-count pill,
//! its class, and its pins with their sheets (#1270). The card is built in
//! `update`; this only draws it.

use super::symbol_hover::{
    HOVER_CARD_MAX_W, HOVER_CARD_MIN_W, HOVER_CARD_PAD_H, HOVER_CARD_PAD_W, HOVER_GLYPH,
    HOVER_TITLE_GAP, HOVER_TITLE_H, hover_card, hover_card_title,
};
use super::*;
use crate::app::state::net_nav::HoverCard;

/// Height budget of one caption line, for placing the card.
const ROW_H: f32 = 16.0;
/// Horizontal padding inside the pin-count pill (6 px at the default base).
const PILL_PAD_X: f32 = crate::styles::chrome_px(6.0 * 11.0 / 12.0);

impl Signex {
    pub(super) fn view_net_hover_card(
        &self,
        card: &HoverCard,
        window_id: iced::window::Id,
    ) -> Element<'_, Message> {
        use signex_widgets::text;

        let tokens = &self.document_view.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let ink = crate::styles::ti(tokens.text);
        let muted = crate::styles::tokens::resolve(tokens).ink4;
        let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
        let ui_font_name = self.panel_ctx_for(window_id).ui_font_name.as_str();

        let pin_count = (card.pins.len() + card.hidden) as i64;
        let pill = iced::widget::container(
            text(signex_i18n::t!(
                i18n,
                signex_i18n::ids::OVERLAY_NET_CARD_PIN_COUNT,
                "count" => pin_count
            ))
            .size(TS.caption)
            .wrapping(iced::widget::text::Wrapping::None),
        )
        .padding([0.0, PILL_PAD_X])
        .style(crate::styles::pill(
            tokens,
            crate::styles::PillTone::Neutral,
        ))
        .boxed();
        let wire = crate::styles::ti(self.document_view.panel_ctx.canvas_colors.wire);
        let title = hover_card_title(
            crate::styles::tinted_glyph(
                crate::icons::icon_wire(self.ui_state.effective_theme_id()),
                HOVER_GLYPH,
                wire,
            ),
            signex_i18n::UserText::new(card.name.clone()),
            Some(pill),
            ui_font_name,
            ink,
        );

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(4);
        rows.push(title);
        if let Some(class) = &card.class {
            rows.push(
                text(signex_i18n::t!(i18n, signex_i18n::ids::OVERLAY_NET_CARD_CLASS, "class" => class.as_str()))
                    .size(TS.caption)
                    .color(muted)
                    .boxed(),
            );
        }
        // One pin a line, its sheet muted beside it: the mockup lists pins
        // only, but the sheet is how a pin on another page is found.
        let pins: Vec<Element<'_, Message>> = card
            .pins
            .iter()
            .map(|pin| {
                iced::widget::row![
                    // The pin as the design names it (designator and number).
                    text(signex_i18n::UserText::new(pin.pin.clone()))
                        .font(mono)
                        .size(TS.caption)
                        .color(ink),
                    text(signex_i18n::UserText::new(pin.sheet.clone()))
                        .font(mono)
                        .size(TS.caption)
                        .color(muted),
                ]
                .spacing(SP.s)
                .boxed()
            })
            .collect();
        if !pins.is_empty() {
            rows.push(iced::widget::column(pins).boxed());
        }
        if card.hidden > 0 {
            rows.push(
                text(signex_i18n::t!(
                    i18n,
                    signex_i18n::ids::OVERLAY_NET_CARD_MORE,
                    "count" => card.hidden.to_string()
                ))
                .size(TS.caption)
                .color(muted)
                .boxed(),
            );
        }
        let lines =
            card.pins.len() + usize::from(card.class.is_some()) + usize::from(card.hidden > 0);
        let height = HOVER_CARD_PAD_H + HOVER_TITLE_H + HOVER_TITLE_GAP + ROW_H * lines as f32;
        let width = card_width_estimate(card, ui_font_name, i18n);

        let body = hover_card(
            iced::widget::column(rows).spacing(HOVER_TITLE_GAP).boxed(),
            width,
            tokens,
        );

        let (x, y) = canvas_region::place_card(
            (card.screen.0 as f32, card.screen.1 as f32),
            (width, height),
            self.card_bounds(window_id),
        );
        signex_widgets::Translate::new(body, (x, y)).boxed()
    }
}

/// The card's width: the widest of the title row (glyph, name and pill), the
/// class line and the pin lines, plus the padding, held between
/// [`HOVER_CARD_MIN_W`] and [`HOVER_CARD_MAX_W`].
fn card_width_estimate(card: &HoverCard, ui_family: &str, i18n: &signex_i18n::I18n) -> f32 {
    let ui = signex_types::font::nonblank_family(ui_family);
    let mono = Some(crate::fonts::CHROME_MONO_FONT);
    let width = signex_types::font::estimate_text_width_px;
    let pill = i18n.message_plain(
        signex_i18n::ids::OVERLAY_NET_CARD_PIN_COUNT,
        &[("count", ((card.pins.len() + card.hidden) as i64).into())],
    );
    let mut widest = HOVER_GLYPH
        + HOVER_TITLE_GAP
        + width(&card.name, TS.body, ui)
        + HOVER_TITLE_GAP
        + width(&pill, TS.caption, ui)
        + 2.0 * PILL_PAD_X;
    if let Some(class) = &card.class {
        let shown = i18n.message_plain(
            signex_i18n::ids::OVERLAY_NET_CARD_CLASS,
            &[("class", class.as_str().into())],
        );
        widest = widest.max(width(&shown, TS.caption, ui));
    }
    for pin in &card.pins {
        widest = widest.max(width(&format!("{}{}", pin.pin, pin.sheet), TS.caption, mono) + SP.s);
    }
    (widest + HOVER_CARD_PAD_W)
        .ceil()
        .clamp(HOVER_CARD_MIN_W, HOVER_CARD_MAX_W)
}

#[cfg(test)]
mod tests {
    use crate::app::state::net_nav::HoverCard;
    use crate::app::state::net_nav::hover::HoverCardPin;

    fn card(pins: &[(&str, &str)], hidden: usize) -> HoverCard {
        HoverCard {
            name: "VCC".into(),
            class: Some("Power".into()),
            pins: pins
                .iter()
                .map(|(pin, sheet)| HoverCardPin {
                    pin: (*pin).into(),
                    sheet: (*sheet).into(),
                })
                .collect(),
            hidden,
            screen: (0, 0),
        }
    }

    fn texts(card: &HoverCard) -> Vec<String> {
        let (app, _boot) = crate::app::Signex::new();
        crate::layout_probe::lay_out_texts(
            app.view_net_hover_card(card, iced::window::Id::unique()),
            iced::Size::new(800.0, 600.0),
        )
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
    }

    /// The rendered card reads as the `format!` lines it replaced (#2059):
    /// the class line, and the hidden-pin count without grouping. Each pin
    /// keeps its sheet beside it.
    #[test]
    fn the_card_names_the_class_and_the_hidden_pins() {
        let texts = texts(&card(&[("R1.2", "Main")], 1200));
        for expected in ["Class: Power", "R1.2", "Main", "+1200 more"] {
            assert!(
                texts.iter().any(|t| t == expected),
                "{expected:?} not in {texts:?}"
            );
        }
    }

    /// Mockup row 10: the pill beside the name counts every pin of the net, the shown
    /// and the hidden ones, in the singular for one.
    #[test]
    fn the_pin_count_pill_counts_the_shown_and_the_hidden_pins() {
        let three = texts(&card(&[("R1.2", "Main"), ("C3.1", "Power")], 1));
        assert!(three.iter().any(|t| t == "3 pins"), "{three:?}");
        let one = texts(&card(&[("R1.2", "Main")], 0));
        assert!(one.iter().any(|t| t == "1 pin"), "{one:?}");
    }
}
