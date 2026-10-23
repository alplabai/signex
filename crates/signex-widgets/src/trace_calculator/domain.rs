//! IPC-2221 conductor sizing: the maths, the units and the validation.
//!
//! This file is deliberately free of any UI import so the same functions
//! can later be called from a routing constraint without pulling in a
//! widget toolkit (issue #303).
//!
//! # Method
//!
//! IPC-2221 (Generic Standard on Printed Board Design), in its section on
//! conductor sizing, gives a curve fit relating the current a conductor
//! can carry to its cross-section and its temperature rise:
//!
//! ```text
//! I = k * dT^0.44 * A^0.725
//! ```
//!
//! with `I` in amperes, `dT` the rise above ambient in degrees Celsius and
//! `A` the cross-section in square mils. `k` is 0.048 for conductors on an
//! outer layer and 0.024 for conductors on an inner layer. Solving for the
//! area gives `A = (I / (k * dT^0.44))^(1/0.725)`, and the trace width is
//! that area divided by the copper thickness.
//!
//! The resistance, voltage drop and power loss are NOT part of IPC-2221.
//! They are the textbook resistor relation `R = rho * L / A` with the
//! conductor's resistivity corrected linearly for temperature,
//! `rho(T) = rho20 * (1 + alpha * (T - 20 degC))`, evaluated at the
//! conductor temperature `T = ambient + rise`; then `V = I * R` and
//! `P = I^2 * R`.
//!
//! # IPC-2152
//!
//! IPC-2152 (Standard for Determining Current-Carrying Capacity in Printed
//! Board Design) supersedes the IPC-2221 chart for current capacity. The
//! results here are the IPC-2221 curve fit only; they are not a
//! substitute for an IPC-2152 analysis.
//!
//! # Sources
//!
//! | Constant | Value | Source |
//! |---|---|---|
//! | Curve-fit exponents | 0.44 (rise), 0.725 (area) | IPC-2221, conductor sizing |
//! | `k` outer / inner layer | 0.048 / 0.024 | IPC-2221, conductor sizing |
//! | Mil | 25.4 micrometres, exactly | international inch = 25.4 mm (1959 agreement), mil = inch / 1000 |
//! | Inch | 0.0254 m, exactly | same |
//! | Copper weight | 1/2, 1, 2, 3 oz/ft^2 = 0.7, 1.4, 2.8, 4.2 mil | IPC-2221 (February 1998), §6.2, Figure 6-4 |
//! | Resistivity `rho20` | 1.7241e-8 ohm*m at 20 degC | International Annealed Copper Standard value for annealed copper (100 % IACS = 58 MS/m), IEC 60028 |
//! | Temperature coefficient `alpha` | 0.00393 per kelvin, referenced to 20 degC | International Annealed Copper Standard value for annealed copper, IEC 60028 |
//! | Reference temperature | 20 degC | the temperature `rho20` and `alpha` are quoted at |
//! | Fahrenheit-degree rise to Celsius | x 5/9 | definition of the scales |
//! | Fahrenheit to Celsius | (F - 32) x 5/9 | definition of the scales |
//! | Absolute zero | -273.15 degC | SI definition of the kelvin |
//! | Outer-layer chart extent | up to 35 A, rise 10-100 degC | IPC-2221 (February 1998), §6.2 and Figure 6-4 |
//! | Inner-layer chart extent | up to 17.5 A, rise 10-45 degC | IPC-2221 (February 1998), §6.2 and Figure 6-4 |
//! | Cross-section / width extent | up to 700 mil^2 / 400 mil | IPC-2221 (February 1998), §6.2 and Figure 6-4 |
//! | Copper-weight extent | 0.5-3 oz/ft^2 | IPC-2221 (February 1998), §6.2 and Figure 6-4 |
//! | Current derating | about 10 % built in; a further 15 % for panels up to 0.8 mm or copper from 108 um | IPC-2221 (February 1998), §6.2 and Figure 6-4 |
//!
//! # Defaults
//!
//! The defaults on the dialog are this crate's own choices: 1 oz/ft^2 is
//! the most common outer-layer copper weight; a 20 degC rise is a
//! customary conservative design margin; a 20 degC ambient is the
//! temperature the resistivity data is quoted at, so the correction
//! starts from the rise alone; 5 A and 50 mm are round numbers that land
//! inside the chart.

