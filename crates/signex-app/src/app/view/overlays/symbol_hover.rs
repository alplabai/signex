//! The symbol hover card (Graphite mockup row 10): a placed symbol's designator as the
//! title, then its value, footprint and library beside a label gutter. The
//! card shell and title row are shared with the net card (`net_hover.rs`).

use super::*;

/// Widest a hover card grows; past it the values wrap (#2100, #2092).
pub(super) const HOVER_CARD_MAX_W: f32 = 320.0;
/// Narrowest a hover card gets (220 px at the default base).
pub(super) const HOVER_CARD_MIN_W: f32 = crate::styles::chrome_px(220.0 * 11.0 / 12.0);
/// Card padding: 8 px top and bottom, 10 px at the sides (default base).
const HOVER_CARD_PAD_Y: f32 = crate::styles::chrome_px(8.0 * 11.0 / 12.0);
const HOVER_CARD_PAD_X: f32 = crate::styles::chrome_px(10.0 * 11.0 / 12.0);
/// Left and right padding plus the 1 px border on each side.
pub(super) const HOVER_CARD_PAD_W: f32 = 2.0 * HOVER_CARD_PAD_X + 2.0;
/// Top and bottom padding plus the border.
pub(super) const HOVER_CARD_PAD_H: f32 = 2.0 * HOVER_CARD_PAD_Y + 2.0;
/// Gap under the title row and between its glyph and text (6 px).
pub(super) const HOVER_TITLE_GAP: f32 = crate::styles::chrome_px(6.0 * 11.0 / 12.0);
/// Side of the glyph in front of a card title (14 px).
pub(super) const HOVER_GLYPH: f32 = crate::styles::chrome_px(14.0 * 11.0 / 12.0);
/// Height budget of the title row, for placing the card.
pub(super) const HOVER_TITLE_H: f32 = 20.0;
/// The label gutter: six body ems (72 px at the 12 px body).
const LABEL_W: f32 = TS.body * 6.0;
/// Grid gaps of the field rows: 2 px between rows, 10 px between the label
/// and its value.
const ROW_GAP: f32 = crate::styles::chrome_px(2.0 * 11.0 / 12.0);
const COLUMN_GAP: f32 = HOVER_CARD_PAD_X;

