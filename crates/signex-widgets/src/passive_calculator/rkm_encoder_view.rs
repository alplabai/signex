//! The RKM encoder page (mockup m17): Component, Value (with its SI prefix
//! and unit inside the field) and Tolerance on one line, the folding
//! "Optional" resistor fields, the enable-first production date, then the
//! Specification / RKM code / Production date code strip and the colour
//! codes.

use iced::alignment::Vertical;
use iced::widget::{Column, Row, container, pick_list, responsive};
use iced::{Background, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, ids, t};

use super::color_code::ComponentColorCode;
use super::color_code_view::color_code_representations;
use super::control::TARGET_UNAVAILABLE;
use super::domain::{ComponentKind, SiPrefix, Tolerance};
use super::look::{CalculatorLook, CalculatorPalette};
use super::names::{component_name, cycle_name, month_name};
use super::network::format_value;
use super::production_date_code::{ProductionDateCycle, ProductionMonth};
use super::rkm_code::{RatedPower, TemperatureCoefficient};
use super::rkm_encoder::{RkmEncoder, RkmEncoderMessage};
use super::widgets::{
    CM, NARROW_WIDTH, SP, TS, bare_pick_style, composite_field, ghost_button, input_grid, labeled,
    medium, metric_cell, metric_strip, metric_value, prefix_picker, unit_suffix, upper,
};
use crate::chevron::chevron;
use crate::dropdown_style::{GraphiteDropdown, pick_list_menu_style};
use crate::enable_section::EnableSection;
use crate::{PickListPlaceholder, button, text};

/// Shares of the input line: Component, Value, Tolerance (1fr 1.3fr 1fr).
const COMPONENT_SHARE: u16 = 10;
const VALUE_SHARE: u16 = 13;
const TOLERANCE_SHARE: u16 = 10;
/// Shares of the output strip: Specification, RKM code, date code (2fr 1fr 1fr).
const SPECIFICATION_SHARE: u16 = 2;
const CODE_SHARE: u16 = 1;
/// Indent of the production-date pickers under their enable box.
const DATE_INDENT: f32 = SP.l + SP.xl;

type Msg = RkmEncoderMessage;

