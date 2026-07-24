use iced::widget::{
    Space, button, column, container, pick_list, row, rule, scrollable, text, text_input,
};
use iced::{Alignment, Element, Length};

use crate::{
    LayerResult, LengthUnit, TemperatureRiseUnit, TemperatureUnit, ThicknessUnit, TraceCalculation,
    TraceInput, WidthUnit, calculate,
};

#[derive(Debug, Clone)]
pub enum PcbTraceCalculatorMessage {
    CurrentChanged(String),
    ThicknessChanged(String),
    ThicknessUnitChanged(ThicknessUnit),
    TemperatureRiseChanged(String),
    TemperatureRiseUnitChanged(TemperatureRiseUnit),
    AmbientTemperatureChanged(String),
    AmbientTemperatureUnitChanged(TemperatureUnit),
    TraceLengthChanged(String),
    TraceLengthUnitChanged(LengthUnit),
    InternalWidthUnitChanged(WidthUnit),
    ExternalWidthUnitChanged(WidthUnit),
    Reset,
}

#[derive(Debug, Clone)]
pub struct PcbTraceCalculatorState {
    current: String,
    thickness: String,
    thickness_unit: ThicknessUnit,
    temperature_rise: String,
    temperature_rise_unit: TemperatureRiseUnit,
    ambient_temperature: String,
    ambient_temperature_unit: TemperatureUnit,
    trace_length: String,
    trace_length_unit: LengthUnit,
    internal_width_unit: WidthUnit,
    external_width_unit: WidthUnit,
}

impl Default for PcbTraceCalculatorState {
    fn default() -> Self {
        Self {
            current: "10".to_string(),
            thickness: "2".to_string(),
            thickness_unit: ThicknessUnit::OuncePerSquareFoot,
            temperature_rise: "10".to_string(),
            temperature_rise_unit: TemperatureRiseUnit::Kelvin,
            ambient_temperature: "25".to_string(),
            ambient_temperature_unit: TemperatureUnit::Celsius,
            trace_length: "1".to_string(),
            trace_length_unit: LengthUnit::Inch,
            internal_width_unit: WidthUnit::Mil,
            external_width_unit: WidthUnit::Mil,
        }
    }
}

impl PcbTraceCalculatorState {
    pub fn update(&mut self, message: PcbTraceCalculatorMessage) {
        match message {
            PcbTraceCalculatorMessage::CurrentChanged(value) => self.current = value,
            PcbTraceCalculatorMessage::ThicknessChanged(value) => self.thickness = value,
            PcbTraceCalculatorMessage::ThicknessUnitChanged(unit) => self.thickness_unit = unit,
            PcbTraceCalculatorMessage::TemperatureRiseChanged(value) => {
                self.temperature_rise = value;
            }
            PcbTraceCalculatorMessage::TemperatureRiseUnitChanged(unit) => {
                self.temperature_rise_unit = unit;
            }
            PcbTraceCalculatorMessage::AmbientTemperatureChanged(value) => {
                self.ambient_temperature = value;
            }
            PcbTraceCalculatorMessage::AmbientTemperatureUnitChanged(unit) => {
                self.ambient_temperature_unit = unit;
            }
            PcbTraceCalculatorMessage::TraceLengthChanged(value) => self.trace_length = value,
            PcbTraceCalculatorMessage::TraceLengthUnitChanged(unit) => {
                self.trace_length_unit = unit;
            }
            PcbTraceCalculatorMessage::InternalWidthUnitChanged(unit) => {
                self.internal_width_unit = unit;
            }
            PcbTraceCalculatorMessage::ExternalWidthUnitChanged(unit) => {
                self.external_width_unit = unit;
            }
            PcbTraceCalculatorMessage::Reset => *self = Self::default(),
        }
    }