use std::fmt;

const TEMPERATURE_RISE_EXPONENT: f64 = 0.44;
const AREA_EXPONENT: f64 = 0.725;
const OUTER_LAYER_K: f64 = 0.048;
const INNER_LAYER_K: f64 = 0.024;

const METRES_PER_MIL: f64 = 25.4e-6;
const METRES_PER_INCH: f64 = 0.0254;
const SQUARE_METRES_PER_SQUARE_MIL: f64 = METRES_PER_MIL * METRES_PER_MIL;
/// Thickness of 1 oz/ft^2 copper foil, in mils (IPC-2221, Figure 6-4).
const MILS_PER_OUNCE_COPPER: f64 = 1.4;

const COPPER_RESISTIVITY_20C_OHM_M: f64 = 1.7241e-8;
const COPPER_ALPHA_PER_K: f64 = 0.00393;
const REFERENCE_TEMPERATURE_C: f64 = 20.0;
const ABSOLUTE_ZERO_C: f64 = -273.15;

/// What one layer kind's current chart covers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerChart {
    pub max_current_a: f64,
    pub min_rise_c: f64,
    pub max_rise_c: f64,
}

/// Extent of the IPC-2221 conductor-sizing charts the curve fit was drawn
/// from. Outside it the formula still evaluates but is an extrapolation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartRange {
    pub outer: LayerChart,
    pub inner: LayerChart,
    pub max_area_mil2: f64,
    pub max_width_mil: f64,
    pub min_copper_oz: f64,
    pub max_copper_oz: f64,
}

impl ChartRange {
    const fn layer(&self, layer: LayerKind) -> LayerChart {
        match layer {
            LayerKind::Inner => self.inner,
            LayerKind::Outer => self.outer,
        }
    }
}

/// The chart extents, see the "Sources" table.
pub const CHART_RANGE: ChartRange = ChartRange {
    outer: LayerChart {
        max_current_a: 35.0,
        min_rise_c: 10.0,
        max_rise_c: 100.0,
    },
    inner: LayerChart {
        max_current_a: 17.5,
        min_rise_c: 10.0,
        max_rise_c: 45.0,
    },
    max_area_mil2: 700.0,
    max_width_mil: 400.0,
    min_copper_oz: 0.5,
    max_copper_oz: 3.0,
};

/// Which side of the board stack the conductor sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Inner,
    Outer,
}

impl LayerKind {
    const fn k(self) -> f64 {
        match self {
            Self::Inner => INNER_LAYER_K,
            Self::Outer => OUTER_LAYER_K,
        }
    }
}

impl fmt::Display for LayerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Inner => "inner",
            Self::Outer => "outer",
        })
    }
}

/// A request, every quantity in SI-based canonical units. Validated by
/// [`calculate`], not on construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceInput {
    pub current_a: f64,
    pub copper_thickness_m: f64,
    pub temperature_rise_c: f64,
    pub ambient_c: f64,
    pub length_m: f64,
}

/// What one layer kind needs to carry the requested current.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerResult {
    pub area_mil2: f64,
    pub width_m: f64,
    pub resistance_ohm: f64,
    pub voltage_drop_v: f64,
    pub power_loss_w: f64,
}