impl RkmEncoder {
    pub fn view<'a>(&'a self, i18n: &'a I18n, look: CalculatorLook<'a>) -> Element<'a, Msg> {
        let CalculatorLook {
            palette,
            mono,
            mut enable_section,
            ..
        } = look;
        // The veil over the switched-off pickers is the dialog surface.
        enable_section.panel = palette.surface;

        let mut page = Column::new()
            .push(
                text(t!(i18n, ids::PASSIVE_CALCULATOR_RKM_CAPTION))
                    .size(TS.caption)
                    .color(palette.ink4)
                    .boxed(),
            )
            .push(
                responsive(move |size| self.inputs(i18n, palette, mono, size.width))
                    .height(Length::Shrink)
                    .boxed(),
            )
            .spacing(SP.m)
            .width(Length::Fill);
        if self.kind == ComponentKind::Resistor {
            page = page.push(self.optional_section(i18n, palette));
        }
        page = page.push(
            EnableSection::new(
                t!(i18n, ids::PASSIVE_CALCULATOR_PRODUCTION_DATE),
                self.is_production_date_enabled,
            )
            .on_toggle(Msg::ProductionDateToggled)
            .view(enable_section, self.production_date_pickers(i18n, palette)),
        );
        page = page.push(
            responsive(move |size| self.output(i18n, palette, mono, size.width))
                .height(Length::Shrink)
                .boxed(),
        );
        if self.is_production_date_enabled && self.production_date_code().is_none() {
            page = page.push(
                text(t!(i18n, ids::PASSIVE_CALCULATOR_DATE_INCOMPLETE))
                    .size(TS.caption)
                    .color(palette.ink4)
                    .boxed(),
            );
        }
        page.push(color_code_representations(
            i18n,
            ComponentColorCode::representations_for_kind_with_temperature_coefficient(
                self.kind,
                self.component(),
                self.tolerance,
                self.temperature_coefficient,
            ),
            self.kind,
            palette,
        ))
        .boxed()
    }

    /// Component, Value and Tolerance, laid out for `width` px.
    pub(super) fn inputs<'a>(
        &'a self,
        i18n: &'a I18n,
        palette: CalculatorPalette,
        mono: Font,
        width: f32,
    ) -> Element<'a, Msg> {
        let component = pick_list(
            Some(self.kind),
            ComponentKind::ALL,
            move |kind: &ComponentKind| component_name(i18n, *kind).into_string(),
        )
        .graphite()
        .on_select(Msg::KindChanged)
        .width(Length::Fill)
        .boxed();
        let value = pick_list(Some(self.value), self.value_options(), ToString::to_string)
            .on_select(Msg::ValueChanged)
            .font(mono)
            .text_size(TS.body)
            .padding(0.0)
            .width(Length::Fill)
            .style(bare_pick_style(palette, None))
            .menu_style(pick_list_menu_style)
            .boxed();
        let value_field = composite_field(
            vec![
                value,
                prefix_picker(
                    self.prefix,
                    SiPrefix::for_kind(self.kind),
                    Msg::PrefixChanged,
                    palette,
                    mono,
                ),
                unit_suffix(self.kind.unit_symbol(), palette),
            ],
            palette,
        );
        let tolerance = pick_list(Some(self.tolerance), Tolerance::ALL, ToString::to_string)
            .graphite()
            .on_select(Msg::ToleranceChanged)
            .width(Length::Fill)
            .boxed();
        input_grid(
            vec![
                (
                    COMPONENT_SHARE,
                    labeled(
                        t!(i18n, ids::PASSIVE_CALCULATOR_COMPONENT),
                        palette,
                        component,
                    ),
                ),
                (
                    VALUE_SHARE,
                    labeled(
                        t!(i18n, ids::PASSIVE_CALCULATOR_VALUE),
                        palette,
                        value_field,
                    ),
                ),
                (
                    TOLERANCE_SHARE,
                    labeled(
                        t!(i18n, ids::PASSIVE_CALCULATOR_TOLERANCE),
                        palette,
                        tolerance,
                    ),
                ),
            ],
            width < NARROW_WIDTH,
        )
    }

    /// The folding "Optional" section: temperature coefficient and rated
    /// power, with the button that clears both (`.sec`).
    fn optional_section<'a>(
        &'a self,
        i18n: &'a I18n,
        palette: CalculatorPalette,
    ) -> Element<'a, Msg> {
        let is_open = self.is_optional_expanded;
        let fold_label: Row<Element<'a, Msg>> = Row::new()
            .push(chevron(is_open, TS.section, palette.ink4).boxed())
            .push(
                text(upper(i18n, ids::PASSIVE_CALCULATOR_OPTIONAL))
                    .size(TS.section)
                    .font(medium(Font::DEFAULT))
                    .color(palette.ink3)
                    .boxed(),
            )
            .spacing(SP.xs)
            .align_y(Vertical::Center);
        let fold = button(fold_label)
            .on_press(Msg::OptionalSectionToggled)
            .width(Length::Fill)
            .padding(Padding::from([0.0, SP.xs]))
            .style(move |_: &Theme, status: button::Status| button::Style {
                background: matches!(status, button::Status::Hovered)
                    .then_some(Background::Color(palette.hover)),
                text_color: palette.ink3,
                ..button::Style::default()
            });
        let header = Row::new()
            .push(fold.boxed())
            .push(ghost_button(
                t!(i18n, ids::PASSIVE_CALCULATOR_CLEAR_OPTIONAL),
                Msg::ClearResistorOptionalFields,
                palette,
            ))
            .height(CM.field)
            .align_y(Vertical::Center)
            .width(Length::Fill);
        let mut section = Column::new().push(header.boxed()).width(Length::Fill);
        if is_open {
            let temperature = pick_list(
                self.temperature_coefficient,
                TemperatureCoefficient::ALL,
                ToString::to_string,
            )
            .graphite()
            .on_select(Msg::TemperatureCoefficientChanged)
            .placeholder_label(t!(i18n, ids::PASSIVE_CALCULATOR_NOT_SPECIFIED))
            .width(Length::Fill)
            .boxed();
            let power = pick_list(self.rated_power, RatedPower::ALL, ToString::to_string)
                .graphite()
                .on_select(Msg::RatedPowerChanged)
                .placeholder_label(t!(i18n, ids::PASSIVE_CALCULATOR_NOT_SPECIFIED))
                .width(Length::Fill)
                .boxed();
            section = section.push(
                container(input_grid(
                    vec![
                        (
                            1,
                            labeled(
                                t!(i18n, ids::PASSIVE_CALCULATOR_TEMPERATURE_COEFFICIENT),
                                palette,
                                temperature,
                            ),
                        ),
                        (
                            1,
                            labeled(
                                t!(i18n, ids::PASSIVE_CALCULATOR_RATED_POWER),
                                palette,
                                power,
                            ),
                        ),
                    ],
                    false,
                ))
                .padding(Padding::from([SP.s, 0.0]))
                .boxed(),
            );
        }
        section.boxed()
    }

    /// Cycle, Year and Month under the production-date enable box.
    fn production_date_pickers<'a>(
        &'a self,
        i18n: &'a I18n,
        palette: CalculatorPalette,
    ) -> Element<'a, Msg> {
        let not_specified = || t!(i18n, ids::PASSIVE_CALCULATOR_NOT_SPECIFIED);
        let cycle = pick_list(
            self.production_date_cycle,
            ProductionDateCycle::ALL,
            move |cycle: &ProductionDateCycle| cycle_name(i18n, *cycle).into_string(),
        )
        .graphite()
        .on_select(Msg::ProductionDateCycleChanged)
        .placeholder_label(not_specified())
        .width(Length::Fill)
        .boxed();
        let year = pick_list(
            self.production_year,
            self.production_year_options(),
            ToString::to_string,
        )
        .graphite()
        .on_select(Msg::ProductionYearChanged)
        .placeholder_label(not_specified())
        .width(Length::Fill)
        .boxed();
        let month = pick_list(
            self.production_month,
            ProductionMonth::ALL,
            move |month: &ProductionMonth| month_name(i18n, *month).into_string(),
        )
        .graphite()
        .on_select(Msg::ProductionMonthChanged)
        .placeholder_label(not_specified())
        .width(Length::Fill)
        .boxed();
        let pickers: Row<Element<'a, Msg>> = Row::new()
            .push(labeled(
                t!(i18n, ids::PASSIVE_CALCULATOR_CYCLE),
                palette,
                cycle,
            ))
            .push(labeled(
                t!(i18n, ids::PASSIVE_CALCULATOR_YEAR),
                palette,
                year,
            ))
            .push(labeled(
                t!(i18n, ids::PASSIVE_CALCULATOR_MONTH),
                palette,
                month,
            ))
            .spacing(SP.l)
            .width(Length::Fill);
        container(pickers)
            .padding(Padding {
                left: DATE_INDENT,
                bottom: SP.xs,
                ..Padding::ZERO
            })
            .width(Length::Fill)
            .boxed()
    }

    /// Specification / RKM code / Production date code, for `width` px.
    pub(super) fn output<'a>(
        &'a self,
        i18n: &'a I18n,
        palette: CalculatorPalette,
        mono: Font,
        width: f32,
    ) -> Element<'a, Msg> {
        let component = self.component();
        let mut specification = format!(
            "{} {}",
            format_value(component.value(), self.kind),
            self.tolerance
        );
        if let Some(temperature_coefficient) = self.temperature_coefficient {
            specification.push_str(&format!(" {temperature_coefficient}"));
        }
        if let Some(rated_power) = self.rated_power {
            specification.push_str(&format!(" {rated_power}"));
        }
        let date_value: Element<'a, Msg> = if !self.is_production_date_enabled {
            text(t!(i18n, ids::PASSIVE_CALCULATOR_DATE_OFF))
                .size(TS.body)
                .color(palette.ink4)
                .boxed()
        } else {
            match self.production_date_code() {
                Some(code) => metric_value(code.to_string(), palette, mono),
                None => metric_value(TARGET_UNAVAILABLE, palette, mono),
            }
        };
        let cells = vec![
            (
                SPECIFICATION_SHARE,
                metric_cell(
                    t!(i18n, ids::PASSIVE_CALCULATOR_SPECIFICATION),
                    metric_value(specification, palette, mono),
                    None,
                    palette,
                ),
            ),
            (
                CODE_SHARE,
                metric_cell(
                    t!(i18n, ids::PASSIVE_CALCULATOR_RKM_CODE),
                    metric_value(self.code().to_string(), palette, mono),
                    None,
                    palette,
                ),
            ),
            (
                CODE_SHARE,
                metric_cell(
                    t!(i18n, ids::PASSIVE_CALCULATOR_PRODUCTION_DATE),
                    date_value,
                    None,
                    palette,
                ),
            ),
        ];
        let per_row = if width < NARROW_WIDTH { 2 } else { 3 };
        metric_strip(cells, per_row, palette)
    }
}
