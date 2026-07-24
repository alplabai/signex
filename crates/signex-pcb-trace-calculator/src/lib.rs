//! IPC-2221 PCB conductor sizing calculations and interactive Signex tool.

pub mod tool;

use std::fmt;

const IPC_TEMPERATURE_EXPONENT: f64 = 0.44;
const IPC_AREA_EXPONENT: f64 = 0.725;
const INTERNAL_LAYER_COEFFICIENT: f64 = 0.024;
const EXTERNAL_LAYER_COEFFICIENT: f64 = 0.048;
const SQUARE_MIL_TO_SQUARE_CM: f64 = 6.4516e-6;
const COPPER_RESISTIVITY_AT_25_C_OHM_CM: f64 = 17.0e-7;
const COPPER_TEMPERATURE_COEFFICIENT_PER_C: f64 = 0.0039;
const OUNCE_COPPER_THICKNESS_CM: f64 = 0.0035;
const MAX_GRAPH_CURRENT_AMPERE: f64 = 35.0;
const MIN_GRAPH_TEMPERATURE_RISE_C: f64 = 10.0;
const MAX_GRAPH_TEMPERATURE_RISE_C: f64 = 100.0;
const MIN_GRAPH_COPPER_WEIGHT_OUNCE: f64 = 0.5;
const MAX_GRAPH_COPPER_WEIGHT_OUNCE: f64 = 3.0;
const MAX_GRAPH_WIDTH_CM: f64 = 0.4 * 2.54;
const ABSOLUTE_ZERO_C: f64 = -273.15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Internal,
    External,
}

