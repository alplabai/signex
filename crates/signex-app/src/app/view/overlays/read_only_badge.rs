//! The "read-only" badge drawn over the canvas of a sheet written by a newer
//! Signex (#1402), so the state is visible before the user tries an edit.

use iced::widget::{container, row, svg};
use iced::{Background, Border, Element, Length, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use crate::app::Message;
use crate::styles;
const TS: styles::TypeScale = styles::TypeScale::DEFAULT;
const SP: styles::Spacing = styles::Spacing::DEFAULT;

/// Width of the tooltip, in caption-size ems, so its two sentences wrap.
const TIP_WIDTH_EM: f32 = 24.0;

/// Strength of the warning colour in the badge's outline.
const OUTLINE_ALPHA: f32 = 0.3;

/// Padlock glyph of the badge, tinted at draw time.
static LOCK_GLYPH: std::sync::LazyLock<svg::Handle> = std::sync::LazyLock::new(|| {
    svg::Handle::from_memory(
        br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/></svg>"##
            .as_slice(),
    )
});

/// Top-right badge: a lock and the state in a warning pill, with a tooltip
/// saying why and what to do. Nothing in it takes a click (a tooltip only
/// watches the pointer), so every press falls through to the canvas below.
/// Its text is `overlay-read-only-badge`; the tab says the same word.
pub(in crate::app::view) fn read_only_badge<'a>(
    tokens: &ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, Message> {
    let (fill, ink) = styles::pill_colors(tokens, styles::PillTone::Warn);
    let outline = iced::Color {
        a: OUTLINE_ALPHA,
        ..ink
    };
    let lock = svg(LOCK_GLYPH.clone())
        .width(Length::Fixed(TS.caption))
        .height(Length::Fixed(TS.caption))
        .style(move |_: &iced::Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let badge = signex_i18n::t!(i18n, signex_i18n::ids::OVERLAY_READ_ONLY_BADGE);
    let label = text(badge)
        .size(TS.caption)
        .font(iced::Font {
            weight: iced::font::Weight::Medium,
            ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
        })
        .color(ink);
    let chip = container(
        row![lock, label]
            .spacing(SP.s + SP.xs)
            .align_y(iced::Alignment::Center),
    )
    .height(Length::Fixed(styles::ChromeMetrics::DEFAULT.field))
    .align_y(iced::alignment::Vertical::Center)
    .padding([0.0, SP.m + SP.xs])
    .style(move |_: &iced::Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            color: outline,
            width: 1.0,
            radius: styles::Radii::GRAPHITE.pill.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let with_tip = signex_widgets::tooltip(
        chip,
        read_only_tip(tokens, i18n),
        iced::widget::tooltip::Position::Bottom,
    )
    .gap(SP.s);
    container(with_tip)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Top)
        .padding(iced::Padding::ZERO.top(badge_top()).right(SP.m))
        .boxed()
}

/// Canvas-local y of the badge's top edge: under the Active Bar, which the
/// overlay layer centres on the whole window and so can reach the canvas's
/// right corner when a dock narrows the canvas (mockup row 74).
fn badge_top() -> f32 {
    let metrics = signex_widgets::active_bar::Metrics::GRAPHITE;
    crate::active_bar::bar_top_margin() + crate::active_bar::bar_height_for(&metrics) + SP.m
}

/// Why the sheet is read-only and what to do about it.
fn read_only_tip<'a>(tokens: &ThemeTokens, i18n: &signex_i18n::I18n) -> Element<'a, Message> {
    let r = styles::tokens::resolve(tokens);
    let (fill, ink) = (r.primary, r.primary_ink);
    container(
        text(signex_i18n::t!(
            i18n,
            signex_i18n::ids::OVERLAY_READ_ONLY_BADGE_TOOLTIP
        ))
        .size(TS.caption)
        .color(ink),
    )
    .width(Length::Fixed(TS.caption * TIP_WIDTH_EM))
    .padding([SP.xs, SP.s + SP.xs])
    .style(move |_: &iced::Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: styles::Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::read_only_badge;

    /// Mockup row 74: the Active Bar spans canvas-local y from its top
    /// margin to its bottom edge and is centred on the window, so with a
    /// dock open its right end reaches the badge's corner. The whole pill,
    /// lock and first letter included, must sit below the bar.
    #[test]
    fn the_badge_sits_below_the_active_bar() {
        let (app, _boot) = crate::app::Signex::new();
        let i18n = signex_i18n::I18n::en_us();
        let label = i18n.message_plain(signex_i18n::ids::OVERLAY_READ_ONLY_BADGE, &[]);
        let badge = read_only_badge(&app.document_state.panel_ctx.tokens, &i18n);
        let texts = crate::layout_probe::lay_out_texts(badge, iced::Size::new(900.0, 500.0));
        let text = crate::layout_probe::find(&texts, &label);
        let metrics = signex_widgets::active_bar::Metrics::GRAPHITE;
        let bar_bottom =
            crate::active_bar::bar_top_margin() + crate::active_bar::bar_height_for(&metrics);
        assert!(
            text.bounds.y >= bar_bottom,
            "badge text top {} is inside the Active Bar (bottom {bar_bottom})",
            text.bounds.y
        );
        assert!(
            text.bounds.x + text.bounds.width <= 900.0,
            "the badge stays on the canvas: {:?}",
            text.bounds
        );
    }

    #[test]
    fn the_tooltip_says_why_and_what_to_do() {
        let tip = signex_i18n::I18n::en_us_ref()
            .message_plain(signex_i18n::ids::OVERLAY_READ_ONLY_BADGE_TOOLTIP, &[]);
        assert!(tip.contains("saved by a newer"), "{tip}");
        assert!(tip.contains("Update"), "{tip}");
    }
}
