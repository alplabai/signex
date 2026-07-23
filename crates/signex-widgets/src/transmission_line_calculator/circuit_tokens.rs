use crate::transmission_line_calculator::*;
use std::collections::BTreeMap;

/// Serializes circuit tokens to its compact textual form.
pub(crate) fn serialize_circuit_tokens(circuit: &[SmithChartElement]) -> String {
    circuit
        .iter()
        .map(serialize_smith_chart_element_token)
        .collect::<Vec<_>>()
        .join("__")
}

/// Splits circuit tokens into its constituent values.
pub(crate) fn split_circuit_tokens(value: &str) -> Vec<&str> {
    if value.trim().is_empty() {
        return Vec::new();
    }

    let mut rows = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while let Some(relative) = value[index..].find("__") {
        let delimiter = index + relative;
        let next = delimiter + 2;
        if starts_with_smith_chart_element_kind(&value[next..]) {
            let row = &value[start..delimiter];
            if !row.trim().is_empty() {
                rows.push(row);
            }
            start = next;
        }
        index = delimiter + 1;
    }
    let row = &value[start..];
    if !row.trim().is_empty() {
        rows.push(row);
    }
    rows
}

/// Serializes smith chart element token to its compact textual form.
fn serialize_smith_chart_element_token(element: &SmithChartElement) -> String {
    match element {
        SmithChartElement::BlackBox {
            impedance,
            tolerance_percent,
        } => token(
            "blackBox",
            [
                format_number(impedance.re),
                format_number(impedance.im),
                format_optional(*tolerance_percent),
            ],
        ),
        SmithChartElement::LoadTermination {
            impedance,
            tolerance_percent,
        } => token(
            "loadTerm",
            [
                format_number(impedance.re),
                format_number(impedance.im),
                format_optional(*tolerance_percent),
            ],
        ),
        SmithChartElement::SeriesCapacitor {
            capacitance_f,
            esr_ohm,
            esl_h,
            tolerance_percent,
        } => {
            let (value, unit) = format_token_capacitance(*capacitance_f);
            token(
                "seriesCap",
                [
                    value,
                    unit,
                    format_optional(*tolerance_percent),
                    format_number(*esr_ohm),
                    format_number(*esl_h),
                ],
            )
        }
        SmithChartElement::ShuntCapacitor {
            capacitance_f,
            esr_ohm,
            esl_h,
            tolerance_percent,
        } => {
            let (value, unit) = format_token_capacitance(*capacitance_f);
            token(
                "shortedCap",
                [
                    value,
                    unit,
                    format_optional(*tolerance_percent),
                    format_number(*esr_ohm),
                    format_number(*esl_h),
                ],
            )
        }
        SmithChartElement::SeriesInductor {
            inductance_h,
            esr_ohm,
            tolerance_percent,
        } => {
            let (value, unit) = format_token_inductance(*inductance_h);
            token(
                "seriesInd",
                [
                    value,
                    unit,
                    format_optional(*tolerance_percent),
                    format_number(*esr_ohm),
                ],
            )
        }
        SmithChartElement::ShuntInductor {
            inductance_h,
            esr_ohm,
            tolerance_percent,
        } => {
            let (value, unit) = format_token_inductance(*inductance_h);
            token(
                "shortedInd",
                [
                    value,
                    unit,
                    format_optional(*tolerance_percent),
                    format_number(*esr_ohm),
                ],
            )
        }
        SmithChartElement::SeriesResistor {
            resistance_ohm,
            esl_h,
            tolerance_percent,
        } => {
            let (value, unit) = format_token_resistance(*resistance_ohm);
            token(
                "seriesRes",
                [
                    value,
                    unit,
                    format_optional(*tolerance_percent),
                    format_number(*esl_h),
                ],
            )
        }
        SmithChartElement::ShuntResistor {
            resistance_ohm,
            esl_h,
            tolerance_percent,
        } => {
            let (value, unit) = format_token_resistance(*resistance_ohm);
            token(
                "shortedRes",
                [
                    value,
                    unit,
                    format_optional(*tolerance_percent),
                    format_number(*esl_h),
                ],
            )
        }
        SmithChartElement::SeriesParallelRlc {
            resistance_ohm,
            inductance_h,
            capacitance_f,
        } => {
            let (resistance_value, resistance_unit) = format_token_resistance(*resistance_ohm);
            let (inductance_value, inductance_unit) = format_token_inductance(*inductance_h);
            let (capacitance_value, capacitance_unit) = format_token_capacitance(*capacitance_f);
            token(
                "seriesRlc",
                [
                    resistance_value,
                    resistance_unit,
                    inductance_value,
                    inductance_unit,
                    capacitance_value,
                    capacitance_unit,
                ],
            )
        }
        SmithChartElement::Custom {
            points,
            interpolation,
        } => {
            let mut value = BTreeMap::new();
            for point in points {
                value.insert(
                    format_number(point.frequency_hz),
                    CustomPointTokenValue {
                        real: point.impedance.re,
                        imaginary: point.impedance.im,
                    },
                );
            }
            token(
                "custom",
                [
                    match interpolation {
                        CustomInterpolation::SampleAndHold => "sah".to_string(),
                        CustomInterpolation::Linear => "linear".to_string(),
                    },
                    serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string()),
                ],
            )
        }
        SmithChartElement::TransmissionLine {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        } => {
            let (value, unit) = format_token_length(*length_m);
            token(
                "transmissionLine",
                [
                    value,
                    unit,
                    format_optional(*tolerance_percent),
                    format_number(*characteristic_impedance_ohm),
                    format_number(*effective_dielectric),
                ],
            )
        }
        SmithChartElement::OpenStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        } => {
            let (value, unit) = format_token_length(*length_m);
            token(
                "stub",
                [
                    value,
                    unit,
                    format_optional(*tolerance_percent),
                    format_number(*characteristic_impedance_ohm),
                    format_number(*effective_dielectric),
                ],
            )
        }
        SmithChartElement::ShortedStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        } => {
            let (value, unit) = format_token_length(*length_m);
            token(
                "shortedStub",
                [
                    value,
                    unit,
                    format_optional(*tolerance_percent),
                    format_number(*characteristic_impedance_ohm),
                    format_number(*effective_dielectric),
                ],
            )
        }
        SmithChartElement::Transformer {
            model,
            l1_h,
            l2_h,
            coupling_or_turns_ratio,
        } => {
            let (l1_value, l1_unit) = format_token_inductance(*l1_h);
            let (l2_value, l2_unit) = format_token_inductance(*l2_h);
            token(
                "transformer",
                [
                    l1_value,
                    l1_unit,
                    l2_value,
                    l2_unit,
                    format_number(*coupling_or_turns_ratio),
                    match model {
                        TransformerModel::Ideal => "ideal".to_string(),
                        TransformerModel::CoupledInductor => "coupledInductor".to_string(),
                    },
                ],
            )
        }
        SmithChartElement::SParameter(block) => serialize_s_parameter_token(block),
    }
}

