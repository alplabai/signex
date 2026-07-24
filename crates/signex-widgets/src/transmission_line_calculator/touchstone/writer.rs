use std::{fs, path::Path};

use rust_rf::{
    Network,
    io::{TouchstoneParameter, touchstone_string},
};

use crate::transmission_line_calculator::{SParameterBlock, ScalarUnit, SolveError};

use super::TouchstoneFormat;

pub fn serialize_touchstone(
    block: &SParameterBlock,
    format: TouchstoneFormat,
) -> Result<String, SolveError> {
    let network = block.network();
    let (frequency_unit, frequency_multiplier) =
        touchstone_frequency_unit(block.source_frequency_unit)?;
    validate_writer_policy(network, format)?;
    let rust_rf_output = touchstone_string(network, TouchstoneParameter::Scattering, format)
        .map_err(|error| {
            write_error(format!(
                "rust-rf could not serialize the Touchstone network: {error}"
            ))
        })?;
    adapt_rust_rf_output(
        &rust_rf_output,
        network,
        frequency_unit,
        frequency_multiplier,
    )
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

fn adapt_rust_rf_output(
    rust_rf_output: &str,
    network: &Network,
    frequency_unit: &str,
    frequency_multiplier: f64,
) -> Result<String, SolveError> {
    let mut output = String::from("! Touchstone 2.1 file written by Signex\n");
    let mut in_network_data = false;
    for line in rust_rf_output.lines() {
        if line == "[Version] 2.0" {
            output.push_str("[Version] 2.1\n");
        } else if line.starts_with("# ") {
            output.push_str(&format!(
                "# {frequency_unit} S {} R {}\n",
                rust_rf_format_name(line)?,
                format_number(network.z0[(0, 0)].re)
            ));
        } else if line.starts_with("[Number of Ports]") {
            output.push_str(line);
            output.push('\n');
            if network.ports() == 2 {
                output.push_str("[Two-Port Data Order] 21_12\n");
            }
        } else if line.starts_with("[Number of Frequencies]") {
            output.push_str(line);
            output.push('\n');
            if let Some(noise) = &network.noise {
                output.push_str(&format!(
                    "[Number of Noise Frequencies] {}\n",
                    noise.frequency.points()
                ));
            }
        } else if line.starts_with("[Reference]") {
            output.push_str("[Reference]");
            for port in 0..network.ports() {
                output.push(' ');
                output.push_str(&format_number(network.z0[(0, port)].re));
            }
            output.push('\n');
            output.push_str("[Matrix Format] Full\n");
        } else if line == "[Network Data]" {
            in_network_data = true;
            output.push_str("[Network Data]\n");
        } else if line == "[End]" {
            in_network_data = false;
            append_noise_data(&mut output, network, frequency_multiplier);
            output.push_str("[End]\n");
        } else if in_network_data {
            output.push_str(&adapt_network_record(
                line,
                network.ports(),
                frequency_multiplier,
            )?);
            output.push('\n');
        } else if !line.starts_with('!') {
            output.push_str(line);
            output.push('\n');
        }
    }
    Ok(output)
}

fn adapt_network_record(
    line: &str,
    ports: usize,
    frequency_multiplier: f64,
) -> Result<String, SolveError> {
    let mut fields = line.split_whitespace().collect::<Vec<_>>();
    let frequency_hz = fields
        .first()
        .ok_or_else(|| write_error("rust-rf emitted an empty network record"))?
        .parse::<f64>()
        .map_err(|error| write_error(format!("rust-rf emitted an invalid frequency: {error}")))?;
    let mut output = vec![format_number(frequency_hz / frequency_multiplier)];
    if ports == 2 {
        if fields.len() != 9 {
            return Err(write_error(
                "rust-rf emitted an incomplete two-port network record",
            ));
        }
        output.extend(
            [1, 2, 5, 6, 3, 4, 7, 8]
                .into_iter()
                .map(|index| fields[index].to_owned()),
        );
    } else {
        fields.remove(0);
        output.extend(fields.into_iter().map(str::to_owned));
    }
    Ok(output.join(" "))
}

fn append_noise_data(output: &mut String, network: &Network, frequency_multiplier: f64) {
    let Some(noise) = &network.noise else {
        return;
    };
    output.push_str("[Noise Data]\n");
    for index in 0..noise.frequency.points() {
        let optimum = noise.optimal_reflection[index];
        let values = [
            noise.frequency.values_hz()[index] / frequency_multiplier,
            noise.minimum_noise_figure_db[index],
            optimum.norm(),
            optimum.arg().to_degrees(),
            noise.equivalent_noise_resistance[index],
        ];
        output.push_str(
            &values
                .into_iter()
                .map(format_number)
                .collect::<Vec<_>>()
                .join(" "),
        );
        output.push('\n');
    }
}

fn validate_writer_policy(network: &Network, format: TouchstoneFormat) -> Result<(), SolveError> {
    if network.frequency_points() == 0 {
        return Err(write_error("at least one network point is required"));
    }
    validate_frequencies(network.frequency.values_hz(), "network")?;
    for value in &network.s {
        if !value.re.is_finite() || !value.im.is_finite() {
            return Err(write_error("network values must be finite"));
        }
        if format == TouchstoneFormat::DecibelAngle && value.norm() == 0.0 {
            return Err(write_error(
                "zero network values cannot be represented exactly in DB format",
            ));
        }
    }
    for reference in &network.z0 {
        if !reference.re.is_finite() || reference.re <= 0.0 || reference.im != 0.0 {
            return Err(write_error(
                "port reference impedances must be finite, real, and positive",
            ));
        }
    }
    if let Some(noise) = &network.noise {
        if network.ports() != 2 {
            return Err(write_error(
                "noise data is only valid for two-port documents",
            ));
        }
        validate_frequencies(noise.frequency.values_hz(), "noise")?;
        for index in 0..noise.frequency.points() {
            let optimum = noise.optimal_reflection[index];
            if !noise.minimum_noise_figure_db[index].is_finite()
                || !optimum.re.is_finite()
                || !optimum.im.is_finite()
                || !noise.equivalent_noise_resistance[index].is_finite()
                || noise.equivalent_noise_resistance[index] < 0.0
            {
                return Err(write_error("noise values must be finite and valid"));
            }
        }
    }
    Ok(())
}

fn validate_frequencies(
    frequencies_hz: &ndarray::Array1<f64>,
    section: &str,
) -> Result<(), SolveError> {
    for (index, frequency_hz) in frequencies_hz.iter().enumerate() {
        if !frequency_hz.is_finite() || *frequency_hz < 0.0 {
            return Err(write_error(format!(
                "{section} frequencies must be finite and nonnegative"
            )));
        }
        if index > 0 && frequencies_hz[index - 1] == *frequency_hz {
            return Err(write_error(format!("{section} frequencies must be unique")));
        }
    }
    Ok(())
}

fn rust_rf_format_name(option_line: &str) -> Result<&str, SolveError> {
    option_line
        .split_whitespace()
        .nth(3)
        .ok_or_else(|| write_error("rust-rf emitted an incomplete option line"))
}

fn touchstone_frequency_unit(unit: ScalarUnit) -> Result<(&'static str, f64), SolveError> {
    match unit {
        ScalarUnit::Hertz => Ok(("Hz", 1.0)),
        ScalarUnit::KiloHertz => Ok(("kHz", 1.0e3)),
        ScalarUnit::MegaHertz => Ok(("MHz", 1.0e6)),
        ScalarUnit::GigaHertz => Ok(("GHz", 1.0e9)),
        _ => Err(write_error(
            "Touchstone supports Hz, kHz, MHz, or GHz frequency units",
        )),
    }
}

fn format_number(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else {
        value.to_string()
    }
}

fn write_error(reason: impl Into<String>) -> SolveError {
    SolveError::TouchstoneWriteFailed {
        reason: reason.into(),
    }
}