    fn calculation(&self) -> Result<TraceCalculation, String> {
        let current = parse_input("current", &self.current)?;
        let thickness = parse_input("copper thickness", &self.thickness)?;
        let temperature_rise = parse_input("temperature rise", &self.temperature_rise)?;
        let ambient_temperature = parse_input("ambient temperature", &self.ambient_temperature)?;
        let trace_length = parse_input("trace length", &self.trace_length)?;

        calculate(TraceInput {
            current_ampere: current,
            copper_thickness_cm: self.thickness_unit.to_cm(thickness),
            temperature_rise_celsius: self.temperature_rise_unit.to_celsius(temperature_rise),
            ambient_temperature_celsius: self
                .ambient_temperature_unit
                .to_celsius(ambient_temperature),
            trace_length_cm: self.trace_length_unit.to_cm(trace_length),
        })
        .map_err(|error| error.to_string())
    }
}

pub fn view(state: &PcbTraceCalculatorState) -> Element<'_, PcbTraceCalculatorMessage> {
    let header = row![
        column![
            text("PCB Trace Calculator").size(22),
            text("IPC-2221 conductor sizing").size(12),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
        button(text("Reset")).on_press(PcbTraceCalculatorMessage::Reset),
    ]
    .align_y(Alignment::Center)
    .spacing(12);

    let electrical = column![
        text("Electrical").size(14),
        input_row(
            "Current",
            &state.current,
            "A",
            PcbTraceCalculatorMessage::CurrentChanged,
        ),
        unit_input_row(
            "Copper thickness",
            &state.thickness,
            ThicknessUnit::ALL,
            state.thickness_unit,
            PcbTraceCalculatorMessage::ThicknessChanged,
            PcbTraceCalculatorMessage::ThicknessUnitChanged,
        ),
        unit_input_row(
            "Trace length",
            &state.trace_length,
            LengthUnit::ALL,
            state.trace_length_unit,
            PcbTraceCalculatorMessage::TraceLengthChanged,
            PcbTraceCalculatorMessage::TraceLengthUnitChanged,
        ),
    ]
    .spacing(10)
    .width(Length::Fill);

    let thermal = column![
        text("Thermal").size(14),
        unit_input_row(
            "Temperature rise",
            &state.temperature_rise,
            TemperatureRiseUnit::ALL,
            state.temperature_rise_unit,
            PcbTraceCalculatorMessage::TemperatureRiseChanged,
            PcbTraceCalculatorMessage::TemperatureRiseUnitChanged,
        ),
        unit_input_row(
            "Ambient temperature",
            &state.ambient_temperature,
            TemperatureUnit::ALL,
            state.ambient_temperature_unit,
            PcbTraceCalculatorMessage::AmbientTemperatureChanged,
            PcbTraceCalculatorMessage::AmbientTemperatureUnitChanged,
        ),
    ]
    .spacing(10)
    .width(Length::Fill);

    let inputs = row![electrical, Space::new().width(20), thermal]
        .spacing(8)
        .width(Length::Fill);

    let results = match state.calculation() {
        Ok(calculation) => calculation_view(state, &calculation),
        Err(error) => container(text(error).size(13))
            .padding(12)
            .width(Length::Fill)
            .style(container::rounded_box)
            .into(),
    };

    let body = column![
        header,
        rule::horizontal(1),
        inputs,
        rule::horizontal(1),
        results
    ]
    .spacing(16)
    .padding(20)
    .width(Length::Fill);

    container(scrollable(body))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn calculation_view<'a>(
    state: &PcbTraceCalculatorState,
    calculation: &TraceCalculation,
) -> Element<'a, PcbTraceCalculatorMessage> {
    let panels = row![
        result_panel(
            "Internal layer",
            calculation.internal,
            state.internal_width_unit,
            PcbTraceCalculatorMessage::InternalWidthUnitChanged,
        ),
        result_panel(
            "External layer in air",
            calculation.external,
            state.external_width_unit,
            PcbTraceCalculatorMessage::ExternalWidthUnitChanged,
        ),
    ]
    .spacing(12)
    .width(Length::Fill);

    if calculation.warnings.is_empty() {
        return panels.into();
    }

    let warning_text = calculation
        .warnings
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    column![
        panels,
        container(column![
            text("IPC-2221 graph range").size(13),
            text(format!("{warning_text} Results are extrapolated.")).size(12),
        ])
        .padding(10)
        .width(Length::Fill)
        .style(container::rounded_box),
    ]
    .spacing(12)
    .into()
}

