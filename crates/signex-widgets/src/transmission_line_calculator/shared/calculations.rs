use crate::transmission_line_calculator::{
    Complex, CustomPoint, SParameterBlock, SParameterKind, ScalarUnit, parse_touchstone,
};

/// Parses custom points from its textual representation.
pub(in crate::transmission_line_calculator::tool) fn parse_custom_points(
    value: &str,
) -> Result<Vec<CustomPoint>, String> {
    let mut points = Vec::new();
    for entry in value
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let parts = entry.split(',').map(str::trim).collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err("Custom Z(f) entries use MHz,R,X".to_string());
        }
        points.push(CustomPoint {
            frequency_hz: parse_field("custom frequency", parts[0])? * 1.0e6,
            impedance: Complex::new(
                parse_field("custom resistance", parts[1])?,
                parse_field("custom reactance", parts[2])?,
            ),
        });
    }
    if points.is_empty() {
        return Err("Custom Z(f) needs at least one point".to_string());
    }
    Ok(points)
}

/// Parses marker list from its textual representation.
pub(in crate::transmission_line_calculator::tool) fn parse_marker_list(
    value: &str,
) -> Result<Vec<Complex>, String> {
    value
        .split(';')
        .filter(|entry| !entry.trim().is_empty())
        .map(parse_marker_entry)
        .collect()
}

/// Parses marker entry from its textual representation.
pub(in crate::transmission_line_calculator::tool) fn parse_marker_entry(
    entry: &str,
) -> Result<Complex, String> {
    let entry = entry.trim();
    let (mode, value) = entry
        .split_once(':')
        .map(|(mode, value)| (Some(mode.trim().to_ascii_lowercase()), value.trim()))
        .unwrap_or((None, entry));
    let fields = value.split(',').map(str::trim).collect::<Vec<_>>();
    match mode.as_deref() {
        Some("polar") | Some("p") => {
            if fields.len() != 2 {
                return Err("Polar marker entries use polar:magnitude,angleDeg".to_string());
            }
            Ok(Complex::from_polar(
                parse_field("marker magnitude", fields[0])?,
                parse_field("marker angle", fields[1])?,
            ))
        }
        None | Some("rect") | Some("rectangular") | Some("r") => {
            if fields.len() != 2 {
                return Err("Rectangular marker entries use R,X".to_string());
            }
            Ok(Complex::new(
                parse_field("marker resistance", fields[0])?,
                parse_field("marker reactance", fields[1])?,
            ))
        }
        Some(mode) => Err(format!(
            "Unsupported marker mode '{mode}'; use R,X or polar:magnitude,angleDeg"
        )),
    }
}

/// Parses scalar list from its textual representation.
pub(in crate::transmission_line_calculator::tool) fn parse_scalar_list(
    value: &str,
) -> Result<Vec<f64>, String> {
    value
        .split([';', ','])
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| parse_field("overlay value", entry))
        .collect()
}

/// Parses VSWR circle list from its textual representation.
pub(in crate::transmission_line_calculator::tool) fn parse_vswr_circle_list(
    value: &str,
    input_db: bool,
) -> Result<Vec<f64>, String> {
    parse_scalar_list(value).map(|values| {
        values
            .into_iter()
            .map(|value| {
                if input_db {
                    10.0_f64.powf(value / 20.0)
                } else {
                    value
                }
            })
            .map(f64::abs)
            .collect()
    })
}

/// Parses touchstone input from its textual representation.
pub(in crate::transmission_line_calculator::tool) fn parse_touchstone_input(
    value: &str,
) -> Result<SParameterBlock, String> {
    let normalized = value.replace('|', "\n");
    let block = parse_touchstone(&normalized).map_err(|err| err.to_string())?;
    if matches!(block.kind, SParameterKind::S1P | SParameterKind::S2P) {
        Ok(block)
    } else {
        Err("Unsupported Touchstone data".to_string())
    }
}

/// Parses field from its textual representation.
pub(in crate::transmission_line_calculator::tool) fn parse_field(
    label: &str,
    value: &str,
) -> Result<f64, String> {
    sanitize_numeric_input(value)
        .parse::<f64>()
        .map_err(|_| format!("Invalid {label}"))
}

/// Sanitizes numeric input for safe numeric parsing.
pub(in crate::transmission_line_calculator::tool) fn sanitize_numeric_input(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| {
            character.is_ascii_digit() || matches!(character, '.' | '-' | 'e' | 'E')
        })
        .collect()
}

/// Parses optional from its textual representation.
pub(in crate::transmission_line_calculator::tool) fn parse_optional(
    label: &str,
    value: &str,
) -> Result<f64, String> {
    if value.trim().is_empty() {
        Ok(0.0)
    } else {
        parse_field(label, value)
    }
}

/// Parses an optional percentage tolerance, treating blank or zero as absent.
pub(in crate::transmission_line_calculator::tool) fn optional_tolerance(
    label: &str,
    value: &str,
) -> Result<Option<f64>, String> {
    let parsed = parse_optional(label, value)?;
    if parsed > 0.0 {
        Ok(Some(parsed))
    } else {
        Ok(None)
    }
}

/// Formats optional for display or serialization.
pub(in crate::transmission_line_calculator::tool) fn format_optional(value: Option<f64>) -> String {
    value.map(format_number).unwrap_or_else(|| "0".to_string())
}

/// Formats number for display or serialization.
pub(in crate::transmission_line_calculator::tool) fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let formatted = format!("{value:.12}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

/// Computes impedance quality factor as the absolute reactance-to-resistance ratio.
pub(in crate::transmission_line_calculator::tool) fn quality_factor(value: Complex) -> f64 {
    if value.re.abs() > f64::EPSILON {
        (value.im / value.re).abs()
    } else {
        f64::INFINITY
    }
}

/// Formats complex for display or serialization.
pub(in crate::transmission_line_calculator::tool) fn format_complex(
    value: Complex,
    unit: &str,
) -> String {
    let sign = if value.im < 0.0 { "-" } else { "+" };
    let suffix = if unit.is_empty() {
        String::new()
    } else {
        format!(" {unit}")
    };
    format!("{:.4} {} j{:.4}{}", value.re, sign, value.im.abs(), suffix)
}

/// Formats complex and polar for display or serialization.
pub(in crate::transmission_line_calculator::tool) fn format_complex_and_polar(
    value: Complex,
    unit: &str,
) -> String {
    let suffix = if unit.is_empty() {
        String::new()
    } else {
        format!(" {unit}")
    };
    format!(
        "{}; {:.4} ∠ {:.2}°{}",
        format_complex(value, unit),
        value.magnitude(),
        value.phase_degrees(),
        suffix,
    )
}

/// Formats db for display or serialization.
pub(in crate::transmission_line_calculator::tool) fn format_db(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.2} dB")
    } else {
        "infinite".to_string()
    }
}

/// Formats finite for display or serialization.
pub(in crate::transmission_line_calculator::tool) fn format_finite(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.4}")
    } else {
        "infinite".to_string()
    }
}

/// Formats frequency for display or serialization.
pub(in crate::transmission_line_calculator::tool) fn format_frequency(
    frequency_hz: f64,
    unit: ScalarUnit,
) -> String {
    let symbol = unit.frequency_symbol().unwrap_or("MHz");
    format!(
        "{} {symbol}",
        format_number(frequency_hz / unit.multiplier())
    )
}
