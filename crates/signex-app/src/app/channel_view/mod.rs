//! The channel strip under a repeated sheet's canvas and the compiled
//! channel view (#1279). Editor is the editable sheet; a channel tab shows
//! the compiled channels as cards, read-only (the gate is
//! `Signex::refuse_if_channel_view`, at the mutation gateway). The strip's
//! data is `PanelContext::compiled_design.strip`, built in update. A card's
//! variant fit is a project setting, not a sheet edit, so the read-only gate
//! does not apply to it (#2662).

mod cards;
mod fit;
#[cfg(test)]
mod fit_tests;
mod strip;

use iced::widget::column;
use iced::{Element, Length, Widget as _};

use crate::app::{Message, Signex};

impl Signex {
    /// `document` with the channel strip under it when the active sheet is
    /// repeated; a channel tab replaces the sheet with the channel cards.
    /// Undocked windows show the sheet alone.
    pub(crate) fn with_channel_strip<'a>(
        &'a self,
        is_main: bool,
        document: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let ctx = &self.document_state.panel_ctx;
        let Some(view) = ctx.compiled_design.strip.as_ref().filter(|_| is_main) else {
            return document;
        };
        let body = match view.active {
            Some(active) => cards::view(ctx, view, active),
            None => document,
        };
        column![body, strip::view(ctx, view)]
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    //! mockup row 151: the compiled channel view's cards and channel tabs.

    use std::path::PathBuf;

    use iced::{Color, Size};
    use signex_types::schematic::ChannelOccurrence;
    use signex_types::theme::{ThemeId, theme_tokens};

    use crate::layout_probe::{LaidOutText, draw_pixels, lay_out_texts};
    use crate::panels::PanelContext;
    use crate::panels::compiled_design::{ChannelStripView, ChannelTabView};
    use crate::styles::ti;

    /// Four compiled channels, `active` chosen.
    fn channels(active: Option<u32>) -> ChannelStripView {
        let tab = |i: u32| ChannelTabView {
            channel: ChannelOccurrence {
                name: format!("CH{i}"),
                index: i,
                number: i,
                of: 4,
            },
            parts: Some(9),
            nets: Some(7),
            designators: vec![format!("CH{i}_R5")],
            fits: Vec::new(),
        };
        ChannelStripView {
            sheet: PathBuf::from("buck_channel.snxsch"),
            file: "buck_channel.snxsch".into(),
            tabs: (1..=4).map(tab).collect(),
            active,
            designator_map: Vec::new(),
            variant: None,
            is_fit_locked: false,
        }
    }

    fn context(theme: ThemeId) -> PanelContext {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.tokens = theme_tokens(theme);
        ctx.theme_id = theme;
        ctx
    }

    /// The text whose content, isolation marks dropped, is `label`.
    fn at<'t>(texts: &'t [LaidOutText], label: &str) -> &'t LaidOutText {
        texts
            .iter()
            .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == label)
            .unwrap_or_else(|| panic!("no {label:?} in {texts:#?}"))
    }

    fn luminance(r: f32, g: f32, b: f32) -> f32 {
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// mockup row 151: the floating Active Bar covers the top of the canvas
    /// region; every card header starts below it.
    #[test]
    fn card_headers_start_below_the_floating_active_bar() {
        let ctx = context(ThemeId::GraphiteDark);
        let view = channels(Some(1));
        let metrics = signex_widgets::active_bar::Metrics::GRAPHITE;
        let bar_bottom =
            crate::active_bar::bar_top_margin() + crate::active_bar::bar_height_for(&metrics);
        let texts = lay_out_texts(super::cards::view(&ctx, &view, 1), Size::new(1200.0, 700.0));
        for name in ["CH1", "CH2"] {
            let header = at(&texts, name).bounds;
            assert!(
                header.y >= bar_bottom,
                "{name} header at y {} is under the bar, which ends at {bar_bottom}",
                header.y
            );
        }
    }

    /// mockup row 151: an off channel tab's label reads on the strip in both
    /// themes (it was the button's default black, dark on the dark strip).
    #[test]
    fn off_channel_tab_labels_contrast_with_the_strip() {
        for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let ctx = context(theme);
            let view = channels(Some(1));
            let window = Size::new(800.0, 40.0);
            let texts = lay_out_texts(super::strip::view(&ctx, &view), window);
            let pixels = draw_pixels(super::strip::view(&ctx, &view), window);
            let ground: Color = ti(ctx.tokens.toolbar_bg);
            let ground = luminance(ground.r, ground.g, ground.b);
            for label in ["Editor", "CH2", "CH3", "CH4"] {
                let b = at(&texts, label).bounds;
                let mut contrast: f32 = 0.0;
                for y in b.y.floor() as usize..(b.y + b.height).ceil() as usize {
                    for x in b.x.floor() as usize..(b.x + b.width).ceil() as usize {
                        let i = (y * window.width as usize + x) * 4;
                        let [r, g, b] =
                            [pixels[i], pixels[i + 1], pixels[i + 2]].map(|c| f32::from(c) / 255.0);
                        contrast = contrast.max((luminance(r, g, b) - ground).abs());
                    }
                }
                assert!(
                    contrast > 0.25,
                    "{theme:?}: {label} barely differs from the strip ({contrast})"
                );
            }
        }
    }
}
