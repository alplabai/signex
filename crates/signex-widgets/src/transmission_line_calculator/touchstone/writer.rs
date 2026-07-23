use std::{fs, path::Path};

use crate::transmission_line_calculator::{
    Complex, NoisePoint, SParameterBlock, SParameterKind, SParameterPoint, ScalarUnit, SolveError,
};

use super::TouchstoneFormat;

pub fn serialize_touchstone(
    block: &SParameterBlock,
    format: TouchstoneFormat,
) -> Result<String, SolveError> {
    let kind = block.kind();
    let port_count = port_count(kind);
    let frequency_unit = touchstone_frequency_unit_name(block.source_frequency_unit)?;
    let frequency_multiplier = block.source_frequency_unit.multiplier();
    let references = effective_port_references(block, port_count)?;
    let point_values = block.points();
    let mut points = point_values.iter().collect::<Vec<_>>();
    points.sort_by(|left, right| left.frequency_hz.total_cmp(&right.frequency_hz));
    validate_points(&points, kind, format)?;

    let noise_values = block.noise();
    let mut noise = noise_values.iter().collect::<Vec<_>>();
    noise.sort_by(|left, right| left.frequency_hz.total_cmp(&right.frequency_hz));
    validate_noise(&noise, kind)?;

    let mut output = String::new();
    output.push_str("! Touchstone 2.1 file written by Signex\n");
    output.push_str("[Version] 2.1\n");
    output.push_str(&format!(
        "# {frequency_unit} S {} R {}\n",
        touchstone_format_name(format),
        format_touchstone_number(references[0])
    ));
    output.push_str(&format!("[Number of Ports] {port_count}\n"));
    if kind == SParameterKind::S2P {
        output.push_str("[Two-Port Data Order] 21_12\n");
    }
    output.push_str(&format!("[Number of Frequencies] {}\n", points.len()));
    if !noise.is_empty() {
        output.push_str(&format!("[Number of Noise Frequencies] {}\n", noise.len()));
    }
    output.push_str("[Reference]");
    for reference in &references {
        output.push(' ');
        output.push_str(&format_touchstone_number(*reference));
    }
    output.push('\n');
    output.push_str("[Matrix Format] Full\n");
    output.push_str("[Network Data]\n");

    for point in points {
        let mut values = vec![format_touchstone_number(
            point.frequency_hz / frequency_multiplier,
        )];
        append_formatted_pair(&mut values, point.s11, format)?;
        if kind == SParameterKind::S2P {
            append_formatted_pair(&mut values, point.s21.unwrap(), format)?;
            append_formatted_pair(&mut values, point.s12.unwrap(), format)?;
            append_formatted_pair(&mut values, point.s22.unwrap(), format)?;
        }
        output.push_str(&values.join(" "));
        output.push('\n');
    }

    if !noise.is_empty() {
        output.push_str("[Noise Data]\n");
        for point in noise {
            let values = [
                point.frequency_hz / frequency_multiplier,
                point.fmin_db,
                point.optimum_gamma.magnitude(),
                point.optimum_gamma.phase_degrees(),
                point.rn_ohm,
            ]
            .map(format_touchstone_number);
            output.push_str(&values.join(" "));
            output.push('\n');
        }
    }
    output.push_str("[End]\n");
    Ok(output)
}

pub fn write_touchstone(
    path: impl AsRef<Path>,
    block: &SParameterBlock,
    format: TouchstoneFormat,
) -> Result<(), SolveError> {
    let path = path.as_ref();
    let raw = serialize_touchstone(block, format)?;
    fs::write(path, raw).map_err(|error| SolveError::TouchstoneWriteFailed {
        reason: format!("{}: {error}", path.display()),
    })
}

fn port_count(kind: SParameterKind) -> usize {
    match kind {
        SParameterKind::S1P => 1,
        SParameterKind::S2P => 2,
    }
}

fn effective_port_references(
    block: &SParameterBlock,
    expected_port_count: usize,
) -> Result<Vec<f64>, SolveError> {
    let reference_impedance_ohm = block.reference_impedance_ohm();
    validate_positive_finite(reference_impedance_ohm, "reference impedance")?;
    let port_references = block.port_reference_impedances_ohm();
    let references = if port_references.is_empty() {
        vec![reference_impedance_ohm; expected_port_count]
    } else if port_references.len() == expected_port_count {
        port_references
    } else {
        return Err(write_error(format!(
            "expected {expected_port_count} port reference impedances"
        )));
    };
    for reference in &references {
        validate_positive_finite(*reference, "port reference impedance")?;
    }
    if references[0] != reference_impedance_ohm {
        return Err(write_error(
            "the primary and port 1 reference impedances disagree",
        ));
    }
    Ok(references)
}

