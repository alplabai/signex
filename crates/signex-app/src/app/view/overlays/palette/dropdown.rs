//! The command palette card and its backdrop. In the main window the card
//! hangs just under the title bar, centred on the search field (or the search
//! button), and the field is the query input, so the card has no search row.
//! A palette in its own window, or under the search button, carries its own
//! input row. Scoring and grouping happen in `app::command_palette`, so this
//! stays a thin renderer.

use iced::widget::{Space, column, container, mouse_area, row, scrollable};
use iced::{Background, Border, Color, Element, Length, Rectangle, Widget as _};
use signex_i18n::{ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, text_input};

use super::{SP, TS};
use crate::app::command_palette::{COMMAND_PALETTE_INPUT_ID, COMMAND_PALETTE_WINDOW_INPUT_ID};
use crate::app::view::chrome::SearchSlot;
use crate::app::{CommandPaletteMsg, Message, Signex};
use crate::styles::ChromeMetrics;

/// Width of the command palette card in an undocked window.
const WINDOW_PALETTE_WIDTH: f32 = 480.0;
/// Width of the Graphite palette card under the chrome strip (640 px, see
/// DECISIONS 2026-11-10); independent of the search field's own width.
const GRAPHITE_PALETTE_WIDTH: f32 = 640.0;
/// Tallest the card grows.
const CARD_MAX_H: f32 = 360.0;
/// Gap between the card and the window edges it is clamped to (mockup row 9,
/// accepted by Caner).
const PALETTE_EDGE_GAP: f32 = SP.xl;

impl Signex {
    /// The palette card: an optional input row, the grouped result list and
    /// the hint footer.
    pub(in crate::app::view) fn view_command_palette_dropdown(&self) -> Element<'_, Message> {
        use crate::app::command_palette::MAX_RESULTS;

        let palette = &self.ui_state.command_palette;
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &*self.ui_state.i18n;
        let muted_c = crate::styles::ti(tokens.text_secondary);

        // Ranked and grouped in `update` (`refresh_command_palette_results`),
        // read here.
        let results = &palette.results;
        let total = palette.total_matches;
        let selected = palette.selected_index.min(total.saturating_sub(1));

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(MAX_RESULTS.min(total));
        for (display_idx, entry) in results.iter().enumerate() {
            let is_active = display_idx == selected;
            // `results` is already grouped (see `group_for_display`), so a
            // header goes wherever the source changes.
            if display_idx == 0 || results[display_idx - 1].source != entry.source {
                let count = results.iter().filter(|e| e.source == entry.source).count();
                rows.push(super::group_header(i18n, tokens, entry.source, count));
            }
            rows.push(super::result_row(
                i18n,
                tokens,
                entry,
                &palette.query,
                is_active,
                Message::CommandPalette(CommandPaletteMsg::Select(display_idx)),
            ));
        }