impl LayerResult {
    #[must_use]
    pub fn width(self, unit: WidthUnit) -> f64 {
        unit.from_metres(self.width_m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceCalculation {
    pub inner: LayerResult,
    pub outer: LayerResult,
    pub warnings: Vec<RangeWarning>,
}

/// The five quantities the user supplies; names the field in errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputField {
    Current,
    CopperThickness,
    TemperatureRise,
    AmbientTemperature,
    TraceLength,
}

impl fmt::Display for InputField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Current => "current",
            Self::CopperThickness => "copper thickness",
            Self::TemperatureRise => "temperature rise",
            Self::AmbientTemperature => "ambient temperature",
            Self::TraceLength => "trace length",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalculationError {
    NonFinite(InputField),
    NonPositive(InputField),
    BelowAbsoluteZero,
    NonPositiveResistivity,
    ResultOutOfRange,
}

impl fmt::Display for CalculationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite(field) => write!(f, "The {field} must be a finite number."),
            Self::NonPositive(field) => write!(f, "The {field} must be greater than zero."),
            Self::BelowAbsoluteZero => {
                f.write_str("The ambient temperature is below absolute zero.")
            }
            Self::NonPositiveResistivity => {
                f.write_str("At this temperature the copper resistivity model is not positive.")
            }
            Self::ResultOutOfRange => {
                f.write_str("The result is outside the range this calculator can represent.")
            }
        }
    }
}

impl std::error::Error for CalculationError {}

/// A reason the result sits outside the IPC-2221 charts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RangeWarning {
    CurrentAboveChart(LayerKind),
    RiseOutsideChart(LayerKind),
    AreaAboveChart(LayerKind),
    WidthAboveChart(LayerKind),
    CopperOutsideChart,
}

impl fmt::Display for RangeWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let range = CHART_RANGE;
        match *self {
            Self::CurrentAboveChart(layer) => write!(
                f,
                "The {layer}-layer chart stops at {} A.",
                range.layer(layer).max_current_a
            ),
            Self::RiseOutsideChart(layer) => write!(
                f,
                "The {layer}-layer chart covers a {}-{} \u{b0}C rise only.",
                range.layer(layer).min_rise_c,
                range.layer(layer).max_rise_c
            ),
            Self::AreaAboveChart(layer) => write!(
                f,
                "The {layer}-layer cross-section exceeds the {} mil\u{b2} the chart reaches.",
                range.max_area_mil2
            ),
            Self::WidthAboveChart(layer) => write!(
                f,
                "The {layer}-layer width exceeds the {} mil the chart's width scale reaches.",
                range.max_width_mil
            ),
            Self::CopperOutsideChart => write!(
                f,
                "The chart spans {}-{} oz/ft\u{b2} copper only.",
                range.min_copper_oz, range.max_copper_oz
            ),
        }
    }
}

/// Copper foil thickness units. Order here is the order of the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThicknessUnit {
    Micrometer,
    Millimeter,
    Mil,
    #[default]
    OuncePerSquareFoot,
}

impl ThicknessUnit {
    pub const ALL: [Self; 4] = [
        Self::Micrometer,
        Self::Millimeter,
        Self::Mil,
        Self::OuncePerSquareFoot,
    ];

    #[must_use]
    pub fn to_metres(self, value: f64) -> f64 {
        value
            * match self {
                Self::Micrometer => 1.0e-6,
                Self::Millimeter => 1.0e-3,
                Self::Mil => METRES_PER_MIL,
                Self::OuncePerSquareFoot => MILS_PER_OUNCE_COPPER * METRES_PER_MIL,
            }
    }
}

impl fmt::Display for ThicknessUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Micrometer => "\u{b5}m",
            Self::Millimeter => "mm",
            Self::Mil => "mil",
            Self::OuncePerSquareFoot => "oz/ft\u{b2}",
        })
    }
}

/// Trace width units for the results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WidthUnit {
    #[default]
    Millimeter,
    Micrometer,
    Mil,
}

impl WidthUnit {
    pub const ALL: [Self; 3] = [Self::Millimeter, Self::Micrometer, Self::Mil];

    #[must_use]
    pub fn from_metres(self, metres: f64) -> f64 {
        metres
            / match self {
                Self::Millimeter => 1.0e-3,
                Self::Micrometer => 1.0e-6,
                Self::Mil => METRES_PER_MIL,
            }
    }
}

impl fmt::Display for WidthUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Millimeter => "mm",
            Self::Micrometer => "\u{b5}m",
            Self::Mil => "mil",
        })
    }
}