/// Returns whether a token starts with a recognized Smith-chart element kind.
fn starts_with_smith_chart_element_kind(value: &str) -> bool {
    let kind = value.split('_').next().unwrap_or_default();
    matches!(
        kind,
        "blackBox"
            | "loadTerm"
            | "seriesCap"
            | "shortedCap"
            | "seriesInd"
            | "shortedInd"
            | "seriesRes"
            | "shortedRes"
            | "seriesRlc"
            | "custom"
            | "customZ"
            | "transmissionLine"
            | "stub"
            | "shortedStub"
            | "transformer"
            | "sparam"
    )
}

/// Serializes s parameter token to its compact textual form.
fn serialize_s_parameter_token(block: &SParameterBlock) -> String {
    let kind = match block.kind {
        SParameterKind::S1P => "s1p",
        SParameterKind::S2P => "s2p",
    };
    let freq_unit = block
        .source_frequency_unit
        .frequency_symbol()
        .unwrap_or("MHz");
    let mut fields = vec![
        "sparam".to_string(),
        kind.to_string(),
        freq_unit.to_string(),
        format_number(block.reference_impedance_ohm),
    ];
    if block.raw.len() > 1000 {
        fields.push("tooLong".to_string());
        return fields.join("_");
    }
    for point in &block.points {
        fields.push(format_number(
            point.frequency_hz / block.source_frequency_unit.multiplier(),
        ));
        fields.push(format_number(point.s11.magnitude()));
        fields.push(format_number(point.s11.phase_degrees()));
        if block.kind == SParameterKind::S2P {
            let s21 = point.s21.unwrap_or(Complex::ZERO);
            let s12 = point.s12.unwrap_or(Complex::ZERO);
            let s22 = point.s22.unwrap_or(Complex::ZERO);
            fields.push(format_number(s21.magnitude()));
            fields.push(format_number(s21.phase_degrees()));
            fields.push(format_number(s12.magnitude()));
            fields.push(format_number(s12.phase_degrees()));
            fields.push(format_number(s22.magnitude()));
            fields.push(format_number(s22.phase_degrees()));
        }
    }
    if block.kind == SParameterKind::S2P && !block.noise.is_empty() {
        fields.push("noise".to_string());
        for point in &block.noise {
            fields.push(format_number(
                point.frequency_hz / block.source_frequency_unit.multiplier(),
            ));
            fields.push(format_number(point.fmin_db));
            fields.push(format_number(point.optimum_gamma.magnitude()));
            fields.push(format_number(point.optimum_gamma.phase_degrees()));
            fields.push(format_number(point.rn_ohm));
        }
    }
    fields.join("_")
}

