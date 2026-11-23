//! The Placement Paused banner: one line under the Active Bar while Tab
//! pauses a placement, so the sheet and the frozen ghost stay in view while
//! the panel edits the values (#2529).

use super::*;
use iced::widget::{Space, column, container, row};
use signex_types::theme::ThemeTokens;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Pause glyph (two bars) of the banner, tinted at draw time.
static PAUSE_GLYPH: std::sync::LazyLock<iced::widget::svg::Handle> = std::sync::LazyLock::new(
    || {
        iced::widget::svg::Handle::from_memory(
        br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#000"><rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/></svg>"##
            .as_slice(),
    )
    },
);

impl Signex {
    /// The banner while the canvas of `window_id`, the footprint editor it
    /// shows, or the board tool armed on its board has placement paused. Resume goes on placing with the
    /// edited values; Esc goes back to placing without them
    /// (`dispatch/escape.rs`). Only the banner takes the pointer: the layer
    /// around it is empty space, so the canvas keeps its hit-testing.
    pub(in crate::app::view) fn placement_paused_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        let footprint_paused = self
            .footprint_editor_in_window(window_id)
            .is_some_and(|(_, ed)| ed.state.placement_paused);
        let canvas_paused = self
            .interaction_state
            .canvas_for_window(window_id)
            .placement_paused;
        let board_paused = self.board_tool_paused_in_window(window_id);
        if !(canvas_paused || footprint_paused || board_paused) {
            return None;
        }
        let tokens = &self.document_state.panel_ctx.tokens;
        let banner = placement_paused_banner(tokens, &self.ui_state.i18n);
        Some(
            column![
                Space::new().height(banner_top(tokens)),
                container(scroll_guard(banner))
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            ]
            .boxed(),
        )
    }
}

/// Window y of the banner's top edge: under the Active Bar, which the
/// overlay layer centres on the window the same way.
fn banner_top(tokens: &ThemeTokens) -> f32 {
    let metrics = signex_widgets::active_bar::Metrics::GRAPHITE;
    canvas_region::chrome_top(tokens)
        + crate::active_bar::bar_top_margin()
        + crate::active_bar::bar_height_for(&metrics)
        + SP.m
}

/// Pause glyph, bold title, the body, the Esc hint and Resume, on one line.
fn placement_paused_banner<'a>(
    tokens: &ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, Message> {
    let r = crate::styles::tokens::resolve(tokens);
    let ink = crate::styles::ti(tokens.text);
    let (ink3, ink4) = (r.ink3, r.ink4);
    let title_font = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let glyph = iced::widget::svg(PAUSE_GLYPH.clone())
        .width(Length::Fixed(TS.title))
        .height(Length::Fixed(TS.title))
        .style(move |_: &iced::Theme, _| iced::widget::svg::Style {
            color: Some(ink3),
            ..Default::default()
        });
    let line = row![
        glyph,
        signex_widgets::text(signex_i18n::t!(
            i18n,
            signex_i18n::ids::OVERLAY_PLACEMENT_PAUSED_TITLE
        ))
        .size(TS.body)
        .font(title_font)
        .color(ink),
        signex_widgets::text(signex_i18n::t!(
            i18n,
            signex_i18n::ids::OVERLAY_PLACEMENT_PAUSED_BODY
        ))
        .size(TS.body)
        .color(ink3),
        signex_widgets::text(signex_i18n::t!(
            i18n,
            signex_i18n::ids::OVERLAY_PLACEMENT_PAUSED_ESC_HINT
        ))
        .size(TS.caption)
        .color(ink4),
        crate::styles::dialog_button(
            signex_i18n::t!(i18n, signex_i18n::ids::OVERLAY_PLACEMENT_PAUSED_RESUME),
            crate::styles::DialogButtonKind::Primary,
            Some(Message::Tool(ToolMessage::ResumePlacement)),
            tokens,
        ),
    ]
    .spacing(SP.m + SP.xs)
    .align_y(iced::Alignment::Center);
    let (raised, line_c) = (r.raised, crate::styles::ti(tokens.border));
    let shadow = crate::styles::floating_shadow(tokens);
    container(line)
        .padding(iced::Padding {
            top: SP.xs + SP.xs,
            right: SP.xs + SP.xs,
            bottom: SP.xs + SP.xs,
            left: SP.l,
        })
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(raised)),
            border: iced::Border {
                width: 1.0,
                color: line_c,
                radius: crate::styles::Radii::GRAPHITE.card.into(),
                ..iced::Border::default()
            },
            shadow,
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The banner sits under the Active Bar, not over it.
    #[test]
    fn the_banner_sits_below_the_active_bar() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let metrics = signex_widgets::active_bar::Metrics::GRAPHITE;
        let bar_bottom = canvas_region::chrome_top(&tokens)
            + crate::active_bar::bar_top_margin()
            + crate::active_bar::bar_height_for(&metrics);
        assert!(banner_top(&tokens) > bar_bottom);
    }

    #[test]
    fn a_paused_canvas_shows_the_banner_and_an_unpaused_one_does_not() {
        let (mut app, _boot) = Signex::new();
        let window = app
            .ui_state
            .main_window_id
            .unwrap_or_else(iced::window::Id::unique);
        app.ui_state.main_window_id = Some(window);
        assert!(app.placement_paused_overlay(window).is_none());
        app.interaction_state.active_canvas_mut().placement_paused = true;
        assert!(app.placement_paused_overlay(window).is_some());
    }

    /// The banner's wording follows the Esc decision (#2529): Esc goes back
    /// to placing, it does not cancel the tool.
    #[test]
    fn the_esc_hint_says_back_to_placing() {
        let i18n = signex_i18n::I18n::en_us_ref();
        let hint = i18n.message_plain(signex_i18n::ids::OVERLAY_PLACEMENT_PAUSED_ESC_HINT, &[]);
        assert_eq!(hint, "Esc back to placing");
        let body = i18n.message_plain(signex_i18n::ids::OVERLAY_PLACEMENT_PAUSED_BODY, &[]);
        assert_eq!(body, "Editing properties in the panel.");
    }
}
