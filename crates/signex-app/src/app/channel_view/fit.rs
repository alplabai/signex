//! A channel card's variant state (#2662, mockup row 151): the header pill
//! that says how the channel fits in the active variant ("Lite: Fitted",
//! "Lite: 2 not fitted", "Lite: Not fitted"), a click on it that leaves the
//! whole channel out of the variant or lets the sheets decide again, and
//! the not-fitted look: a greyed card with a cross over it, and greyed
//! lines for the parts the variant leaves out. Base shows no pill: it is
//! the sheets.

use iced::widget::{container, svg};
use iced::{Color, Element, Length, Theme, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::text;

use crate::app::{CompiledDesignMsg, Message};
use crate::panels::PanelContext;
use crate::panels::compiled_design::{ChannelStripView, ChannelTabView};
use crate::styles::{PillTone, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
/// Horizontal padding of the pill, in `caption`-size characters.
const PILL_PAD_EMS: f32 = 0.5;

/// The cross over a channel the variant leaves out, stretched to the card.
const CROSS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" preserveAspectRatio="none"><path d="M4 4L96 96M96 4L4 96" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/></svg>"#;

/// The header pill of `tab` in the strip's variant; `None` in Base or
/// before a compile.
pub(super) fn pill<'a>(
    ctx: &'a PanelContext,
    view: &'a ChannelStripView,
    tab: &'a ChannelTabView,
) -> Option<Element<'a, Message>> {
    let variant = view.variant.as_deref()?;
    if tab.fits.is_empty() {
        return None;
    }
    let i18n = &*ctx.i18n;
    let not_fitted = tab.not_fitted();
    let (words, tone) = match not_fitted {
        0 => (
            t!(i18n, ids::PANELS_CHANNEL_CARD_FITTED, "variant" => variant),
            PillTone::Ok,
        ),
        _ if tab.is_not_fitted() => (
            t!(i18n, ids::PANELS_CHANNEL_CARD_NOT_FITTED, "variant" => variant),
            PillTone::Neutral,
        ),
        count => (
            t!(
                i18n,
                ids::PANELS_CHANNEL_CARD_PARTLY_FITTED,
                "variant" => variant,
                "count" => count
            ),
            PillTone::Warn,
        ),
    };
    let face = container(text(words).size(TS.caption))
        .padding([0.0, TS.caption * PILL_PAD_EMS])
        .style(crate::styles::pill(&ctx.tokens, tone));
    if view.is_fit_locked {
        return Some(face.boxed());
    }
    Some(
        signex_widgets::button(face)
            .padding(0)
            .on_press(Message::from(CompiledDesignMsg::ChannelFitPicked {
                parts: tab.fits.iter().map(|fit| fit.part.clone()).collect(),
                fitted: channel_click(tab),
            }))
            .style(|_: &Theme, _| iced::widget::button::Style::default())
            .boxed(),
    )
}

/// What a click on the pill sets: a channel the variant leaves out
/// entirely follows its sheets again, any other is left out.
pub(super) fn channel_click(tab: &ChannelTabView) -> Option<bool> {
    if tab.is_not_fitted() {
        None
    } else {
        Some(false)
    }
}

/// The ink of a designator line, greyed when the variant leaves the part
/// out.
pub(super) fn line_ink(ctx: &PanelContext, tab: &ChannelTabView, designator: &str) -> Color {
    let res = resolve(&ctx.tokens);
    if tab.is_part_not_fitted(designator) {
        faded(res.ink4)
    } else {
        res.ink3
    }
}

/// The cross laid over a card whose channel the variant leaves out.
pub(super) fn cross<'a>(ctx: &PanelContext) -> Element<'a, Message> {
    let ink = faded(ti(ctx.tokens.error));
    svg(crate::icons::static_glyph(CROSS))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(iced::ContentFit::Fill)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// `color` at the disabled opacity.
pub(super) fn faded(color: Color) -> Color {
    Color {
        a: color.a * crate::styles::DISABLED_OPACITY,
        ..color
    }
}
