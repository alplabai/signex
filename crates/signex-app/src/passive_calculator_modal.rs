//! Tools ▸ Passive Network Calculator — the resistor / capacitor / inductor
//! network calculator plus the RKM encoder (mockup row 17). It opens in its
//! own OS window like the other tool dialogs (docs/DETACHED_MODALS.md); the
//! in-window card below is the fallback when that window is not available.

use iced::widget::{Widget as _, column, container};
use iced::{Element, Length};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::passive_calculator::{CalculatorControl, CalculatorLook, CalculatorPalette};

use crate::app::view::dialogs::modal_title_header;
use crate::app::{Message, OverlayMsg};
use crate::panels::enable_section::enable_section_style;
use crate::styles;
use crate::styles::tokens::{hover_overlay, resolve};
use crate::styles::{TypeScale, ti};

/// Card size: the mockup's 680 px dialog, tall enough for a two-part
/// result without scrolling.
const MODAL_W: f32 = 680.0;
const MODAL_H: f32 = 600.0;

/// The calculator painted as a card in the main window (the fallback when
/// its own window is not available).
pub fn view<'a>(
    i18n: &'a I18n,
    tokens: &'a ThemeTokens,
    control: &'a CalculatorControl,
) -> Element<'a, Message> {
    build(i18n, tokens, control, false)
}

/// The calculator as the content of its own OS window: no header (the
/// window title names it), filling the window.
pub fn window_view<'a>(
    i18n: &'a I18n,
    tokens: &'a ThemeTokens,
    control: &'a CalculatorControl,
) -> Element<'a, Message> {
    build(i18n, tokens, control, true)
}

/// The calculator's colours, faces, mode switch and enable-section look on
/// `tokens`.
pub(crate) fn calculator_look(tokens: &ThemeTokens) -> CalculatorLook<'static> {
    let r = resolve(tokens);
    let palette = CalculatorPalette {
        ink: ti(tokens.text),
        ink2: ti(tokens.text_secondary),
        ink3: r.ink3,
        ink4: r.ink4,
        accent: ti(tokens.accent),
        accent_text: r.accent_text,
        border: ti(tokens.border),
        line_soft: r.line_soft,
        line_strong: r.line_strong,
        surface: ti(tokens.paper),
        panel: ti(tokens.panel_bg),
        field: r.field,
        sunken: r.sunken,
        primary: r.primary,
        primary_ink: r.primary_ink,
        hover: hover_overlay(tokens),
        ok: ti(tokens.success),
        ok_soft: r.ok_soft,
        warn: ti(tokens.warning),
        warn_soft: r.warn_soft,
        error: ti(tokens.error),
    };
    CalculatorLook {
        palette,
        mono: crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT),
        segmented: r.segmented(TypeScale::DEFAULT.body),
        enable_section: enable_section_style(tokens),
    }
}

fn build<'a>(
    i18n: &'a I18n,
    tokens: &'a ThemeTokens,
    control: &'a CalculatorControl,
    windowed: bool,
) -> Element<'a, Message> {
    // The control scrolls its own body, so this wrapper only bounds it —
    // nesting a second scrollable would swallow wheel events before the
    // inner one sees them.
    let body = container(
        control
            .view(i18n, tokens, calculator_look(tokens))
            .map(Message::PassiveCalculator),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    if windowed {
        return container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(styles::modal_card(tokens))
            .boxed();
    }
    let header = modal_title_header(
        t!(i18n, ids::DIALOG_TITLE_PASSIVE_CALCULATOR).into_string(),
        Message::Overlay(OverlayMsg::ClosePassiveCalculator),
        tokens,
    );
    let card = container(column![header, body].spacing(0))
        .width(Length::Fixed(MODAL_W))
        .height(Length::Fixed(MODAL_H))
        .clip(true)
        .style(styles::modal_card(tokens));

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    /// The calculator paints with the resolved Graphite tokens on both
    /// themes: the sunken prefix chip and mode-switch track, the dialog
    /// surface under the switched-off production date, the primary button.
    #[test]
    fn the_look_reads_the_resolved_graphite_tokens() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let r = resolve(&tokens);
            let look = calculator_look(&tokens);
            assert_eq!(look.palette.sunken, r.sunken, "{id:?}");
            assert_eq!(look.segmented.track, r.sunken, "{id:?}");
            assert_eq!(look.palette.surface, ti(tokens.paper), "{id:?}");
            assert_eq!(look.palette.primary, r.primary, "{id:?}");
            assert_eq!(look.palette.ok_soft, r.ok_soft, "{id:?}");
        }
    }
}
