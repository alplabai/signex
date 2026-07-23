use std::{fs, path::Path};

use crate::transmission_line_calculator::{
    Complex, NoisePoint, SParameterBlock, SParameterKind, SParameterPoint, ScalarUnit, SolveError,
    reflection_to_impedance,
};

mod matrix_format;
mod option_line;
mod touchstone_format;
#[cfg(test)]
#[path = "../../../tests/transmission_line_calculator/touchstone_tests.rs"]
mod touchstone_tests;
mod touchstone_version;
mod two_port_data_order;
mod version2_header;

use matrix_format::MatrixFormat;
use option_line::OptionLine;
pub use touchstone_format::TouchstoneFormat;
use touchstone_version::TouchstoneVersion;
use two_port_data_order::TwoPortDataOrder;
use version2_header::Version2Header;

const DEFAULT_REFERENCE_IMPEDANCE_OHM: f64 = 50.0;

/// Parses one-port or two-port S-parameter data in Touchstone 1.x or 2.x syntax.
///
/// Touchstone 2.x keywords are case-insensitive. Network records may span lines,
/// and comments may occupy a complete line or follow data. The supported subset
/// includes `RI`, `MA`, and `DB` encodings, full or triangular matrices, per-port
/// references, and optional two-port noise data. A missing two-port order uses
/// the conventional `21_12` order for compatibility with published examples.
///
/// # Errors
///
/// Returns [`SolveError::TouchstoneParseFailed`] when required metadata is
/// absent, the document contains unsupported parameter or port types, or a
/// numeric record is malformed.
pub fn parse_touchstone(raw: &str) -> Result<SParameterBlock, SolveError> {
    let lines = significant_lines(raw);
    let first = lines
        .first()
        .ok_or_else(|| touchstone_parse_error("file contains no Touchstone data"))?;
    let (version, option_line_index) = match split_keyword(first)? {
        Some((name, argument)) if name.eq_ignore_ascii_case("Version") => {
            let version = parse_version(argument)?;
            (version, 1)
        }
        Some(_) => {
            return Err(touchstone_parse_error(
                "the first keyword must be `[Version]`",
            ));
        }
        None => (TouchstoneVersion::Version1, 0),
    };

    let option_line_text = lines
        .get(option_line_index)
        .ok_or_else(|| touchstone_parse_error("missing Touchstone option line"))?;
    let options = parse_option_line(option_line_text)?;

    match version {
        TouchstoneVersion::Version1 => {
            parse_version1(raw, &lines[option_line_index + 1..], options)
        }
        TouchstoneVersion::Version2 => {
            parse_version2(raw, &lines[option_line_index + 1..], options)
        }
    }
}

/// Reads and parses a Touchstone document from `path`.
///
/// # Errors
///
/// Returns [`SolveError::TouchstoneReadFailed`] when the file cannot be read as
/// UTF-8 text, or the parsing error returned by [`parse_touchstone`].
pub fn read_touchstone(path: impl AsRef<Path>) -> Result<SParameterBlock, SolveError> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path).map_err(|error| SolveError::TouchstoneReadFailed {
        reason: format!("{}: {error}", path.display()),
    })?;
    parse_touchstone(&raw)
}