impl fmt::Display for LayerKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Internal => "internal",
            Self::External => "external",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceInput {
    pub current_ampere: f64,
    pub copper_thickness_cm: f64,
    pub temperature_rise_celsius: f64,
    pub ambient_temperature_celsius: f64,
    pub trace_length_cm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerResult {
    pub area_square_mil: f64,
    pub area_square_cm: f64,
    pub width_cm: f64,
    pub resistance_ohm: f64,
    pub voltage_drop_volt: f64,
    pub power_loss_watt: f64,
}

impl LayerResult {
    #[must_use]
    pub fn width(self, unit: WidthUnit) -> f64 {
        unit.from_cm(self.width_cm)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceCalculation {
    pub internal: LayerResult,
    pub external: LayerResult,
    pub warnings: Vec<RangeWarning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputField {
    Current,
    CopperThickness,
    TemperatureRise,
    AmbientTemperature,
    TraceLength,
}

impl fmt::Display for InputField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Current => "current",
            Self::CopperThickness => "copper thickness",
            Self::TemperatureRise => "temperature rise",
            Self::AmbientTemperature => "ambient temperature",
            Self::TraceLength => "trace length",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CalculationError {
    #[error("{0} must be a finite number")]
    NonFinite(InputField),
    #[error("{0} must be greater than zero")]
    NonPositive(InputField),
    #[error("ambient or conductor temperature is below absolute zero")]
    BelowAbsoluteZero,
    #[error("temperature produces a non-positive copper resistivity")]
    NonPositiveCopperResistivity,
    #[error("calculation result is outside the supported numeric range")]
    ResultOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeWarning {
    CurrentAboveGraphRange,
    TemperatureRiseOutsideGraphRange,
    CopperThicknessOutsideGraphRange,
    WidthAboveGraphRange(LayerKind),
}

impl fmt::Display for RangeWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CurrentAboveGraphRange => "Current is above 35 A.",
            Self::TemperatureRiseOutsideGraphRange => "Temperature rise is outside 10-100 deg C.",
            Self::CopperThicknessOutsideGraphRange => "Copper thickness is outside 0.5-3 oz/ft^2.",
            Self::WidthAboveGraphRange(LayerKind::Internal) => {
                "Internal trace width is above 0.4 inch."
            }
            Self::WidthAboveGraphRange(LayerKind::External) => {
                "External trace width is above 0.4 inch."
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThicknessUnit {
    #[default]
    OuncePerSquareFoot,
    Mil,
    Millimeter,
    Micrometer,
}

impl ThicknessUnit {
    pub const ALL: [Self; 4] = [
        Self::OuncePerSquareFoot,
        Self::Mil,
        Self::Millimeter,
        Self::Micrometer,
    ];

    #[must_use]
    pub fn to_cm(self, value: f64) -> f64 {
        value
            * match self {
                Self::OuncePerSquareFoot => OUNCE_COPPER_THICKNESS_CM,
                Self::Mil => 0.00254,
                Self::Millimeter => 0.1,
                Self::Micrometer => 0.0001,
            }
    }
}

impl fmt::Display for ThicknessUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OuncePerSquareFoot => "oz/ft^2",
            Self::Mil => "mil",
            Self::Millimeter => "mm",
            Self::Micrometer => "um",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WidthUnit {
    #[default]
    Mil,
    Millimeter,
    Micrometer,
}

impl WidthUnit {
    pub const ALL: [Self; 3] = [Self::Mil, Self::Millimeter, Self::Micrometer];

    #[must_use]
    pub fn from_cm(self, value: f64) -> f64 {
        value
            / match self {
                Self::Mil => 0.00254,
                Self::Millimeter => 0.1,
                Self::Micrometer => 0.0001,
            }
    }
}

impl fmt::Display for WidthUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Mil => "mil",
            Self::Millimeter => "mm",
            Self::Micrometer => "um",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LengthUnit {
    #[default]
    Inch,
    Foot,
    Mil,
    Millimeter,
    Micrometer,
    Centimeter,
    Meter,
}

impl LengthUnit {
    pub const ALL: [Self; 7] = [
        Self::Inch,
        Self::Foot,
        Self::Mil,
        Self::Millimeter,
        Self::Micrometer,
        Self::Centimeter,
        Self::Meter,
    ];

    #[must_use]
    pub fn to_cm(self, value: f64) -> f64 {
        value
            * match self {
                Self::Inch => 2.54,
                Self::Foot => 30.48,
                Self::Mil => 0.00254,
                Self::Millimeter => 0.1,
                Self::Micrometer => 0.0001,
                Self::Centimeter => 1.0,
                Self::Meter => 100.0,
            }
    }
}

impl fmt::Display for LengthUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Inch => "inch",
            Self::Foot => "feet",
            Self::Mil => "mil",
            Self::Millimeter => "mm",
            Self::Micrometer => "um",
            Self::Centimeter => "cm",
            Self::Meter => "m",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemperatureRiseUnit {
    #[default]
    Kelvin,
    FahrenheitDegree,
}

impl TemperatureRiseUnit {
    pub const ALL: [Self; 2] = [Self::Kelvin, Self::FahrenheitDegree];

    #[must_use]
    pub fn to_celsius(self, value: f64) -> f64 {
        match self {
            Self::Kelvin => value,
            Self::FahrenheitDegree => value * 5.0 / 9.0,
        }
    }
}

impl fmt::Display for TemperatureRiseUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Kelvin => "K",
            Self::FahrenheitDegree => "deg F",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

impl TemperatureUnit {
    pub const ALL: [Self; 2] = [Self::Celsius, Self::Fahrenheit];

    #[must_use]
    pub fn to_celsius(self, value: f64) -> f64 {
        match self {
            Self::Celsius => value,
            Self::Fahrenheit => (value - 32.0) * 5.0 / 9.0,
        }
    }
}

impl fmt::Display for TemperatureUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Celsius => "deg C",
            Self::Fahrenheit => "deg F",
        })
    }
}

/// Calculates IPC-2221 internal and external conductor requirements.
///
/// # Errors
///
/// Returns an error when an input is non-finite, a required magnitude is not
/// positive, the temperature would produce a non-physical resistance, or an
/// intermediate result exceeds the supported numeric range.
pub fn calculate(input: TraceInput) -> Result<TraceCalculation, CalculationError> {
    validate_input(input)?;

    let internal = calculate_layer(input, INTERNAL_LAYER_COEFFICIENT)?;
    let external = calculate_layer(input, EXTERNAL_LAYER_COEFFICIENT)?;
    let copper_weight_ounce = input.copper_thickness_cm / OUNCE_COPPER_THICKNESS_CM;
    let mut warnings = Vec::new();

    if input.current_ampere > MAX_GRAPH_CURRENT_AMPERE {
        warnings.push(RangeWarning::CurrentAboveGraphRange);
    }
    if !(MIN_GRAPH_TEMPERATURE_RISE_C..=MAX_GRAPH_TEMPERATURE_RISE_C)
        .contains(&input.temperature_rise_celsius)
    {
        warnings.push(RangeWarning::TemperatureRiseOutsideGraphRange);
    }
    if !(MIN_GRAPH_COPPER_WEIGHT_OUNCE..=MAX_GRAPH_COPPER_WEIGHT_OUNCE)
        .contains(&copper_weight_ounce)
    {
        warnings.push(RangeWarning::CopperThicknessOutsideGraphRange);
    }
    if internal.width_cm > MAX_GRAPH_WIDTH_CM {
        warnings.push(RangeWarning::WidthAboveGraphRange(LayerKind::Internal));
    }
    if external.width_cm > MAX_GRAPH_WIDTH_CM {
        warnings.push(RangeWarning::WidthAboveGraphRange(LayerKind::External));
    }

    Ok(TraceCalculation {
        internal,
        external,
        warnings,
    })
}

fn validate_input(input: TraceInput) -> Result<(), CalculationError> {
    for (field, value) in [
        (InputField::Current, input.current_ampere),
        (InputField::CopperThickness, input.copper_thickness_cm),
        (InputField::TemperatureRise, input.temperature_rise_celsius),
        (
            InputField::AmbientTemperature,
            input.ambient_temperature_celsius,
        ),
        (InputField::TraceLength, input.trace_length_cm),
    ] {
        if !value.is_finite() {
            return Err(CalculationError::NonFinite(field));
        }
    }

    for (field, value) in [
        (InputField::Current, input.current_ampere),
        (InputField::CopperThickness, input.copper_thickness_cm),
        (InputField::TemperatureRise, input.temperature_rise_celsius),
        (InputField::TraceLength, input.trace_length_cm),
    ] {
        if value <= 0.0 {
            return Err(CalculationError::NonPositive(field));
        }
    }

    if input.ambient_temperature_celsius < ABSOLUTE_ZERO_C
        || input.ambient_temperature_celsius + input.temperature_rise_celsius < ABSOLUTE_ZERO_C
    {
        return Err(CalculationError::BelowAbsoluteZero);
    }
    let conductor_temperature_celsius =
        input.ambient_temperature_celsius + input.temperature_rise_celsius;
    if 1.0 + COPPER_TEMPERATURE_COEFFICIENT_PER_C * (conductor_temperature_celsius - 25.0) <= 0.0 {
        return Err(CalculationError::NonPositiveCopperResistivity);
    }

    Ok(())
}

fn calculate_layer(input: TraceInput, coefficient: f64) -> Result<LayerResult, CalculationError> {
    // IPC-2221: I = k * delta_T^0.44 * A^0.725, solved here for A in square mils.
    let area_square_mil = (input.current_ampere
        / (coefficient
            * input
                .temperature_rise_celsius
                .powf(IPC_TEMPERATURE_EXPONENT)))
    .powf(1.0 / IPC_AREA_EXPONENT);
    let area_square_cm = area_square_mil * SQUARE_MIL_TO_SQUARE_CM;
    let width_cm = area_square_cm / input.copper_thickness_cm;
    let conductor_temperature_celsius =
        input.ambient_temperature_celsius + input.temperature_rise_celsius;
    let resistance_ohm = COPPER_RESISTIVITY_AT_25_C_OHM_CM * input.trace_length_cm / area_square_cm
        * (1.0 + COPPER_TEMPERATURE_COEFFICIENT_PER_C * (conductor_temperature_celsius - 25.0));

    let result = LayerResult {
        area_square_mil,
        area_square_cm,
        width_cm,
        resistance_ohm,
        voltage_drop_volt: input.current_ampere * resistance_ohm,
        power_loss_watt: input.current_ampere * input.current_ampere * resistance_ohm,
    };
    if [
        result.area_square_mil,
        result.area_square_cm,
        result.width_cm,
        result.resistance_ohm,
        result.voltage_drop_volt,
        result.power_loss_watt,
    ]
    .into_iter()
    .all(f64::is_finite)
    {
        Ok(result)
    } else {
        Err(CalculationError::ResultOutOfRange)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        let tolerance = expected.abs().max(1.0) * 1.0e-10;
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    fn default_input() -> TraceInput {
        TraceInput {
            current_ampere: 10.0,
            copper_thickness_cm: ThicknessUnit::OuncePerSquareFoot.to_cm(2.0),
            temperature_rise_celsius: 10.0,
            ambient_temperature_celsius: 25.0,
            trace_length_cm: LengthUnit::Inch.to_cm(1.0),
        }
    }

    #[test]
    fn default_example_matches_ipc_curve_fit_and_losses() {
        let result = calculate(default_input()).expect("valid example");

        assert_close(result.internal.area_square_mil, 1015.3252432459805);
        assert_close(result.internal.width(WidthUnit::Mil), 368.41801683497005);
        assert_close(result.internal.resistance_ohm, 0.0006848974803031959);
        assert_close(result.internal.voltage_drop_volt, 0.006848974803031959);
        assert_close(result.internal.power_loss_watt, 0.06848974803031959);

        assert_close(result.external.area_square_mil, 390.29357025918654);
        assert_close(result.external.width(WidthUnit::Mil), 141.62080977976197);
        assert_close(result.external.resistance_ohm, 0.001781719592064004);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn unit_conversions_use_physical_dimensions() {
        assert_close(ThicknessUnit::Mil.to_cm(1.0), 0.00254);
        assert_close(ThicknessUnit::Millimeter.to_cm(1.0), 0.1);
        assert_close(ThicknessUnit::Micrometer.to_cm(35.0), 0.0035);
        assert_close(LengthUnit::Foot.to_cm(1.0), 30.48);
        assert_close(LengthUnit::Meter.to_cm(1.0), 100.0);
        assert_close(WidthUnit::Millimeter.from_cm(0.25), 2.5);
        assert_close(TemperatureRiseUnit::FahrenheitDegree.to_celsius(18.0), 10.0);
        assert_close(TemperatureUnit::Fahrenheit.to_celsius(77.0), 25.0);
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        let mut input = default_input();
        input.current_ampere = f64::NAN;
        assert_eq!(
            calculate(input),
            Err(CalculationError::NonFinite(InputField::Current))
        );

        let mut input = default_input();
        input.trace_length_cm = 0.0;
        assert_eq!(
            calculate(input),
            Err(CalculationError::NonPositive(InputField::TraceLength))
        );

        let mut input = default_input();
        input.ambient_temperature_celsius = -274.0;
        assert_eq!(calculate(input), Err(CalculationError::BelowAbsoluteZero));

        let mut input = default_input();
        input.ambient_temperature_celsius = -250.0;
        assert_eq!(
            calculate(input),
            Err(CalculationError::NonPositiveCopperResistivity)
        );

        let mut input = default_input();
        input.current_ampere = f64::MAX;
        assert_eq!(calculate(input), Err(CalculationError::ResultOutOfRange));
    }

    #[test]
    fn graph_range_extrapolations_are_reported() {
        let result = calculate(TraceInput {
            current_ampere: 36.0,
            copper_thickness_cm: ThicknessUnit::OuncePerSquareFoot.to_cm(4.0),
            temperature_rise_celsius: 5.0,
            ..default_input()
        })
        .expect("finite extrapolation");

        assert!(
            result
                .warnings
                .contains(&RangeWarning::CurrentAboveGraphRange)
        );
        assert!(
            result
                .warnings
                .contains(&RangeWarning::TemperatureRiseOutsideGraphRange)
        );
        assert!(
            result
                .warnings
                .contains(&RangeWarning::CopperThicknessOutsideGraphRange)
        );
        assert!(result.warnings.iter().any(|warning| matches!(
            warning,
            RangeWarning::WidthAboveGraphRange(LayerKind::Internal)
        )));
        assert!(result.warnings.iter().any(|warning| matches!(
            warning,
            RangeWarning::WidthAboveGraphRange(LayerKind::External)
        )));
    }
}