impl Signex {
    /// Hover card of a placed symbol: its designator, then value, footprint
    /// and library. Only paints after the cursor has dwelled on a Symbol hit
    /// for 700 ms, so impulsive motion does not pop it. None while text is
    /// being edited, when no schematic is active, or when the uuid no longer
    /// resolves (the symbol was deleted while the dwell timer ran).
    pub(in crate::app::view) fn view_hover_tooltip(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        use signex_i18n::{Localized, ids, t};
        use signex_widgets::text;

        if self.interaction_state.editing_text.is_some() {
            return None;
        }
        // A net card (a wire, pin or label, #1270) takes the place of the
        // symbol card while the pointer rests on a pin.
        if let Some(card) = &self.document_state.net_nav.hover.card {
            return Some(self.view_net_hover_card(card, window_id));
        }
        let interaction = &self.interaction_state;
        let uuid = interaction.hover_symbol_uuid?;
        let started = interaction.hover_started_at?;
        if started.elapsed() < std::time::Duration::from_millis(700) {
            return None;
        }
        let (sx, sy) = interaction.hover_screen_pos?;
        let active_path = self.document_state.active_path.as_ref()?;
        let engine = self.document_state.engines.get(active_path)?;
        let symbol = engine.document().symbols.iter().find(|s| s.uuid == uuid)?;

        let tokens = &self.document_state.panel_ctx.tokens;
        let ink = crate::styles::ti(tokens.text);
        let label_ink = crate::styles::tokens::resolve(tokens).ink4;
        // The values are data read off the canvas, so they keep the canvas
        // font. Mockup row 10 draws the chrome mono instead; that is an open
        // maintainer decision.
        let canvas_font_name = self.document_state.panel_ctx.canvas_font_name.as_str();
        let canvas_font = crate::fonts::canvas_iced_font(canvas_font_name);
        let ui_font_name = self.panel_ctx_for(window_id).ui_font_name.as_str();

        let i18n = &self.ui_state.i18n;
        let mut fields: Vec<(Localized, String)> = Vec::with_capacity(3);
        fields.push((
            t!(i18n, ids::OVERLAY_SYMBOL_CARD_VALUE),
            crate::library::part_label::card_value(&symbol.value).to_owned(),
        ));
        if !symbol.footprint.is_empty() {
            fields.push((
                t!(i18n, ids::OVERLAY_SYMBOL_CARD_FOOTPRINT),
                symbol.footprint.clone(),
            ));
        }
        if !symbol.lib_id.is_empty() {
            fields.push((
                t!(i18n, ids::OVERLAY_SYMBOL_CARD_LIBRARY),
                self.library
                    .drift
                    .display_name(&symbol.lib_id, &self.library.open_libraries),
            ));
        }
        let designator = self.shown_designator(symbol).to_owned();
        let card_w = symbol_card_width(&designator, &fields, ui_font_name, canvas_font_name);

        let rows_len = fields.len();
        let rows: Vec<Element<'_, Message>> = fields
            .into_iter()
            .map(|(label, value)| {
                iced::widget::row![
                    text(label)
                        .size(TS.body)
                        .color(label_ink)
                        .width(Length::Fixed(LABEL_W)),
                    // The symbol's own field value (design data).
                    text(signex_i18n::UserText::new(value))
                        .font(canvas_font)
                        .size(TS.body)
                        .color(ink)
                        .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                        .width(Length::Fill),
                ]
                .spacing(COLUMN_GAP)
                .align_y(iced::Alignment::Start)
                .boxed()
            })
            .collect();

        let chip = crate::icons::icon_component(self.ui_state.effective_theme_id());
        let title = hover_card_title(
            crate::styles::tinted_glyph(chip, HOVER_GLYPH, ink),
            signex_i18n::UserText::new(designator),
            None,
            ui_font_name,
            ink,
        );
        let card = hover_card(
            iced::widget::column![title, iced::widget::column(rows).spacing(ROW_GAP)]
                .spacing(HOVER_TITLE_GAP)
                .boxed(),
            card_w,
            tokens,
        );

        // Below-right of the cursor, flipped or clamped to stay on the
        // canvas (not the Properties panel beside it). Rows wrap, so the
        // height budgets a few lines each plus the card padding; iced lays
        // the card out at its true size.
        const ESTIMATED_ROW_H: f32 = 28.0;
        let estimated_h =
            HOVER_CARD_PAD_H + HOVER_TITLE_H + HOVER_TITLE_GAP + ESTIMATED_ROW_H * rows_len as f32;
        let (x, y) =
            canvas_region::place_card((sx, sy), (card_w, estimated_h), self.card_bounds(window_id));
        Some(signex_widgets::Translate::new(card, (x, y)).boxed())
    }
}

/// The card both hover cards draw (mockup row 10): `content` on the raised surface with
/// a 1 px border, the card radius and the floating shadow of the menus and
/// popovers, padded 8 / 10, `width` px wide.
pub(super) fn hover_card<'a>(
    content: Element<'a, Message>,
    width: f32,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let raised = crate::styles::tokens::resolve(tokens).raised;
    let border_c = crate::styles::ti(tokens.border);
    let shadow = crate::styles::floating_shadow(tokens);
    iced::widget::container(content)
        .padding([HOVER_CARD_PAD_Y, HOVER_CARD_PAD_X])
        .width(Length::Fixed(width))
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(raised)),
            border: iced::Border {
                color: border_c,
                width: 1.0,
                radius: crate::styles::Radii::GRAPHITE.card.into(),
                ..iced::Border::default()
            },
            shadow,
            ..iced::widget::container::Style::default()
        })
        .boxed()
}

/// A hover card's title row: `glyph`, `title` in the semibold UI face, and
/// `trailing` (a pill) at the right end.
pub(super) fn hover_card_title<'a>(
    glyph: Element<'a, Message>,
    title: signex_i18n::UserText,
    trailing: Option<Element<'a, Message>>,
    ui_font_name: &str,
    ink: iced::Color,
) -> Element<'a, Message> {
    let font = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(ui_font_name)
    };
    let mut line = iced::widget::row![
        glyph,
        signex_widgets::text(title)
            .size(TS.body)
            .font(font)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::Glyph),
    ]
    .spacing(HOVER_TITLE_GAP)
    .align_y(iced::Alignment::Center);
    if let Some(trailing) = trailing {
        line = line
            .push_boxed(iced::widget::Space::new().width(Length::Fill))
            .push(trailing);
    }
    line.width(Length::Fill).boxed()
}