        // The title-bar field is the main window's input; a palette without
        // one carries its own input row at the top of the card.
        let slot = palette.window.is_none().then(|| self.chrome_search_slot());
        let input: Option<Element<'_, Message>> = match (palette.window, slot) {
            (Some(window), _) => Some(card_input(
                tokens,
                &palette.query,
                t!(i18n, ids::OVERLAY_PALETTE_WINDOW_SEARCH),
                COMMAND_PALETTE_WINDOW_INPUT_ID.clone(),
                move |q| Message::CommandPalette(CommandPaletteMsg::QueryTypedInWindow(window, q)),
            )),
            (None, Some(SearchSlot::Button { .. })) => Some(card_input(
                tokens,
                &palette.query,
                t!(i18n, ids::MENU_BAR_SEARCH_PLACEHOLDER),
                COMMAND_PALETTE_INPUT_ID.clone(),
                |q| Message::CommandPalette(CommandPaletteMsg::QueryChanged(q)),
            )),
            _ => None,
        };

        let body: Element<'_, Message> = if total == 0 {
            container(
                text(t!(i18n, ids::OVERLAY_PALETTE_NO_RESULTS))
                    .size(TS.label)
                    .color(muted_c),
            )
            .padding([SP.l, 14.0])
            .width(Length::Fill)
            .boxed()
        } else {
            let list = column(rows).spacing(SP.xs).padding(SP.s);
            // Bound the list so the card keeps room for its input row (when
            // it has one) and the hint footer.
            let bars = if input.is_some() { 2.0 } else { 1.0 };
            let list_cap = CARD_MAX_H - bars * (TS.body * 1.6 + 2.0 * SP.l);
            scrollable(list)
                .height(Length::Fit.max(list_cap))
                .style(super::scrollbar_style(tokens))
                .boxed()
        };

        // The footer's right side: how many rows matched ("N results",
        // mockup row 9, accepted by Caner), or, when rows are cut, how many
        // more there are.
        let footer: Element<'_, Message> = if total > 0 {
            let note = if total > MAX_RESULTS {
                t!(i18n, ids::OVERLAY_PALETTE_MORE_RESULTS, "count" => (total - MAX_RESULTS) as i64)
            } else {
                t!(i18n, ids::OVERLAY_PALETTE_RESULT_COUNT, "count" => total as i64)
            };
            super::hint_footer(i18n, tokens, results.get(selected), Some(note))
        } else {
            Space::new().height(0).boxed()
        };

        // The card centres under the same slot the title bar lays the field
        // or button out with, so it reads as the field's dropdown, including
        // after a resize. An undocked window has no title-bar field.
        let (origin, card_w) = match slot {
            Some(slot) => {
                let (field_x, field_w) = slot.anchor();
                let (x, w) = chrome_palette_card(field_x, field_w, self.ui_state.window_size.0);
                ((x, crate::menu_bar::MENU_BAR_HEIGHT + SP.s), w)
            }
            None => ((16.0, 16.0), WINDOW_PALETTE_WIDTH),
        };
        let mut content = column![];
        if let Some(input) = input {
            content = content.push(super::search_row(i18n, tokens, input));
        }
        let card = container(content.push(body).push(footer))
            .width(card_w)
            .height(Length::Fit.max(CARD_MAX_H))
            .padding(0)
            .style(crate::styles::modal_card(tokens));
        signex_widgets::Translate::new(card.boxed(), origin).boxed()
    }

    /// The palette's click-outside layer in the theme `scrim`. While the
    /// title-bar field is the query input the layer leaves the field
    /// uncovered, so it stays undimmed with its focus border, and a click on
    /// it keeps the palette open instead of closing it.
    pub(in crate::app::view) fn command_palette_backdrop(
        &self,
        scrim: Color,
    ) -> Element<'static, Message> {
        let close = || Message::CommandPalette(CommandPaletteMsg::Close);
        let hole = match (
            self.ui_state.command_palette.window,
            self.chrome_search_slot(),
        ) {
            (None, SearchSlot::Field { x, w }) => field_hole(x, w),
            _ => return Self::dismiss_layer_tinted(close(), scrim),
        };
        let piece = |width: Length, height: Length| -> Element<'static, Message> {
            container(Self::dismiss_layer_tinted(close(), scrim))
                .width(width)
                .height(height)
                .boxed()
        };
        let field = mouse_area(
            Space::new()
                .width(Length::Fixed(hole.width))
                .height(Length::Fill),
        )
        .on_press(Message::CommandPalette(CommandPaletteMsg::FieldClicked))
        .interaction(iced::mouse::Interaction::Text)
        .boxed();
        column![
            piece(Length::Fill, Length::Fixed(hole.y)),
            row![
                piece(Length::Fixed(hole.x), Length::Fill),
                field,
                piece(Length::Fill, Length::Fill),
            ]
            .height(Length::Fixed(hole.height)),
            piece(Length::Fill, Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
    }
}

/// The title-bar field's rectangle in window coordinates: `x`, `w` wide, at
/// the field height, centred in the strip.
fn field_hole(x: f32, w: f32) -> Rectangle {
    let height = ChromeMetrics::DEFAULT.field;
    Rectangle {
        x,
        y: (crate::menu_bar::MENU_BAR_HEIGHT - height) / 2.0,
        width: w,
        height,
    }
}

/// The card's own text input, transparent on the card.
fn card_input<'a>(
    tokens: &ThemeTokens,
    query: &'a str,
    placeholder: signex_i18n::Localized,
    id: iced::widget::Id,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let ink = crate::styles::ti(tokens.text);
    let ink4 = crate::styles::tokens::resolve(tokens).ink4;
    text_input(placeholder, query)
        .id(id)
        .on_input(on_input)
        .on_submit(Message::CommandPalette(CommandPaletteMsg::ExecuteSelected))
        .size(TS.body)
        .width(Length::Fill)
        .padding(0)
        .style(move |_: &iced::Theme, _| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..ink },
        })
        .boxed()
}