/// Serializes a one-port or two-port S-parameter block as Touchstone 2.1 text.
///
/// The output uses a full matrix, explicit port references, deterministic
/// ascending frequency order, and the requested complex-number `format`.
///
/// # Errors
///
/// Returns [`SolveError::TouchstoneWriteFailed`] if the block is empty,
/// internally inconsistent, non-finite, or cannot be represented exactly in
/// the requested format.
pub fn serialize_touchstone(
    block: &SParameterBlock,
    format: TouchstoneFormat,
) -> Result<String, SolveError> {
    let port_count = port_count(block.kind);
    let frequency_unit = touchstone_frequency_unit_name(block.source_frequency_unit)?;
    let frequency_multiplier = block.source_frequency_unit.multiplier();
    let references = effective_port_references(block, port_count)?;
    let mut points = block.points.iter().collect::<Vec<_>>();
    points.sort_by(|left, right| left.frequency_hz.total_cmp(&right.frequency_hz));
    validate_points(&points, block.kind, format)?;

    let mut noise = block.noise.iter().collect::<Vec<_>>();
    noise.sort_by(|left, right| left.frequency_hz.total_cmp(&right.frequency_hz));
    validate_noise(&noise, block.kind)?;

    let mut output = String::new();
    output.push_str("! Touchstone 2.1 file written by Signex\n");
    output.push_str("[Version] 2.1\n");
    output.push_str(&format!(
        "# {frequency_unit} S {} R {}\n",
        touchstone_format_name(format),
        format_touchstone_number(references[0])
    ));
    output.push_str(&format!("[Number of Ports] {port_count}\n"));
    if block.kind == SParameterKind::S2P {
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
        if block.kind == SParameterKind::S2P {
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

/// Serializes a block and writes it to a Touchstone file at `path`.
///
/// # Errors
///
/// Returns the validation error from [`serialize_touchstone`] or
/// [`SolveError::TouchstoneWriteFailed`] when the destination cannot be written.
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

/// Removes comments and blank lines while preserving data-line boundaries.
fn significant_lines(raw: &str) -> Vec<String> {
    raw.trim_start_matches('\u{feff}')
        .replace('–', "-")
        .lines()
        .filter_map(|line| {
            let content = line.split_once('!').map_or(line, |(content, _)| content);
            let content = content.trim();
            (!content.is_empty()).then(|| content.to_string())
        })
        .collect()
}

/// Splits a bracketed keyword from the argument text on the same line.
fn split_keyword(line: &str) -> Result<Option<(&str, &str)>, SolveError> {
    if !line.starts_with('[') {
        return Ok(None);
    }
    let closing_bracket = line
        .find(']')
        .ok_or_else(|| touchstone_parse_error("keyword is missing its closing bracket"))?;
    let name = &line[1..closing_bracket];
    if name.is_empty() || name.starts_with(char::is_whitespace) {
        return Err(touchstone_parse_error("invalid Touchstone keyword"));
    }
    Ok(Some((name, line[closing_bracket + 1..].trim())))
}

/// Parses the required Version 2.0 or 2.1 declaration.
fn parse_version(argument: &str) -> Result<TouchstoneVersion, SolveError> {
    match argument {
        "2.0" | "2.1" => Ok(TouchstoneVersion::Version2),
        _ => Err(touchstone_parse_error(
            "`[Version]` must specify Touchstone 2.0 or 2.1",
        )),
    }
}

/// Parses an order-independent option line and applies standard defaults.
fn parse_option_line(line: &str) -> Result<OptionLine, SolveError> {
    let Some(arguments) = line.strip_prefix('#') else {
        return Err(touchstone_parse_error(
            "expected an option line beginning with `#`",
        ));
    };
    let tokens = arguments.split_whitespace().collect::<Vec<_>>();
    let mut frequency_unit = ScalarUnit::GigaHertz;
    let mut parameter = "S";
    let mut format = TouchstoneFormat::MagnitudeAngle;
    let mut references = vec![DEFAULT_REFERENCE_IMPEDANCE_OHM];
    let mut saw_reference = false;
    let mut index = 0;

    while index < tokens.len() {
        let token = tokens[index];
        if let Some(unit) = parse_frequency_unit_token(token) {
            frequency_unit = unit;
            index += 1;
            continue;
        }
        if matches_ignore_ascii_case(token, &["S", "Y", "Z", "H", "G"]) {
            parameter = token;
            index += 1;
            continue;
        }
        if token.eq_ignore_ascii_case("RI") {
            format = TouchstoneFormat::RealImaginary;
            index += 1;
            continue;
        }
        if token.eq_ignore_ascii_case("MA") {
            format = TouchstoneFormat::MagnitudeAngle;
            index += 1;
            continue;
        }
        if token.eq_ignore_ascii_case("DB") {
            format = TouchstoneFormat::DecibelAngle;
            index += 1;
            continue;
        }
        if token.eq_ignore_ascii_case("R") {
            if saw_reference {
                return Err(touchstone_parse_error(
                    "option line contains more than one `R` entry",
                ));
            }
            saw_reference = true;
            references.clear();
            index += 1;
            while index < tokens.len() {
                let Ok(reference) = tokens[index].parse::<f64>() else {
                    break;
                };
                validate_positive_finite(reference, "reference impedance")?;
                references.push(reference);
                index += 1;
            }
            if references.is_empty() {
                return Err(touchstone_parse_error(
                    "`R` must be followed by a positive reference impedance",
                ));
            }
            continue;
        }
        return Err(touchstone_parse_error(format!(
            "unsupported option-line token `{token}`"
        )));
    }

    if !parameter.eq_ignore_ascii_case("S") {
        return Err(touchstone_parse_error(format!(
            "unsupported `{parameter}` parameters; only S-parameters are supported"
        )));
    }
    Ok(OptionLine {
        frequency_unit,
        format,
        reference_impedances_ohm: references,
    })
}

/// Parses a Version 1.x document whose port count is inferred from each row.
fn parse_version1(
    raw: &str,
    lines: &[String],
    options: OptionLine,
) -> Result<SParameterBlock, SolveError> {
    let mut network_rows = Vec::new();
    let mut noise_rows = Vec::new();
    let mut port_count = None;
    let mut in_noise_data = false;

    for line in lines {
        if line.starts_with('#') {
            continue;
        }
        if split_keyword(line)?.is_some() {
            return Err(touchstone_parse_error(
                "Touchstone 2.x keywords require a `[Version]` declaration",
            ));
        }
        let values = parse_numeric_values(line, "data row")?;
        let inferred_port_count = match values.len() {
            3 => 1,
            9 => 2,
            5 if port_count == Some(2) => {
                in_noise_data = true;
                2
            }
            _ => {
                return Err(touchstone_parse_error(
                    "Version 1.x rows must contain 3, 9, or 5 noise values",
                ));
            }
        };
        if let Some(port_count) = port_count {
            if inferred_port_count != port_count {
                return Err(touchstone_parse_error(
                    "document mixes one-port and two-port network rows",
                ));
            }
        } else {
            port_count = Some(inferred_port_count);
        }
        if in_noise_data {
            if values.len() != 5 {
                return Err(touchstone_parse_error(
                    "network data cannot follow Version 1.x noise data",
                ));
            }
            noise_rows.push(values);
        } else {
            network_rows.push(values);
        }
    }

    let port_count = port_count.ok_or_else(|| touchstone_parse_error("no network data found"))?;
    let references = expand_references(&options.reference_impedances_ohm, port_count)?;
    let network_values = network_rows.into_iter().flatten().collect::<Vec<_>>();
    let (mut points, _) = parse_network_values(
        &network_values,
        port_count,
        MatrixFormat::Full,
        TwoPortDataOrder::S21S12,
        options.format,
        options.frequency_unit.multiplier(),
        references[0],
    )?;
    let noise_values = noise_rows.into_iter().flatten().collect::<Vec<_>>();
    let mut noise = parse_noise_values(
        &noise_values,
        options.frequency_unit.multiplier(),
        options.reference_impedances_ohm[0],
        true,
    )?;
    sort_and_replace_duplicate_points(&mut points);
    sort_and_replace_duplicate_noise(&mut noise);
    Ok(build_block(
        raw,
        port_count,
        options.frequency_unit,
        references,
        points,
        noise,
    ))
}

/// Parses Version 2.x header keywords and sectioned network/noise records.
fn parse_version2(
    raw: &str,
    lines: &[String],
    options: OptionLine,
) -> Result<SParameterBlock, SolveError> {
    if options.reference_impedances_ohm.len() != 1 {
        return Err(touchstone_parse_error(
            "Version 2.x option lines support one `R` value; use `[Reference]` per port",
        ));
    }
    let (header, network_start) = parse_version2_header(lines, &options)?;
    let (network_values, noise_values, saw_noise_data) =
        parse_version2_sections(&lines[network_start..])?;
    let (mut points, point_count) = parse_network_values(
        &network_values,
        header.port_count,
        header.matrix_format,
        header
            .two_port_data_order
            .unwrap_or(TwoPortDataOrder::S21S12),
        options.format,
        options.frequency_unit.multiplier(),
        header.reference_impedances_ohm[0],
    )?;
    if point_count != header.frequency_count {
        return Err(touchstone_parse_error(format!(
            "`[Number of Frequencies]` declares {}, but {point_count} records were found",
            header.frequency_count
        )));
    }

    let mut noise = parse_noise_values(
        &noise_values,
        options.frequency_unit.multiplier(),
        options.reference_impedances_ohm[0],
        false,
    )?;
    match (header.noise_frequency_count, saw_noise_data) {
        (Some(expected), true) if expected == noise.len() => {}
        (Some(expected), true) => {
            return Err(touchstone_parse_error(format!(
                "`[Number of Noise Frequencies]` declares {expected}, but {} records were found",
                noise.len()
            )));
        }
        (Some(_), false) => {
            return Err(touchstone_parse_error(
                "`[Number of Noise Frequencies]` requires a `[Noise Data]` section",
            ));
        }
        (None, true) => {
            return Err(touchstone_parse_error(
                "`[Noise Data]` requires `[Number of Noise Frequencies]`",
            ));
        }
        (None, false) => {}
    }
    if !noise.is_empty() && header.port_count != 2 {
        return Err(touchstone_parse_error(
            "noise data is only valid for two-port documents",
        ));
    }
    sort_and_replace_duplicate_points(&mut points);
    sort_and_replace_duplicate_noise(&mut noise);
    Ok(build_block(
        raw,
        header.port_count,
        options.frequency_unit,
        header.reference_impedances_ohm,
        points,
        noise,
    ))
}

/// Parses Version 2.x metadata up to and including `[Network Data]`.
fn parse_version2_header(
    lines: &[String],
    options: &OptionLine,
) -> Result<(Version2Header, usize), SolveError> {
    let mut port_count = None;
    let mut frequency_count = None;
    let mut noise_frequency_count = None;
    let mut references = None;
    let mut matrix_format = MatrixFormat::Full;
    let mut saw_matrix_format = false;
    let mut two_port_data_order = None;
    let mut in_information = false;
    let mut index = 0;

    while index < lines.len() {
        let line = &lines[index];
        if line.starts_with('#') {
            index += 1;
            continue;
        }
        let keyword = split_keyword(line)?;
        if in_information && keyword.is_none() {
            index += 1;
            continue;
        }
        let Some((name, argument)) = keyword else {
            return Err(touchstone_parse_error(
                "unexpected data before `[Network Data]`",
            ));
        };
        if in_information {
            if name.eq_ignore_ascii_case("End Information") {
                in_information = false;
            }
            index += 1;
            continue;
        }
        if port_count.is_none() && !name.eq_ignore_ascii_case("Number of Ports") {
            return Err(touchstone_parse_error(
                "`[Number of Ports]` must be the first keyword after the option line",
            ));
        }

        if name.eq_ignore_ascii_case("Number of Ports") {
            ensure_not_set(port_count.is_some(), "Number of Ports")?;
            let parsed = parse_single_usize(argument, "Number of Ports")?;
            if !matches!(parsed, 1 | 2) {
                return Err(touchstone_parse_error(
                    "only one-port and two-port documents are supported",
                ));
            }
            port_count = Some(parsed);
        } else if name.eq_ignore_ascii_case("Two-Port Data Order") {
            ensure_not_set(two_port_data_order.is_some(), "Two-Port Data Order")?;
            two_port_data_order = Some(parse_two_port_data_order(argument)?);
        } else if name.eq_ignore_ascii_case("Number of Frequencies") {
            ensure_not_set(frequency_count.is_some(), "Number of Frequencies")?;
            frequency_count = Some(parse_single_usize(argument, "Number of Frequencies")?);
        } else if name.eq_ignore_ascii_case("Number of Noise Frequencies") {
            ensure_not_set(
                noise_frequency_count.is_some(),
                "Number of Noise Frequencies",
            )?;
            let parsed = parse_single_usize(argument, "Number of Noise Frequencies")?;
            if parsed == 0 {
                return Err(touchstone_parse_error(
                    "`[Number of Noise Frequencies]` must be greater than zero",
                ));
            }
            noise_frequency_count = Some(parsed);
        } else if name.eq_ignore_ascii_case("Reference") {
            ensure_not_set(references.is_some(), "Reference")?;
            let mut values = parse_numeric_values(argument, "Reference")?;
            index += 1;
            while index < lines.len()
                && !lines[index].starts_with('[')
                && !lines[index].starts_with('#')
            {
                values.extend(parse_numeric_values(&lines[index], "Reference")?);
                index += 1;
            }
            if values.is_empty() {
                return Err(touchstone_parse_error(
                    "`[Reference]` requires one value per port",
                ));
            }
            for value in &values {
                validate_positive_finite(*value, "reference impedance")?;
            }
            references = Some(values);
            continue;
        } else if name.eq_ignore_ascii_case("Matrix Format") {
            ensure_not_set(saw_matrix_format, "Matrix Format")?;
            matrix_format = parse_matrix_format(argument)?;
            saw_matrix_format = true;
        } else if name.eq_ignore_ascii_case("Begin Information") {
            in_information = true;
        } else if name.eq_ignore_ascii_case("Network Data") {
            if !argument.is_empty() {
                return Err(touchstone_parse_error(
                    "`[Network Data]` does not accept arguments",
                ));
            }
            let port_count = port_count.unwrap();
            let frequency_count = frequency_count.ok_or_else(|| {
                touchstone_parse_error("missing required `[Number of Frequencies]`")
            })?;
            if frequency_count == 0 {
                return Err(touchstone_parse_error(
                    "`[Number of Frequencies]` must be greater than zero",
                ));
            }
            if port_count == 1 && two_port_data_order.is_some() {
                return Err(touchstone_parse_error(
                    "`[Two-Port Data Order]` is invalid for one-port data",
                ));
            }
            let reference_impedances_ohm = match references {
                Some(references) if references.len() == port_count => references,
                Some(_) => {
                    return Err(touchstone_parse_error(format!(
                        "`[Reference]` requires exactly {port_count} values"
                    )));
                }
                None => expand_references(&options.reference_impedances_ohm, port_count)?,
            };
            return Ok((
                Version2Header {
                    port_count,
                    frequency_count,
                    noise_frequency_count,
                    reference_impedances_ohm,
                    matrix_format,
                    two_port_data_order,
                },
                index + 1,
            ));
        } else if name.eq_ignore_ascii_case("Mixed-Mode Order") {
            return Err(touchstone_parse_error(
                "mixed-mode Touchstone data is not supported",
            ));
        } else {
            return Err(touchstone_parse_error(format!(
                "unsupported Touchstone keyword `[{name}]`"
            )));
        }
        index += 1;
    }
    Err(touchstone_parse_error(
        "missing required `[Network Data]` section",
    ))
}

/// Collects numeric values from the Version 2.x network and noise sections.
fn parse_version2_sections(lines: &[String]) -> Result<(Vec<f64>, Vec<f64>, bool), SolveError> {
    let mut network_values = Vec::new();
    let mut noise_values = Vec::new();
    let mut in_noise_data = false;
    let mut saw_noise_data = false;
    let mut saw_end = false;

    for (index, line) in lines.iter().enumerate() {
        if line.starts_with('#') {
            continue;
        }
        if let Some((name, argument)) = split_keyword(line)? {
            if name.eq_ignore_ascii_case("Noise Data") {
                if saw_noise_data || saw_end || !argument.is_empty() {
                    return Err(touchstone_parse_error("invalid `[Noise Data]` section"));
                }
                saw_noise_data = true;
                in_noise_data = true;
                continue;
            }
            if name.eq_ignore_ascii_case("End") {
                if saw_end || !argument.is_empty() {
                    return Err(touchstone_parse_error("invalid `[End]` keyword"));
                }
                if index + 1 != lines.len() {
                    return Err(touchstone_parse_error(
                        "non-comment content appears after `[End]`",
                    ));
                }
                saw_end = true;
                continue;
            }
            return Err(touchstone_parse_error(format!(
                "unexpected keyword `[{name}]` in data section"
            )));
        }
        if saw_end {
            return Err(touchstone_parse_error(
                "non-comment content appears after `[End]`",
            ));
        }
        let values = parse_numeric_values(line, "data section")?;
        if in_noise_data {
            noise_values.extend(values);
        } else {
            network_values.extend(values);
        }
    }
    if !saw_end {
        return Err(touchstone_parse_error("missing required `[End]` keyword"));
    }
    Ok((network_values, noise_values, saw_noise_data))
}

/// Parses flattened network records into one-port or two-port samples.
#[allow(clippy::too_many_arguments)]
fn parse_network_values(
    values: &[f64],
    port_count: usize,
    matrix_format: MatrixFormat,
    order: TwoPortDataOrder,
    format: TouchstoneFormat,
    frequency_multiplier: f64,
    reference_impedance_ohm: f64,
) -> Result<(Vec<SParameterPoint>, usize), SolveError> {
    let pair_count = match (port_count, matrix_format) {
        (1, _) => 1,
        (2, MatrixFormat::Full) => 4,
        (2, MatrixFormat::Lower | MatrixFormat::Upper) => 3,
        _ => return Err(touchstone_parse_error("unsupported port count")),
    };
    let record_width = 1 + pair_count * 2;
    if values.is_empty() || !values.len().is_multiple_of(record_width) {
        return Err(touchstone_parse_error(format!(
            "network data must contain complete {record_width}-value records"
        )));
    }
    let record_count = values.len() / record_width;
    let mut points = Vec::with_capacity(record_count);
    for record in values.chunks_exact(record_width) {
        let frequency_hz = parse_frequency(record[0], frequency_multiplier)?;
        let pairs = record[1..]
            .chunks_exact(2)
            .map(|pair| parse_touchstone_pair(pair[0], pair[1], format))
            .collect::<Result<Vec<_>, _>>()?;
        let (s11, s21, s12, s22) = match (port_count, matrix_format, order) {
            (1, _, _) => (pairs[0], None, None, None),
            (2, MatrixFormat::Full, TwoPortDataOrder::S21S12) => {
                (pairs[0], Some(pairs[1]), Some(pairs[2]), Some(pairs[3]))
            }
            (2, MatrixFormat::Full, TwoPortDataOrder::S12S21) => {
                (pairs[0], Some(pairs[2]), Some(pairs[1]), Some(pairs[3]))
            }
            (2, MatrixFormat::Lower | MatrixFormat::Upper, _) => {
                (pairs[0], Some(pairs[1]), Some(pairs[1]), Some(pairs[2]))
            }
            _ => return Err(touchstone_parse_error("unsupported network matrix")),
        };
        points.push(SParameterPoint {
            frequency_hz,
            s11,
            s21,
            s12,
            s22,
            z_s11: reflection_to_impedance(s11, reference_impedance_ohm),
        });
    }
    Ok((points, record_count))
}

/// Parses flattened five-value noise records.
fn parse_noise_values(
    values: &[f64],
    frequency_multiplier: f64,
    reference_impedance_ohm: f64,
    normalized_resistance: bool,
) -> Result<Vec<NoisePoint>, SolveError> {
    if !values.len().is_multiple_of(5) {
        return Err(touchstone_parse_error(
            "noise data must contain complete five-value records",
        ));
    }
    let mut noise = Vec::with_capacity(values.len() / 5);
    for record in values.chunks_exact(5) {
        for value in record {
            validate_finite(*value, "noise value")?;
        }
        if record[2] < 0.0 {
            return Err(touchstone_parse_error(
                "optimum reflection magnitude must not be negative",
            ));
        }
        if record[4] < 0.0 {
            return Err(touchstone_parse_error(
                "effective noise resistance must not be negative",
            ));
        }
        let frequency_hz = parse_frequency(record[0], frequency_multiplier)?;
        let optimum_gamma = Complex::from_polar(record[2], record[3]);
        let rn_ohm = if normalized_resistance {
            record[4] * reference_impedance_ohm
        } else {
            record[4]
        };
        noise.push(NoisePoint {
            frequency_hz,
            fmin_db: record[1],
            optimum_gamma,
            rn_ohm,
            optimum_admittance: reflection_to_impedance(optimum_gamma, reference_impedance_ohm)
                .reciprocal()
                .unwrap_or(Complex::ZERO),
        });
    }
    Ok(noise)
}

/// Creates the public S-parameter block from parsed fields.
fn build_block(
    raw: &str,
    port_count: usize,
    source_frequency_unit: ScalarUnit,
    port_reference_impedances_ohm: Vec<f64>,
    points: Vec<SParameterPoint>,
    noise: Vec<NoisePoint>,
) -> SParameterBlock {
    SParameterBlock {
        kind: if port_count == 1 {
            SParameterKind::S1P
        } else {
            SParameterKind::S2P
        },
        reference_impedance_ohm: port_reference_impedances_ohm[0],
        port_reference_impedances_ohm,
        source_frequency_unit,
        points,
        noise,
        raw: raw.to_string(),
    }
}

/// Parses one complex network-parameter pair.
fn parse_touchstone_pair(
    first: f64,
    second: f64,
    format: TouchstoneFormat,
) -> Result<Complex, SolveError> {
    validate_finite(first, "network value")?;
    validate_finite(second, "network value")?;
    let value = match format {
        TouchstoneFormat::RealImaginary => Complex::new(first, second),
        TouchstoneFormat::MagnitudeAngle => {
            if first < 0.0 {
                return Err(touchstone_parse_error(
                    "magnitude values must not be negative",
                ));
            }
            Complex::from_polar(first, second)
        }
        TouchstoneFormat::DecibelAngle => Complex::from_polar(10.0_f64.powf(first / 20.0), second),
    };
    if !value.re.is_finite() || !value.im.is_finite() {
        return Err(touchstone_parse_error(
            "network value is outside the supported numeric range",
        ));
    }
    Ok(value)
}

/// Parses and canonicalizes a frequency as an integral hertz key.
fn parse_frequency(value: f64, multiplier: f64) -> Result<f64, SolveError> {
    validate_finite(value, "frequency")?;
    if value < 0.0 {
        return Err(touchstone_parse_error("frequency must not be negative"));
    }
    let frequency_hz = (value * multiplier).round();
    validate_finite(frequency_hz, "frequency")?;
    Ok(frequency_hz)
}

/// Parses all finite floating-point values from one logical data line.
fn parse_numeric_values(line: &str, context: &str) -> Result<Vec<f64>, SolveError> {
    line.split_whitespace()
        .map(|token| {
            let value = token.parse::<f64>().map_err(|_| {
                touchstone_parse_error(format!("invalid numeric value `{token}` in {context}"))
            })?;
            validate_finite(value, context)?;
            Ok(value)
        })
        .collect()
}

/// Converts a legal Touchstone frequency token into its scalar unit.
fn parse_frequency_unit_token(token: &str) -> Option<ScalarUnit> {
    match token.to_ascii_lowercase().as_str() {
        "hz" => Some(ScalarUnit::Hertz),
        "khz" => Some(ScalarUnit::KiloHertz),
        "mhz" => Some(ScalarUnit::MegaHertz),
        "ghz" => Some(ScalarUnit::GigaHertz),
        _ => None,
    }
}

/// Parses a positive integer keyword argument.
fn parse_single_usize(argument: &str, keyword: &str) -> Result<usize, SolveError> {
    let tokens = argument.split_whitespace().collect::<Vec<_>>();
    if tokens.len() != 1 {
        return Err(touchstone_parse_error(format!(
            "`[{keyword}]` requires exactly one integer argument"
        )));
    }
    tokens[0]
        .parse::<usize>()
        .map_err(|_| touchstone_parse_error(format!("`[{keyword}]` requires a positive integer")))
}

/// Parses the matrix-storage keyword argument.
fn parse_matrix_format(argument: &str) -> Result<MatrixFormat, SolveError> {
    if argument.eq_ignore_ascii_case("Full") {
        Ok(MatrixFormat::Full)
    } else if argument.eq_ignore_ascii_case("Lower") {
        Ok(MatrixFormat::Lower)
    } else if argument.eq_ignore_ascii_case("Upper") {
        Ok(MatrixFormat::Upper)
    } else {
        Err(touchstone_parse_error(
            "`[Matrix Format]` must be Full, Lower, or Upper",
        ))
    }
}

/// Parses the two-port transfer-term order keyword argument.
fn parse_two_port_data_order(argument: &str) -> Result<TwoPortDataOrder, SolveError> {
    if argument.eq_ignore_ascii_case("21_12") {
        Ok(TwoPortDataOrder::S21S12)
    } else if argument.eq_ignore_ascii_case("12_21") {
        Ok(TwoPortDataOrder::S12S21)
    } else {
        Err(touchstone_parse_error(
            "`[Two-Port Data Order]` must be 21_12 or 12_21",
        ))
    }
}

/// Expands a shared reference or validates one reference per port.
fn expand_references(references: &[f64], port_count: usize) -> Result<Vec<f64>, SolveError> {
    let expanded = match references {
        [reference] => vec![*reference; port_count],
        references if references.len() == port_count => references.to_vec(),
        _ => {
            return Err(touchstone_parse_error(format!(
                "expected one reference or {port_count} per-port references"
            )));
        }
    };
    for reference in &expanded {
        validate_positive_finite(*reference, "reference impedance")?;
    }
    Ok(expanded)
}

/// Replaces samples that collapse to the same canonical frequency and sorts them.
fn sort_and_replace_duplicate_points(points: &mut Vec<SParameterPoint>) {
    let mut unique: Vec<SParameterPoint> = Vec::with_capacity(points.len());
    for point in points.drain(..) {
        if let Some(existing) = unique
            .iter_mut()
            .find(|existing| existing.frequency_hz == point.frequency_hz)
        {
            *existing = point;
        } else {
            unique.push(point);
        }
    }
    unique.sort_by(|left, right| left.frequency_hz.total_cmp(&right.frequency_hz));
    *points = unique;
}

/// Replaces noise samples at duplicate canonical frequencies and sorts them.
fn sort_and_replace_duplicate_noise(noise: &mut Vec<NoisePoint>) {
    let mut unique: Vec<NoisePoint> = Vec::with_capacity(noise.len());
    for point in noise.drain(..) {
        if let Some(existing) = unique
            .iter_mut()
            .find(|existing| existing.frequency_hz == point.frequency_hz)
        {
            *existing = point;
        } else {
            unique.push(point);
        }
    }
    unique.sort_by(|left, right| left.frequency_hz.total_cmp(&right.frequency_hz));
    *noise = unique;
}

/// Returns the number of ports represented by an S-parameter kind.
fn port_count(kind: SParameterKind) -> usize {
    match kind {
        SParameterKind::S1P => 1,
        SParameterKind::S2P => 2,
    }
}

/// Returns validated per-port references for serialization.
fn effective_port_references(
    block: &SParameterBlock,
    expected_port_count: usize,
) -> Result<Vec<f64>, SolveError> {
    validate_positive_finite_for_write(block.reference_impedance_ohm, "reference impedance")?;
    let references = if block.port_reference_impedances_ohm.is_empty() {
        vec![block.reference_impedance_ohm; expected_port_count]
    } else if block.port_reference_impedances_ohm.len() == expected_port_count {
        block.port_reference_impedances_ohm.clone()
    } else {
        return Err(touchstone_write_error(format!(
            "expected {expected_port_count} port reference impedances"
        )));
    };
    for reference in &references {
        validate_positive_finite_for_write(*reference, "port reference impedance")?;
    }
    if references[0] != block.reference_impedance_ohm {
        return Err(touchstone_write_error(
            "the primary and port 1 reference impedances disagree",
        ));
    }
    Ok(references)
}

/// Validates network points before serialization.
fn validate_points(
    points: &[&SParameterPoint],
    kind: SParameterKind,
    format: TouchstoneFormat,
) -> Result<(), SolveError> {
    if points.is_empty() {
        return Err(touchstone_write_error(
            "at least one network point is required",
        ));
    }
    let mut previous_frequency = None;
    for point in points {
        validate_nonnegative_finite_for_write(point.frequency_hz, "network frequency")?;
        if previous_frequency == Some(point.frequency_hz) {
            return Err(touchstone_write_error("network frequencies must be unique"));
        }
        previous_frequency = Some(point.frequency_hz);
        validate_complex_for_write(point.s11, "S11", format)?;
        match kind {
            SParameterKind::S1P => {
                if point.s21.is_some() || point.s12.is_some() || point.s22.is_some() {
                    return Err(touchstone_write_error(
                        "one-port points must only contain S11",
                    ));
                }
            }
            SParameterKind::S2P => {
                validate_complex_for_write(
                    point
                        .s21
                        .ok_or_else(|| touchstone_write_error("missing S21"))?,
                    "S21",
                    format,
                )?;
                validate_complex_for_write(
                    point
                        .s12
                        .ok_or_else(|| touchstone_write_error("missing S12"))?,
                    "S12",
                    format,
                )?;
                validate_complex_for_write(
                    point
                        .s22
                        .ok_or_else(|| touchstone_write_error("missing S22"))?,
                    "S22",
                    format,
                )?;
            }
        }
    }
    Ok(())
}

/// Validates optional two-port noise data before serialization.
fn validate_noise(noise: &[&NoisePoint], kind: SParameterKind) -> Result<(), SolveError> {
    if !noise.is_empty() && kind != SParameterKind::S2P {
        return Err(touchstone_write_error(
            "noise data is only valid for two-port documents",
        ));
    }
    let mut previous_frequency = None;
    for point in noise {
        validate_nonnegative_finite_for_write(point.frequency_hz, "noise frequency")?;
        if previous_frequency == Some(point.frequency_hz) {
            return Err(touchstone_write_error("noise frequencies must be unique"));
        }
        previous_frequency = Some(point.frequency_hz);
        validate_finite_for_write(point.fmin_db, "minimum noise figure")?;
        validate_complex_for_write(
            point.optimum_gamma,
            "optimum reflection coefficient",
            TouchstoneFormat::MagnitudeAngle,
        )?;
        validate_nonnegative_finite_for_write(point.rn_ohm, "effective noise resistance")?;
    }
    Ok(())
}

/// Validates a complex value for the requested serialization format.
fn validate_complex_for_write(
    value: Complex,
    name: &str,
    format: TouchstoneFormat,
) -> Result<(), SolveError> {
    validate_finite_for_write(value.re, name)?;
    validate_finite_for_write(value.im, name)?;
    if format == TouchstoneFormat::DecibelAngle && value.magnitude() == 0.0 {
        return Err(touchstone_write_error(format!(
            "{name} is zero and cannot be represented exactly in DB format"
        )));
    }
    Ok(())
}

/// Appends one complex value in the requested Touchstone pair format.
fn append_formatted_pair(
    output: &mut Vec<String>,
    value: Complex,
    format: TouchstoneFormat,
) -> Result<(), SolveError> {
    validate_complex_for_write(value, "network value", format)?;
    let (first, second) = match format {
        TouchstoneFormat::RealImaginary => (value.re, value.im),
        TouchstoneFormat::MagnitudeAngle => (value.magnitude(), value.phase_degrees()),
        TouchstoneFormat::DecibelAngle => (20.0 * value.magnitude().log10(), value.phase_degrees()),
    };
    output.push(format_touchstone_number(first));
    output.push(format_touchstone_number(second));
    Ok(())
}

/// Returns the standard option-line name for a complex-number format.
fn touchstone_format_name(format: TouchstoneFormat) -> &'static str {
    match format {
        TouchstoneFormat::RealImaginary => "RI",
        TouchstoneFormat::MagnitudeAngle => "MA",
        TouchstoneFormat::DecibelAngle => "DB",
    }
}

/// Returns the standard option-line token for a supported frequency unit.
fn touchstone_frequency_unit_name(unit: ScalarUnit) -> Result<&'static str, SolveError> {
    match unit {
        ScalarUnit::Hertz => Ok("Hz"),
        ScalarUnit::KiloHertz => Ok("kHz"),
        ScalarUnit::MegaHertz => Ok("MHz"),
        ScalarUnit::GigaHertz => Ok("GHz"),
        _ => Err(touchstone_write_error(
            "Touchstone supports Hz, kHz, MHz, or GHz frequency units",
        )),
    }
}

/// Formats a finite number using Rust's shortest round-trippable representation.
fn format_touchstone_number(value: f64) -> String {
    if value == 0.0 {
        "0".to_string()
    } else {
        value.to_string()
    }
}

/// Returns whether `value` matches one of the supplied ASCII tokens.
fn matches_ignore_ascii_case(value: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
}

/// Rejects a duplicate singleton keyword.
fn ensure_not_set(already_set: bool, keyword: &str) -> Result<(), SolveError> {
    if already_set {
        Err(touchstone_parse_error(format!(
            "`[{keyword}]` appears more than once"
        )))
    } else {
        Ok(())
    }
}

/// Validates a finite parser value.
fn validate_finite(value: f64, name: &str) -> Result<(), SolveError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(touchstone_parse_error(format!("{name} must be finite")))
    }
}