/// Trace length units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LengthUnit {
    #[default]
    Millimeter,
    Centimeter,
    Meter,
    Mil,
    Inch,
}

impl LengthUnit {
    pub const ALL: [Self; 5] = [
        Self::Millimeter,
        Self::Centimeter,
        Self::Meter,
        Self::Mil,
        Self::Inch,
    ];

    #[must_use]
    pub fn to_metres(self, value: f64) -> f64 {
        value
            * match self {
                Self::Millimeter => 1.0e-3,
                Self::Centimeter => 1.0e-2,
                Self::Meter => 1.0,
                Self::Mil => METRES_PER_MIL,
                Self::Inch => METRES_PER_INCH,
            }
    }
}

impl fmt::Display for LengthUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Millimeter => "mm",
            Self::Centimeter => "cm",
            Self::Meter => "m",
            Self::Mil => "mil",
            Self::Inch => "in",
        })
    }
}

/// Unit of a temperature DIFFERENCE (the rise); no offset applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RiseUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

impl RiseUnit {
    pub const ALL: [Self; 2] = [Self::Celsius, Self::Fahrenheit];

    #[must_use]
    pub fn to_celsius_degrees(self, value: f64) -> f64 {
        match self {
            Self::Celsius => value,
            Self::Fahrenheit => value * 5.0 / 9.0,
        }
    }
}

impl fmt::Display for RiseUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Celsius => "\u{b0}C",
            Self::Fahrenheit => "\u{b0}F",
        })
    }
}

/// Unit of an absolute temperature (the ambient).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmbientUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

impl AmbientUnit {
    pub const ALL: [Self; 2] = [Self::Celsius, Self::Fahrenheit];

    #[must_use]
    pub fn to_celsius(self, value: f64) -> f64 {
        match self {
            Self::Celsius => value,
            Self::Fahrenheit => (value - 32.0) * 5.0 / 9.0,
        }
    }
}

impl fmt::Display for AmbientUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Celsius => "\u{b0}C",
            Self::Fahrenheit => "\u{b0}F",
        })
    }
}

/// Size both layer kinds for the requested current.
///
/// # Errors
///
/// A non-finite or non-positive input, an ambient below absolute zero, a
/// temperature at which the resistivity model goes non-positive, or an
/// intermediate result that overflows `f64`.
pub fn calculate(input: TraceInput) -> Result<TraceCalculation, CalculationError> {
    validate(input)?;
    let inner = size_layer(input, LayerKind::Inner)?;
    let outer = size_layer(input, LayerKind::Outer)?;
    Ok(TraceCalculation {
        warnings: range_warnings(input, inner, outer),
        inner,
        outer,
    })
}

fn range_warnings(input: TraceInput, inner: LayerResult, outer: LayerResult) -> Vec<RangeWarning> {
    let range = CHART_RANGE;
    let copper_oz = input.copper_thickness_m / ThicknessUnit::OuncePerSquareFoot.to_metres(1.0);
    let mut warnings = Vec::new();
    for (layer, result) in [(LayerKind::Inner, inner), (LayerKind::Outer, outer)] {
        let chart = range.layer(layer);
        if input.current_a > chart.max_current_a {
            warnings.push(RangeWarning::CurrentAboveChart(layer));
        }
        if !(chart.min_rise_c..=chart.max_rise_c).contains(&input.temperature_rise_c) {
            warnings.push(RangeWarning::RiseOutsideChart(layer));
        }
        if result.area_mil2 > range.max_area_mil2 {
            warnings.push(RangeWarning::AreaAboveChart(layer));
        }
        if result.width(WidthUnit::Mil) > range.max_width_mil {
            warnings.push(RangeWarning::WidthAboveChart(layer));
        }
    }
    if !(range.min_copper_oz..=range.max_copper_oz).contains(&copper_oz) {
        warnings.push(RangeWarning::CopperOutsideChart);
    }
    warnings
}