/// Width of the symbol card: the wider of the title row and the label
/// gutter plus the longest value, plus the padding, held between
/// [`HOVER_CARD_MIN_W`] and [`HOVER_CARD_MAX_W`].
fn symbol_card_width(
    title: &str,
    fields: &[(signex_i18n::Localized, String)],
    ui_family: &str,
    value_family: &str,
) -> f32 {
    let ui = signex_types::font::nonblank_family(ui_family);
    let values = signex_types::font::nonblank_family(value_family);
    let title_w = HOVER_GLYPH
        + HOVER_TITLE_GAP
        + signex_types::font::estimate_text_width_px(title, TS.body, ui);
    let value_w = fields
        .iter()
        .map(|(_, value)| signex_types::font::estimate_text_width_px(value, TS.body, values))
        .fold(0.0, f32::max);
    (title_w.max(LABEL_W + COLUMN_GAP + value_w) + HOVER_CARD_PAD_W)
        .ceil()
        .clamp(HOVER_CARD_MIN_W, HOVER_CARD_MAX_W)
}

#[cfg(test)]
mod tests {
    use signex_types::schematic::{Point, Symbol};

    const R6: uuid::Uuid = uuid::Uuid::from_u128(6);

    /// Mockup row 10: the designator is the title, above Value / Footprint / Library;
    /// no "Designator" row is left, and each value sits beside its label.
    #[test]
    fn the_symbol_card_titles_the_designator_over_its_field_rows() {
        let (mut app, main) = crate::canvas::overlay_camera_tests::graphite_app_showing(|sheet| {
            sheet.symbols.push(Symbol {
                uuid: R6,
                lib_id: "Device:R".to_string(),
                reference: "R6".to_string(),
                value: "10k".to_string(),
                footprint: "Chip-0805".to_string(),
                position: Point::new(40.0, 40.0),
                ..Default::default()
            });
        });
        let window = iced::Size::new(1400.0, 900.0);
        app.ui_state.window_size = (window.width, window.height);
        let interaction = &mut app.interaction_state;
        interaction.hover_symbol_uuid = Some(R6);
        interaction.hover_started_at =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(2));
        interaction.hover_screen_pos = Some((100.0, 100.0));

        let card = app.view_hover_tooltip(main).expect("the card shows");
        let texts = crate::layout_probe::lay_out_texts(card, window);
        let find = |label: &str| {
            texts
                .iter()
                .find(|t| t.text == label)
                .unwrap_or_else(|| panic!("{label:?} not in {texts:?}"))
        };
        let title = find("R6");
        for label in ["Value", "10k", "Footprint", "Chip-0805", "Library"] {
            assert!(
                find(label).bounds.y > title.bounds.y,
                "{label:?} sits under the title"
            );
        }
        assert_eq!(find("Value").bounds.y, find("10k").bounds.y);
        assert!(find("10k").bounds.x > find("Value").bounds.x);
        assert!(texts.iter().all(|t| t.text != "Designator"), "{texts:?}");
    }

    /// The card is never narrower than the mockup row 10 minimum nor wider than the cap.
    #[test]
    fn the_symbol_card_width_stays_between_the_minimum_and_the_cap() {
        let short = super::symbol_card_width("R1", &[], "", "");
        assert_eq!(short, super::HOVER_CARD_MIN_W);
        let long_value = "X".repeat(200);
        let fields = [(
            signex_i18n::t!(
                signex_i18n::I18n::en_us_ref(),
                signex_i18n::ids::OVERLAY_SYMBOL_CARD_FOOTPRINT
            ),
            long_value,
        )];
        assert_eq!(
            super::symbol_card_width("R1", &fields, "", ""),
            super::HOVER_CARD_MAX_W
        );
    }
}