/// Validates a positive finite parser value.
fn validate_positive_finite(value: f64, name: &str) -> Result<(), SolveError> {
    validate_finite(value, name)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(touchstone_parse_error(format!("{name} must be positive")))
    }
}

/// Validates a finite writer value.
fn validate_finite_for_write(value: f64, name: &str) -> Result<(), SolveError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(touchstone_write_error(format!("{name} must be finite")))
    }
}

/// Validates a positive finite writer value.
fn validate_positive_finite_for_write(value: f64, name: &str) -> Result<(), SolveError> {
    validate_finite_for_write(value, name)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(touchstone_write_error(format!("{name} must be positive")))
    }
}

/// Validates a non-negative finite writer value.
fn validate_nonnegative_finite_for_write(value: f64, name: &str) -> Result<(), SolveError> {
    validate_finite_for_write(value, name)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(touchstone_write_error(format!(
            "{name} must not be negative"
        )))
    }
}

/// Creates a parsing error with Touchstone context.
fn touchstone_parse_error(reason: impl Into<String>) -> SolveError {
    SolveError::TouchstoneParseFailed {
        reason: reason.into(),
    }
}

/// Creates a serialization error with Touchstone context.
fn touchstone_write_error(reason: impl Into<String>) -> SolveError {
    SolveError::TouchstoneWriteFailed {
        reason: reason.into(),
    }
}