fn validate(input: TraceInput) -> Result<(), CalculationError> {
    let fields = [
        (InputField::Current, input.current_a),
        (InputField::CopperThickness, input.copper_thickness_m),
        (InputField::TemperatureRise, input.temperature_rise_c),
        (InputField::AmbientTemperature, input.ambient_c),
        (InputField::TraceLength, input.length_m),
    ];
    if let Some((field, _)) = fields.iter().find(|(_, value)| !value.is_finite()) {
        return Err(CalculationError::NonFinite(*field));
    }
    // The ambient is the one quantity that may legitimately be negative.
    let non_positive = fields
        .iter()
        .filter(|(field, _)| *field != InputField::AmbientTemperature)
        .find(|(_, value)| *value <= 0.0);
    if let Some((field, _)) = non_positive {
        return Err(CalculationError::NonPositive(*field));
    }
    // With a positive rise, an ambient at or above absolute zero already
    // keeps the conductor temperature above it.
    if input.ambient_c < ABSOLUTE_ZERO_C {
        return Err(CalculationError::BelowAbsoluteZero);
    }
    if resistivity_factor(input) <= 0.0 {
        return Err(CalculationError::NonPositiveResistivity);
    }
    Ok(())
}

/// `1 + alpha * (T - 20 degC)` at the conductor temperature.
fn resistivity_factor(input: TraceInput) -> f64 {
    let conductor_c = input.ambient_c + input.temperature_rise_c;
    1.0 + COPPER_ALPHA_PER_K * (conductor_c - REFERENCE_TEMPERATURE_C)
}