/// Joins token fields with the compact circuit delimiter.
fn token<const N: usize>(kind: &str, fields: [String; N]) -> String {
    std::iter::once(kind.to_string())
        .chain(fields)
        .collect::<Vec<_>>()
        .join("_")
}

/// Formats token capacitance for display or serialization.
fn format_token_capacitance(value_f: f64) -> (String, String) {
    format_scaled_token_unit(
        value_f,
        &[
            ("F", ScalarUnit::Farad.multiplier()),
            ("mF", ScalarUnit::MilliFarad.multiplier()),
            ("uF", ScalarUnit::MicroFarad.multiplier()),
            ("nF", ScalarUnit::NanoFarad.multiplier()),
            ("pF", ScalarUnit::PicoFarad.multiplier()),
            ("fF", ScalarUnit::FemtoFarad.multiplier()),
        ],
        "F",
    )
}

/// Formats token inductance for display or serialization.
fn format_token_inductance(value_h: f64) -> (String, String) {
    format_scaled_token_unit(
        value_h,
        &[
            ("H", ScalarUnit::Henry.multiplier()),
            ("mH", ScalarUnit::MilliHenry.multiplier()),
            ("uH", ScalarUnit::MicroHenry.multiplier()),
            ("nH", ScalarUnit::NanoHenry.multiplier()),
            ("pH", ScalarUnit::PicoHenry.multiplier()),
            ("fH", ScalarUnit::FemtoHenry.multiplier()),
        ],
        "H",
    )
}

/// Formats token resistance for display or serialization.
fn format_token_resistance(value_ohm: f64) -> (String, String) {
    format_scaled_token_unit(
        value_ohm,
        &[
            ("MΩ", ScalarUnit::MegaOhm.multiplier()),
            ("KΩ", ScalarUnit::KiloOhm.multiplier()),
            ("Ω", ScalarUnit::Ohm.multiplier()),
            ("mΩ", ScalarUnit::MilliOhm.multiplier()),
        ],
        "Ω",
    )
}

/// Formats token length for display or serialization.
fn format_token_length(value_m: f64) -> (String, String) {
    format_scaled_token_unit(
        value_m,
        &[
            ("m", ScalarUnit::Meter.multiplier()),
            ("mm", ScalarUnit::MilliMeter.multiplier()),
            ("um", ScalarUnit::MicroMeter.multiplier()),
        ],
        "m",
    )
}

/// Formats scaled token unit for display or serialization.
fn format_scaled_token_unit(
    value: f64,
    units: &[(&str, f64)],
    zero_unit: &str,
) -> (String, String) {
    if value == 0.0 || !value.is_finite() {
        return (format_number(value), zero_unit.to_string());
    }
    let magnitude = value.abs();
    for (unit, multiplier) in units {
        let scaled = magnitude / multiplier;
        if (1.0..1000.0).contains(&scaled) {
            return (format_number(value / multiplier), (*unit).to_string());
        }
    }
    let (unit, multiplier) = units.last().copied().unwrap_or((zero_unit, 1.0));
    (format_number(value / multiplier), unit.to_string())
}

/// Formats optional for display or serialization.
fn format_optional(value: Option<f64>) -> String {
    value.map(format_number).unwrap_or_else(|| "0".to_string())
}

/// Formats number for display or serialization.
pub(crate) fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let formatted = format!("{value:.15}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

/// Returns whether two numeric strings represent the same finite value.
pub(crate) fn same_number(left: f64, right: f64) -> bool {
    (left - right).abs() <= f64::EPSILON * left.abs().max(right.abs()).max(1.0)
}
