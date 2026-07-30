use chrono::{Datelike, Local};
use iced::widget::{button, column, container, pick_list, row, text};
use iced::{Alignment, Background, Border, Element, Length};
use signex_types::theme::ThemeTokens;

use super::color_code::ComponentColorCode;
use super::color_code_view::color_code_representations;
use super::domain::{
    ComponentKind, ESeries, PreferredComponent, PreferredNumber, SiPrefix, Tolerance,
};
use super::network::format_value;
use super::production_date_code::{ProductionDateCode, ProductionDateCycle, ProductionMonth};
use super::rkm_code::{RatedPower, RkmCode, TemperatureCoefficient};

#[derive(Debug, Clone)]
pub enum RkmEncoderMessage {
    KindChanged(ComponentKind),
    ValueChanged(PreferredNumber),
    PrefixChanged(SiPrefix),
    ToleranceChanged(Tolerance),
    TemperatureCoefficientChanged(TemperatureCoefficient),
    RatedPowerChanged(RatedPower),
    ProductionDateCycleChanged(ProductionDateCycle),
    ProductionYearChanged(u16),
    ProductionMonthChanged(ProductionMonth),
    ClearResistorOptionalFields,
    ClearProductionDate,
}

#[derive(Debug, Clone)]
pub struct RkmEncoder {
    pub kind: ComponentKind,
    pub value: PreferredNumber,
    pub prefix: SiPrefix,
    pub tolerance: Tolerance,
    pub temperature_coefficient: Option<TemperatureCoefficient>,
    pub rated_power: Option<RatedPower>,
    pub production_date_cycle: Option<ProductionDateCycle>,
    pub production_year: Option<u16>,
    pub production_month: Option<ProductionMonth>,
    value_options: Vec<PreferredNumber>,
    production_year_options: Vec<u16>,
}

impl Default for RkmEncoder {
    fn default() -> Self {
        let current_year =
            u16::try_from(Local::now().year()).expect("the current calendar year fits into a u16");
        let value_options = ESeries::E192.preferred_numbers().collect::<Vec<_>>();
        let value = value_options
            .iter()
            .copied()
            .find(|number| number.significand == 470)
            .expect("E192 contains 4.70");
        Self {
            kind: ComponentKind::Resistor,
            value,
            prefix: SiPrefix::Kilo,
            tolerance: Tolerance::Percent1,
            temperature_coefficient: None,
            rated_power: None,
            production_date_cycle: None,
            production_year: None,
            production_month: None,
            value_options,
            production_year_options: production_year_options(current_year),
        }
    }
}

impl RkmEncoder {
    pub fn update(&mut self, message: RkmEncoderMessage) {
        match message {
            RkmEncoderMessage::KindChanged(kind) => {
                self.kind = kind;
                self.prefix = match kind {
                    ComponentKind::Resistor => SiPrefix::Kilo,
                    ComponentKind::Capacitor => SiPrefix::Micro,
                    ComponentKind::Inductor => SiPrefix::Micro,
                };
                if kind != ComponentKind::Resistor {
                    self.temperature_coefficient = None;
                    self.rated_power = None;
                }
            }
            RkmEncoderMessage::ValueChanged(value) => self.value = value,
            RkmEncoderMessage::PrefixChanged(prefix) => self.prefix = prefix,
            RkmEncoderMessage::ToleranceChanged(tolerance) => self.tolerance = tolerance,
            RkmEncoderMessage::TemperatureCoefficientChanged(temperature_coefficient) => {
                self.temperature_coefficient = Some(temperature_coefficient);
            }
            RkmEncoderMessage::RatedPowerChanged(rated_power) => {
                self.rated_power = Some(rated_power);
            }
            RkmEncoderMessage::ProductionDateCycleChanged(cycle) => {
                self.production_date_cycle = Some(cycle);
            }
            RkmEncoderMessage::ProductionYearChanged(year) => {
                self.production_year = Some(year);
            }
            RkmEncoderMessage::ProductionMonthChanged(month) => {
                self.production_month = Some(month);
            }
            RkmEncoderMessage::ClearResistorOptionalFields => {
                self.temperature_coefficient = None;
                self.rated_power = None;
            }
            RkmEncoderMessage::ClearProductionDate => {
                self.production_date_cycle = None;
                self.production_year = None;
                self.production_month = None;
            }
        }
    }

    pub fn code(&self) -> RkmCode {
        let component = self.component();
        let mut code = RkmCode::for_component(self.kind, component, self.tolerance);
        if let Some(temperature_coefficient) = self.temperature_coefficient {
            code = code.with_temperature_coefficient(temperature_coefficient);
        }
        if let Some(rated_power) = self.rated_power {
            code = code.with_rated_power(rated_power);
        }
        code
    }

    pub fn production_date_code(&self) -> Option<ProductionDateCode> {
        Some(ProductionDateCode::new(
            self.production_date_cycle?,
            self.production_year?,
            self.production_month?,
        ))
    }

    pub fn production_year_options(&self) -> &[u16] {
        &self.production_year_options
    }

    pub fn value_options(&self) -> &[PreferredNumber] {
        &self.value_options
    }