fn size_layer(input: TraceInput, layer: LayerKind) -> Result<LayerResult, CalculationError> {
    // IPC-2221: I = k * dT^0.44 * A^0.725, solved for A in square mils.
    let area_mil2 = (input.current_a
        / (layer.k() * input.temperature_rise_c.powf(TEMPERATURE_RISE_EXPONENT)))
    .powf(1.0 / AREA_EXPONENT);
    let area_m2 = area_mil2 * SQUARE_METRES_PER_SQUARE_MIL;
    let resistance_ohm =
        COPPER_RESISTIVITY_20C_OHM_M * input.length_m / area_m2 * resistivity_factor(input);
    let result = LayerResult {
        area_mil2,
        width_m: area_m2 / input.copper_thickness_m,
        resistance_ohm,
        voltage_drop_v: input.current_a * resistance_ohm,
        power_loss_w: input.current_a * input.current_a * resistance_ohm,
    };
    let finite = [
        result.area_mil2,
        result.width_m,
        result.resistance_ohm,
        result.voltage_drop_v,
        result.power_loss_w,
    ]
    .into_iter()
    .all(f64::is_finite);
    if finite {
        Ok(result)
    } else {
        Err(CalculationError::ResultOutOfRange)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        let tolerance = expected.abs().max(1.0e-12) * 1.0e-10;
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    /// 10 A, 10 degC rise, 70 um copper (2 oz class), 1 inch, 25 degC
    /// ambient: the worked IPC-2221 example (about 1015 mil^2 inner,
    /// about 368 mil wide) that was re-derived by hand in review.
    fn worked_example() -> TraceInput {
        TraceInput {
            current_a: 10.0,
            copper_thickness_m: ThicknessUnit::Micrometer.to_metres(70.0),
            temperature_rise_c: 10.0,
            ambient_c: 25.0,
            length_m: LengthUnit::Inch.to_metres(1.0),
        }
    }

    #[test]
    fn worked_example_matches_the_ipc_2221_curve_fit() {
        let result = calculate(worked_example()).expect("valid example");

        assert_close(result.inner.area_mil2, 1015.3252432459805);
        assert_close(result.inner.width(WidthUnit::Mil), 368.41801683497005);
        assert_close(result.outer.area_mil2, 390.29357025918654);
        assert_close(result.outer.width(WidthUnit::Mil), 141.620809779762);
        // 1015 mil^2 is beyond the 700 mil^2 the inner chart reaches.
        assert_eq!(
            result.warnings,
            vec![RangeWarning::AreaAboveChart(LayerKind::Inner)]
        );
    }

    /// The dialog's own defaults: 5 A, 1 oz/ft^2, 20 degC rise, 20 degC
    /// ambient, 50 mm.
    fn dialog_defaults() -> TraceInput {
        TraceInput {
            current_a: 5.0,
            copper_thickness_m: ThicknessUnit::OuncePerSquareFoot.to_metres(1.0),
            temperature_rise_c: 20.0,
            ambient_c: 20.0,
            length_m: LengthUnit::Millimeter.to_metres(50.0),
        }
    }

    #[test]
    fn resistance_follows_the_temperature_corrected_copper_model() {
        let result = calculate(dialog_defaults()).expect("valid defaults");

        // rho20 * L / A * (1 + alpha * (40 - 20)), with the inner area.
        assert_close(result.inner.resistance_ohm, 0.005623777223592189);
        assert_close(result.inner.voltage_drop_v, 0.028118886117960946);
        assert_close(result.inner.power_loss_w, 0.14059443058980473);
        assert_close(result.outer.resistance_ohm, 0.014629918124741499);
        assert_close(result.outer.voltage_drop_v, 0.07314959062370749);
        assert_close(result.outer.power_loss_w, 0.36574795311853747);
    }

    #[test]
    fn one_ounce_copper_is_14_mil_thick() {
        assert_close(
            ThicknessUnit::OuncePerSquareFoot.to_metres(1.0),
            1.4 * 25.4e-6,
        );
        // 2 oz is 2.8 mil: width = area / 2.8 mil, about 362.6 mil.
        let input = TraceInput {
            copper_thickness_m: ThicknessUnit::OuncePerSquareFoot.to_metres(2.0),
            ..worked_example()
        };
        let width = calculate(input).unwrap().inner.width(WidthUnit::Mil);
        assert_close(width, 1015.3252432459805 / 2.8);
    }

    #[test]
    fn unit_conversions_use_physical_dimensions() {
        assert_close(ThicknessUnit::Mil.to_metres(1.0), 25.4e-6);
        assert_close(ThicknessUnit::Millimeter.to_metres(1.0), 1.0e-3);
        assert_close(ThicknessUnit::Micrometer.to_metres(35.0), 35.0e-6);
        assert_close(LengthUnit::Inch.to_metres(1.0), 0.0254);
        assert_close(LengthUnit::Mil.to_metres(1000.0), 0.0254);
        assert_close(LengthUnit::Centimeter.to_metres(2.0), 0.02);
        assert_close(LengthUnit::Meter.to_metres(1.0), 1.0);
        assert_close(WidthUnit::Millimeter.from_metres(0.0025), 2.5);
        assert_close(WidthUnit::Micrometer.from_metres(1.0e-4), 100.0);
        assert_close(RiseUnit::Fahrenheit.to_celsius_degrees(18.0), 10.0);
        assert_close(AmbientUnit::Fahrenheit.to_celsius(77.0), 25.0);
        assert_close(AmbientUnit::Fahrenheit.to_celsius(-40.0), -40.0);
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        let nan_current = TraceInput {
            current_a: f64::NAN,
            ..worked_example()
        };
        assert_eq!(
            calculate(nan_current),
            Err(CalculationError::NonFinite(InputField::Current))
        );

        let zero_length = TraceInput {
            length_m: 0.0,
            ..worked_example()
        };
        assert_eq!(
            calculate(zero_length),
            Err(CalculationError::NonPositive(InputField::TraceLength))
        );

        let below_zero = TraceInput {
            ambient_c: -274.0,
            ..worked_example()
        };
        assert_eq!(
            calculate(below_zero),
            Err(CalculationError::BelowAbsoluteZero)
        );

        // Above absolute zero, but cold enough that 1 + alpha * (T - 20) <= 0.
        let too_cold = TraceInput {
            ambient_c: -250.0,
            ..worked_example()
        };
        assert_eq!(
            calculate(too_cold),
            Err(CalculationError::NonPositiveResistivity)
        );

        let overflow = TraceInput {
            current_a: f64::MAX,
            ..worked_example()
        };
        assert_eq!(calculate(overflow), Err(CalculationError::ResultOutOfRange));
    }

    #[test]
    fn a_negative_ambient_above_the_resistivity_floor_is_accepted() {
        let cold_room = TraceInput {
            ambient_c: -40.0,
            ..worked_example()
        };
        assert!(calculate(cold_room).is_ok());
    }

    fn warnings_for(input: TraceInput) -> Vec<RangeWarning> {
        calculate(input).expect("finite extrapolation").warnings
    }

    #[test]
    fn leaving_the_chart_is_reported_per_cause() {
        let warnings = warnings_for(TraceInput {
            current_a: 36.0,
            copper_thickness_m: ThicknessUnit::OuncePerSquareFoot.to_metres(4.0),
            temperature_rise_c: 5.0,
            ..worked_example()
        });
        for layer in [LayerKind::Inner, LayerKind::Outer] {
            assert!(warnings.contains(&RangeWarning::CurrentAboveChart(layer)));
            assert!(warnings.contains(&RangeWarning::RiseOutsideChart(layer)));
            assert!(warnings.contains(&RangeWarning::AreaAboveChart(layer)));
            assert!(warnings.contains(&RangeWarning::WidthAboveChart(layer)));
        }
        assert!(warnings.contains(&RangeWarning::CopperOutsideChart));
    }

    #[test]
    fn the_inner_chart_stops_at_a_lower_current_than_the_outer() {
        let warnings = warnings_for(TraceInput {
            current_a: 20.0,
            ..worked_example()
        });
        assert!(warnings.contains(&RangeWarning::CurrentAboveChart(LayerKind::Inner)));
        assert!(!warnings.contains(&RangeWarning::CurrentAboveChart(LayerKind::Outer)));
    }

    #[test]
    fn the_inner_chart_covers_a_narrower_rise_span_than_the_outer() {
        let warnings = warnings_for(TraceInput {
            temperature_rise_c: 60.0,
            ..worked_example()
        });
        assert!(warnings.contains(&RangeWarning::RiseOutsideChart(LayerKind::Inner)));
        assert!(!warnings.contains(&RangeWarning::RiseOutsideChart(LayerKind::Outer)));
        // Both charts start at 10 degC.
        let low = warnings_for(TraceInput {
            temperature_rise_c: 5.0,
            ..worked_example()
        });
        assert!(low.contains(&RangeWarning::RiseOutsideChart(LayerKind::Outer)));
    }

    #[test]
    fn a_cross_section_beyond_700_mil_squared_warns_per_layer() {
        // 10 A / 10 degC: 1015 mil^2 inner, 390 mil^2 outer.
        let warnings = warnings_for(worked_example());
        assert!(warnings.contains(&RangeWarning::AreaAboveChart(LayerKind::Inner)));
        assert!(!warnings.contains(&RangeWarning::AreaAboveChart(LayerKind::Outer)));
    }

    #[test]
    fn the_dialog_defaults_stay_inside_the_chart() {
        let result = calculate(dialog_defaults()).expect("defaults are valid");
        assert!(result.warnings.is_empty());
        assert_close(result.inner.area_mil2, 256.2697412447107);
        assert_close(result.outer.resistance_ohm, 0.014629918124741499);
    }

    #[test]
    fn warning_text_names_the_layer_and_the_chart_limit() {
        assert_eq!(
            RangeWarning::CurrentAboveChart(LayerKind::Inner).to_string(),
            "The inner-layer chart stops at 17.5 A."
        );
        assert_eq!(
            RangeWarning::CurrentAboveChart(LayerKind::Outer).to_string(),
            "The outer-layer chart stops at 35 A."
        );
        assert!(
            RangeWarning::WidthAboveChart(LayerKind::Outer)
                .to_string()
                .starts_with("The outer-layer width")
        );
    }
}