/// The x and width of the palette card under the chrome strip: 640 px (or
/// what a very narrow `window_w` leaves), its centre on the centre of the
/// field (or button) at `field_x`, `field_w` wide, clamped to stay inside
/// the window.
fn chrome_palette_card(field_x: f32, field_w: f32, window_w: f32) -> (f32, f32) {
    let w =
        GRAPHITE_PALETTE_WIDTH.min((window_w - 2.0 * PALETTE_EDGE_GAP).max(WINDOW_PALETTE_WIDTH));
    let centred = field_x + field_w / 2.0 - w / 2.0;
    let x = centred
        .min(window_w - w - PALETTE_EDGE_GAP)
        .max(PALETTE_EDGE_GAP);
    (x, w)
}

#[cfg(test)]
mod palette_width_tests {
    use super::*;

    /// The palette card is 640 px wide (maintainer, 2026-11-10).
    #[test]
    fn the_chrome_palette_card_is_640_px_wide() {
        assert_eq!(super::chrome_palette_card(700.0, 380.0, 1600.0).1, 640.0);
    }

    /// The card is centred on the search field when the window has room.
    #[test]
    fn the_chrome_palette_card_centres_on_the_search_field() {
        let (x, w) = super::chrome_palette_card(600.0, 380.0, 1600.0);
        assert_eq!(x + w / 2.0, 600.0 + 380.0 / 2.0);
    }

    /// Near either window edge the card is clamped inside the window.
    #[test]
    fn the_chrome_palette_card_stays_inside_the_window() {
        let gap = super::PALETTE_EDGE_GAP;
        let (x, _) = super::chrome_palette_card(100.0, 380.0, 900.0);
        assert_eq!(x, gap);
        let (x, w) = super::chrome_palette_card(1150.0, 380.0, 1560.0);
        assert_eq!(x + w, 1560.0 - gap);
    }

    /// Mockup row 9: the card's centre is the field's centre.
    #[test]
    fn the_card_centres_under_the_field() {
        let (x, w) = chrome_palette_card(600.0, 400.0, 1600.0);
        assert_eq!(x + w / 2.0, 600.0 + 400.0 / 2.0);
    }

    /// A field near the right edge pulls the card in, never past the window.
    #[test]
    fn the_card_is_clamped_to_the_right_window_edge() {
        let (x, w) = chrome_palette_card(1100.0, 400.0, 1440.0);
        assert!(
            (x + w - (1440.0 - PALETTE_EDGE_GAP)).abs() < 0.01,
            "x={x} w={w}"
        );
    }

    /// A field near the left edge (or a narrow window's search button far
    /// right of a short card) keeps the card inside the left edge.
    #[test]
    fn the_card_is_clamped_to_the_left_window_edge() {
        let (x, _) = chrome_palette_card(0.0, 160.0, 1440.0);
        assert_eq!(x, PALETTE_EDGE_GAP);
    }

    /// The search button on a narrow window: the card stays inside the
    /// window and covers the button's centre.
    #[test]
    fn the_card_under_the_search_button_stays_inside_a_narrow_window() {
        let (button_x, button_w) = (500.0, 26.0);
        let (x, w) = chrome_palette_card(button_x, button_w, 700.0);
        assert!(x >= PALETTE_EDGE_GAP && x + w <= 700.0 - PALETTE_EDGE_GAP + 0.01);
        assert!(x <= button_x + button_w / 2.0 && button_x + button_w / 2.0 <= x + w);
    }

    /// The field's hole in the backdrop is the field: its x and width, the
    /// field height, centred in the strip.
    #[test]
    fn the_backdrop_hole_is_the_title_bar_field() {
        let hole = field_hole(520.0, 400.0);
        assert_eq!((hole.x, hole.width), (520.0, 400.0));
        assert_eq!(hole.height, ChromeMetrics::DEFAULT.field);
        assert!((hole.y * 2.0 + hole.height - crate::menu_bar::MENU_BAR_HEIGHT).abs() < 0.01);
    }
}
