//! The calculator's frame and the network page's inputs (mockup m17): the
//! mode switch under the dialog header, then the page in a scrolling body.

use iced::widget::{Column, container, pick_list, responsive, scrollable};
use iced::{Element, Length, Padding, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;

use super::{CalculatorControl, CalculatorMessage, CalculatorTab};
use crate::dropdown_style::GraphiteDropdown;
use crate::passive_calculator::domain::{ComponentKind, ESeries, SiPrefix};
use crate::passive_calculator::look::{CalculatorLook, CalculatorPalette};
use crate::passive_calculator::names::{component_name, target_error};
use crate::passive_calculator::solver::MAX_PARTS;
use crate::passive_calculator::widgets::{
    CM, NARROW_WIDTH, SP, TS, bare_input_style, composite_field, data, input_grid, labeled,
    panel_box, prefix_picker, primary_button, unit_suffix,
};
use crate::segmented::{SegmentedStyle, segmented};
use crate::theme_ext::{thin_scrollbar, thin_scrollbar_style};
use crate::{Label, text, text_input};

const MAX_COMPONENT_OPTIONS: [usize; MAX_PARTS] = [1, 2, 3, 4];

/// Width of the mode switch, in em of the body size (420 px at 12 px).
const MODE_SWITCH_EM: f32 = 35.0;
/// Shares of the input grid: Target, Preferred series, Maximum parts; the
/// Calculate button takes its own width (1.3fr 1fr .8fr auto).
const TARGET_SHARE: u16 = 13;
const SERIES_SHARE: u16 = 10;
const MAX_PARTS_SHARE: u16 = 8;

/// Fonts and colours of the network page, copied into its layout closure.
#[derive(Clone, Copy)]
pub(super) struct PageLook {
    pub palette: CalculatorPalette,
    pub mono: iced::Font,
}

impl CalculatorControl {
    /// The whole calculator: the mode switch, then the active page. `look`
    /// comes from the caller's theme (see [`CalculatorLook`]).
    pub fn view<'a>(
        &'a self,
        i18n: &'a I18n,
        tokens: &'a ThemeTokens,
        look: CalculatorLook<'a>,
    ) -> Element<'a, CalculatorMessage> {
        let CalculatorLook {
            palette,
            mono,
            segmented: switch_style,
            enable_section,
        } = look;
        let page = match self.active_tab {
            CalculatorTab::RkmEncoder => self
                .rkm_encoder
                .view(
                    i18n,
                    CalculatorLook {
                        palette,
                        mono,
                        segmented: switch_style,
                        enable_section,
                    },
                )
                .map(CalculatorMessage::RkmEncoder)
                .boxed(),
            CalculatorTab::Passive(_) => {
                let page_look = PageLook { palette, mono };
                responsive(move |size| self.network_page(i18n, tokens, page_look, size.width))
                    .height(Length::Shrink)
                    .boxed()
            }
        };
        let body = scrollable(container(page).padding(SP.xl).width(Length::Fill))
            .direction(scrollable::Direction::Vertical(thin_scrollbar(&CM)))
            .style(thin_scrollbar_style(palette.line_strong, palette.ink4))
            .height(Length::Fill);
        Column::new()
            .push(self.mode_switch(i18n, switch_style))
            .push(body.boxed())
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    }

    /// Resistor | Capacitor | Inductor | RKM Encoder, under the header.
    fn mode_switch<'a>(
        &'a self,
        i18n: &'a I18n,
        style: SegmentedStyle,
    ) -> Element<'a, CalculatorMessage> {
        let active = self.active_tab;
        let switch = responsive(move |size| {
            let mut options: Vec<(CalculatorTab, Label<'a>)> = ComponentKind::ALL
                .into_iter()
                .map(|kind| {
                    (
                        CalculatorTab::Passive(kind),
                        Label::from(component_name(i18n, kind)),
                    )
                })
                .collect();
            options.push((
                CalculatorTab::RkmEncoder,
                Label::from(t!(i18n, ids::PASSIVE_CALCULATOR_TAB_RKM)),
            ));
            container(segmented(
                options,
                active,
                style.fill_width(true),
                CalculatorMessage::TabChanged,
            ))
            .width(size.width.min(TS.body * MODE_SWITCH_EM))
            .boxed()
        })
        .height(Length::Shrink);
        container(switch)
            .padding(Padding {
                top: SP.l,
                right: SP.xl,
                bottom: 0.0,
                left: SP.xl,
            })
            .width(Length::Fill)
            .boxed()
    }

    /// The network page laid out for a body `width` px wide.
    pub(super) fn network_page<'a>(
        &'a self,
        i18n: &'a I18n,
        tokens: &'a ThemeTokens,
        look: PageLook,
        width: f32,
    ) -> Element<'a, CalculatorMessage> {
        let PageLook { palette, mono } = look;
        let is_narrow = width < NARROW_WIDTH;
        let state = self.active_state();

        let target = composite_field(
            vec![
                text_input(data("220"), &state.target_input)
                    .on_input(CalculatorMessage::TargetChanged)
                    .font(mono)
                    .size(TS.body)
                    .padding(0.0)
                    .width(Length::Fill)
                    .style(bare_input_style(palette))
                    .boxed(),
                prefix_picker(
                    state.prefix,
                    SiPrefix::for_kind(self.kind),
                    CalculatorMessage::PrefixChanged,
                    palette,
                    mono,
                ),
                unit_suffix(self.kind.unit_symbol(), palette),
            ],
            palette,
        );
        let series = pick_list(Some(state.series), ESeries::ALL, ESeries::to_string)
            .graphite()
            .on_select(CalculatorMessage::SeriesChanged)
            .width(Length::Fill)
            .boxed();
        let max_parts = pick_list(
            Some(state.max_components),
            MAX_COMPONENT_OPTIONS,
            usize::to_string,
        )
        .graphite()
        .on_select(CalculatorMessage::MaxComponentsChanged)
        .width(Length::Fill)
        .boxed();
        let inputs = input_grid(
            vec![
                (
                    TARGET_SHARE,
                    labeled(t!(i18n, ids::PASSIVE_CALCULATOR_TARGET), palette, target),
                ),
                (
                    SERIES_SHARE,
                    labeled(t!(i18n, ids::PASSIVE_CALCULATOR_SERIES), palette, series),
                ),
                (
                    MAX_PARTS_SHARE,
                    labeled(
                        t!(i18n, ids::PASSIVE_CALCULATOR_MAX_PARTS),
                        palette,
                        max_parts,
                    ),
                ),
                (
                    0,
                    primary_button(
                        t!(i18n, ids::PASSIVE_CALCULATOR_CALCULATE),
                        CalculatorMessage::Calculate,
                        palette,
                    ),
                ),
            ],
            is_narrow,
        );

        let mut page = Column::new()
            .push(
                text(t!(i18n, ids::PASSIVE_CALCULATOR_NETWORK_CAPTION))
                    .size(TS.caption)
                    .color(palette.ink4)
                    .boxed(),
            )
            .push(inputs)
            .spacing(SP.m)
            .width(Length::Fill);
        if let Some(error) = state.validation_error {
            page = page.push(
                text(target_error(i18n, error))
                    .size(TS.body)
                    .color(palette.error)
                    .boxed(),
            );
        }
        let below = match &state.result {
            Some(result) => self.result_view(result, i18n, tokens, look, is_narrow),
            None => panel_box(
                text(t!(i18n, ids::PASSIVE_CALCULATOR_EMPTY))
                    .size(TS.body)
                    .color(palette.ink3)
                    .boxed(),
                palette,
            ),
        };
        page.push(
            container(below)
                .padding(Padding {
                    top: SP.m,
                    ..Padding::ZERO
                })
                .boxed(),
        )
        .boxed()
    }
}