    pub fn view<'a>(&'a self, tokens: &'a ThemeTokens) -> Element<'a, RkmEncoderMessage> {
        let inputs = row![
            labeled(
                "Component",
                pick_list(
                    ComponentKind::ALL,
                    Some(self.kind),
                    RkmEncoderMessage::KindChanged,
                )
                .width(125),
            ),
            labeled(
                "Value",
                pick_list(
                    self.value_options.as_slice(),
                    Some(self.value),
                    RkmEncoderMessage::ValueChanged,
                )
                .width(130),
            ),
            labeled(
                "Unit",
                pick_list(
                    SiPrefix::for_kind(self.kind),
                    Some(self.prefix),
                    RkmEncoderMessage::PrefixChanged,
                )
                .width(100),
            ),
            text(self.prefix.unit(self.kind))
                .size(14)
                .color(token_color(tokens.text_secondary)),
            labeled(
                "Tolerance",
                pick_list(
                    Tolerance::ALL,
                    Some(self.tolerance),
                    RkmEncoderMessage::ToleranceChanged,
                )
                .width(125),
            ),
        ]
        .spacing(12)
        .align_y(Alignment::End);

        let mut content = column![inputs].spacing(14);
        if self.kind == ComponentKind::Resistor {
            let optional_fields = row![
                labeled(
                    "Temperature coefficient",
                    pick_list(
                        TemperatureCoefficient::ALL,
                        self.temperature_coefficient,
                        RkmEncoderMessage::TemperatureCoefficientChanged,
                    )
                    .placeholder("Not specified")
                    .width(165),
                ),
                labeled(
                    "Rated power",
                    pick_list(
                        RatedPower::ALL,
                        self.rated_power,
                        RkmEncoderMessage::RatedPowerChanged,
                    )
                    .placeholder("Not specified")
                    .width(135),
                ),
                button(text("Clear optional fields"))
                    .padding([9, 14])
                    .on_press(RkmEncoderMessage::ClearResistorOptionalFields),
            ]
            .spacing(12)
            .align_y(Alignment::End);
            content = content.push(optional_fields);
        }

        let production_date_fields = row![
            labeled(
                "Production date cycle",
                pick_list(
                    ProductionDateCycle::ALL,
                    self.production_date_cycle,
                    RkmEncoderMessage::ProductionDateCycleChanged,
                )
                .placeholder("Not specified")
                .width(150),
            ),
            labeled(
                "Production year",
                pick_list(
                    self.production_year_options.as_slice(),
                    self.production_year,
                    RkmEncoderMessage::ProductionYearChanged,
                )
                .placeholder("Not specified")
                .width(130),
            ),
            labeled(
                "Production month",
                pick_list(
                    ProductionMonth::ALL,
                    self.production_month,
                    RkmEncoderMessage::ProductionMonthChanged,
                )
                .placeholder("Not specified")
                .width(145),
            ),
            button(text("Clear production date"))
                .padding([9, 14])
                .on_press(RkmEncoderMessage::ClearProductionDate),
        ]
        .spacing(12)
        .align_y(Alignment::End);
        content = content.push(production_date_fields);

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
        let color_codes = color_code_representations(
            ComponentColorCode::representations_for_kind_with_temperature_coefficient(
                self.kind,
                component,
                self.tolerance,
                self.temperature_coefficient,
            ),
            self.kind,
            tokens,
        );
        let mut output = column![
            metric("Specification", specification, tokens),
            metric("RKM code", self.code().to_string(), tokens),
        ]
        .spacing(10);
        if let Some(production_date_code) = self.production_date_code() {
            output = output.push(metric(
                "Production date code",
                production_date_code.to_string(),
                tokens,
            ));
        } else if self.production_date_cycle.is_some()
            || self.production_year.is_some()
            || self.production_month.is_some()
        {
            output = output.push(
                text("Select a production-date cycle, year, and month to generate the code.")
                    .size(12)
                    .color(token_color(tokens.text_secondary)),
            );
        }
        output = output.push(color_codes);
        let output: Element<'a, RkmEncoderMessage> = container(output)
            .padding(14)
            .width(Length::Fill)
            .style(move |_theme| panel_style(tokens))
            .into();
        content = content.push(output);

        content.into()
    }

    fn component(&self) -> PreferredComponent {
        PreferredComponent {
            number: self.value,
            decade: self.prefix.exponent(),
        }
    }
}

fn production_year_options(current_year: u16) -> Vec<u16> {
    const FIRST_DOCUMENTED_YEAR: u16 = 1970;
    if current_year < FIRST_DOCUMENTED_YEAR {
        return vec![current_year];
    }
    (FIRST_DOCUMENTED_YEAR..=current_year).rev().collect()
}

fn labeled<'a, M: 'a>(label: &'a str, control: impl Into<Element<'a, M>>) -> Element<'a, M> {
    column![text(label).size(12), control.into()]
        .spacing(4)
        .into()
}

fn metric<'a>(
    label: &'a str,
    value: String,
    tokens: &'a ThemeTokens,
) -> Element<'a, RkmEncoderMessage> {
    column![
        text(label)
            .size(11)
            .color(token_color(tokens.text_secondary)),
        text(value).size(16).color(token_color(tokens.text)),
    ]
    .spacing(2)
    .into()
}

fn token_color(color: signex_types::theme::Color) -> iced::Color {
    iced::Color::from_rgba8(color.r, color.g, color.b, f32::from(color.a) / 255.0)
}

fn panel_style(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(Background::Color(token_color(tokens.panel_bg))),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: token_color(tokens.border),
        },
        ..container::Style::default()
    }
}