fn validate_points(
    points: &[&SParameterPoint],
    kind: SParameterKind,
    format: TouchstoneFormat,
) -> Result<(), SolveError> {
    if points.is_empty() {
        return Err(write_error("at least one network point is required"));
    }
    let mut previous_frequency = None;
    for point in points {
        validate_nonnegative_finite(point.frequency_hz, "network frequency")?;
        if previous_frequency == Some(point.frequency_hz) {
            return Err(write_error("network frequencies must be unique"));
        }
        previous_frequency = Some(point.frequency_hz);
        validate_complex(point.s11, "S11", format)?;
        match kind {
            SParameterKind::S1P => {
                if point.s21.is_some() || point.s12.is_some() || point.s22.is_some() {
                    return Err(write_error("one-port points must only contain S11"));
                }
            }
            SParameterKind::S2P => {
                validate_complex(
                    point.s21.ok_or_else(|| write_error("missing S21"))?,
                    "S21",
                    format,
                )?;
                validate_complex(
                    point.s12.ok_or_else(|| write_error("missing S12"))?,
                    "S12",
                    format,
                )?;
                validate_complex(
                    point.s22.ok_or_else(|| write_error("missing S22"))?,
                    "S22",
                    format,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_noise(noise: &[&NoisePoint], kind: SParameterKind) -> Result<(), SolveError> {
    if !noise.is_empty() && kind != SParameterKind::S2P {
        return Err(write_error(
            "noise data is only valid for two-port documents",
        ));
    }
    let mut previous_frequency = None;
    for point in noise {
        validate_nonnegative_finite(point.frequency_hz, "noise frequency")?;
        if previous_frequency == Some(point.frequency_hz) {
            return Err(write_error("noise frequencies must be unique"));
        }
        previous_frequency = Some(point.frequency_hz);
        validate_finite(point.fmin_db, "minimum noise figure")?;
        validate_complex(
            point.optimum_gamma,
            "optimum reflection coefficient",
            TouchstoneFormat::MagnitudeAngle,
        )?;
        validate_nonnegative_finite(point.rn_ohm, "effective noise resistance")?;
    }
    Ok(())
}

fn validate_complex(
    value: Complex,
    name: &str,
    format: TouchstoneFormat,
) -> Result<(), SolveError> {
    validate_finite(value.re, name)?;
    validate_finite(value.im, name)?;
    if format == TouchstoneFormat::DecibelAngle && value.magnitude() == 0.0 {
        return Err(write_error(format!(
            "{name} is zero and cannot be represented exactly in DB format"
        )));
    }
    Ok(())
}

fn append_formatted_pair(
    output: &mut Vec<String>,
    value: Complex,
    format: TouchstoneFormat,
) -> Result<(), SolveError> {
    validate_complex(value, "network value", format)?;
    let (first, second) = match format {
        TouchstoneFormat::RealImaginary => (value.re, value.im),
        TouchstoneFormat::MagnitudeAngle => (value.magnitude(), value.phase_degrees()),
        TouchstoneFormat::DecibelAngle => (20.0 * value.magnitude().log10(), value.phase_degrees()),
    };
    output.push(format_touchstone_number(first));
    output.push(format_touchstone_number(second));
    Ok(())
}

fn touchstone_format_name(format: TouchstoneFormat) -> &'static str {
    match format {
        TouchstoneFormat::RealImaginary => "RI",
        TouchstoneFormat::MagnitudeAngle => "MA",
        TouchstoneFormat::DecibelAngle => "DB",
    }
}

fn touchstone_frequency_unit_name(unit: ScalarUnit) -> Result<&'static str, SolveError> {
    match unit {
        ScalarUnit::Hertz => Ok("Hz"),
        ScalarUnit::KiloHertz => Ok("kHz"),
        ScalarUnit::MegaHertz => Ok("MHz"),
        ScalarUnit::GigaHertz => Ok("GHz"),
        _ => Err(write_error(
            "Touchstone supports Hz, kHz, MHz, or GHz frequency units",
        )),
    }
}

fn format_touchstone_number(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else {
        value.to_string()
    }
}

fn validate_finite(value: f64, name: &str) -> Result<(), SolveError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(write_error(format!("{name} must be finite")))
    }
}

fn validate_positive_finite(value: f64, name: &str) -> Result<(), SolveError> {
    validate_finite(value, name)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(write_error(format!("{name} must be positive")))
    }
}

fn validate_nonnegative_finite(value: f64, name: &str) -> Result<(), SolveError> {
    validate_finite(value, name)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(write_error(format!("{name} must not be negative")))
    }
}

fn write_error(reason: impl Into<String>) -> SolveError {
    SolveError::TouchstoneWriteFailed {
        reason: reason.into(),
    }
}