fn result_panel<'a>(
    title: &'static str,
    result: LayerResult,
    width_unit: WidthUnit,
    on_width_unit_change: fn(WidthUnit) -> PcbTraceCalculatorMessage,
) -> Element<'a, PcbTraceCalculatorMessage> {
    container(
        column![
            row![
                text(title).size(14),
                Space::new().width(Length::Fill),
                pick_list(WidthUnit::ALL, Some(width_unit), on_width_unit_change,)
                    .width(Length::Fixed(82.0)),
            ]
            .align_y(Alignment::Center),
            output_row(
                "Required trace width",
                format_number(result.width(width_unit)),
                width_unit.to_string(),
            ),
            output_row(
                "Conductor area",
                format_number(result.area_square_mil),
                "mil^2".to_string(),
            ),
            output_row(
                "Resistance",
                format_number(result.resistance_ohm),
                "Ohm".to_string(),
            ),
            output_row(
                "Voltage drop",
                format_number(result.voltage_drop_volt),
                "V".to_string(),
            ),
            output_row(
                "Power loss",
                format_number(result.power_loss_watt),
                "W".to_string(),
            ),
        ]
        .spacing(9),
    )
    .padding(12)
    .width(Length::Fill)
    .style(container::rounded_box)
    .into()
}

fn output_row(
    label: &'static str,
    value: String,
    unit: String,
) -> Element<'static, PcbTraceCalculatorMessage> {
    row![
        text(label).width(Length::Fill),
        text(value).width(Length::Fixed(92.0)),
        text(unit).width(Length::Fixed(54.0)),
    ]
    .align_y(Alignment::Center)
    .spacing(8)
    .into()
}

fn input_row<'a>(
    label: &'static str,
    value: &'a str,
    unit: &'static str,
    on_change: fn(String) -> PcbTraceCalculatorMessage,
) -> Element<'a, PcbTraceCalculatorMessage> {
    row![
        text(label).width(Length::Fixed(132.0)),
        text_input("", value)
            .on_input(on_change)
            .width(Length::Fill),
        text(unit).width(Length::Fixed(72.0)),
    ]
    .align_y(Alignment::Center)
    .spacing(8)
    .into()
}

fn unit_input_row<'a, U: Copy + PartialEq + ToString + 'a, const N: usize>(
    label: &'static str,
    value: &'a str,
    units: [U; N],
    selected_unit: U,
    on_change: fn(String) -> PcbTraceCalculatorMessage,
    on_unit_change: fn(U) -> PcbTraceCalculatorMessage,
) -> Element<'a, PcbTraceCalculatorMessage> {
    row![
        text(label).width(Length::Fixed(132.0)),
        text_input("", value)
            .on_input(on_change)
            .width(Length::Fill),
        pick_list(units, Some(selected_unit), on_unit_change).width(Length::Fixed(92.0)),
    ]
    .align_y(Alignment::Center)
    .spacing(8)
    .into()
}

fn parse_input(label: &str, value: &str) -> Result<f64, String> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("Enter a numeric value for {label}."))
}

fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }

    let exponent = value.abs().log10().floor() as i32;
    if !(-4..=5).contains(&exponent) {
        return format!("{value:.3e}");
    }

    let decimal_places = (2 - exponent).clamp(0, 8) as usize;
    format!("{value:.decimal_places$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_calculates_and_view_constructs() {
        let state = PcbTraceCalculatorState::default();
        let result = state.calculation().expect("default state is valid");
        assert!(result.warnings.is_empty());
        let _: Element<'_, PcbTraceCalculatorMessage> = view(&state);
    }

    #[test]
    fn reset_restores_defaults() {
        let mut state = PcbTraceCalculatorState::default();
        state.update(PcbTraceCalculatorMessage::CurrentChanged("20".to_string()));
        state.update(PcbTraceCalculatorMessage::Reset);
        assert_eq!(state.current, "10");
    }
}
