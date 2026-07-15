use crate::*;

pub fn parse_touchstone(raw: &str) -> Result<SParameterBlock, SolveError> {
    let normalized = raw.trim().replace('–', "-");
    let mut lines = normalized
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !(line.starts_with('!') && !line.contains("Noise parameters")));

    let options = lines
        .next()
        .ok_or_else(|| touchstone_error("missing Touchstone options line"))?;
    let parts = options.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 6 || parts[0] != "#" || !parts[2].eq_ignore_ascii_case("s") {
        return Err(touchstone_error(
            "expected `# <freq_unit> S <RI|MA|DB> R <Z0>`",
        ));
    }

    let (freq_unit, parameter, format, r_marker, z0) = if parts[0] == "#" {
        (parts[1], parts[2], parts[3], parts[4], parts[5])
    } else {
        return Err(touchstone_error("expected options line starting with `#`"));
    };
    if !parameter.eq_ignore_ascii_case("S") || !r_marker.eq_ignore_ascii_case("R") {
        return Err(touchstone_error(
            "only S-parameter files with R reference are supported",
        ));
    }
    let format = match format.to_ascii_uppercase().as_str() {
        "RI" => TouchstoneFormat::RealImaginary,
        "MA" => TouchstoneFormat::MagnitudeAngle,
        "DB" => TouchstoneFormat::DecibelAngle,
        _ => return Err(touchstone_error("unsupported Touchstone format")),
    };
    let source_frequency_unit = parse_frequency_unit(freq_unit)?;
    let frequency_multiplier = source_frequency_unit.multiplier();
    let reference_impedance_ohm = z0
        .parse::<f64>()
        .map_err(|_| touchstone_error("invalid reference impedance"))?;

    let mut points: Vec<SParameterPoint> = Vec::new();
    let mut noise_lines = Vec::new();
    let mut in_noise = false;
    for line in lines {
        if line.contains("Noise parameters") {
            in_noise = true;
            continue;
        }
        if in_noise {
            noise_lines.push(line.to_string());
            continue;
        }
        let values = line
            .split_whitespace()
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| touchstone_error("invalid numeric S-parameter row"))?;
        let point = parse_s_parameter_row(
            &values,
            format,
            frequency_multiplier,
            reference_impedance_ohm,
        )?;
        if let Some(existing) = points
            .iter_mut()
            .find(|existing| same_number(existing.frequency_hz, point.frequency_hz))
        {
            *existing = point;
        } else {
            points.push(point);
        }
    }
    if points.is_empty() {
        return Err(touchstone_error("no S-parameter rows found"));
    }
    points.sort_by(|left, right| left.frequency_hz.total_cmp(&right.frequency_hz));
    let kind = if points[0].s21.is_some() {
        SParameterKind::S2P
    } else {
        SParameterKind::S1P
    };
    let known_freqs = points
        .iter()
        .map(|point| point.frequency_hz)
        .collect::<Vec<_>>();
    let mut noise: Vec<NoisePoint> = Vec::new();
    for line in noise_lines {
        let values = line
            .split_whitespace()
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| touchstone_error("invalid numeric noise row"))?;
        if values.len() != 5 {
            return Err(touchstone_error("invalid Touchstone noise row"));
        }
        let frequency_hz = touchstone_frequency_hz(values[0], frequency_multiplier);
        if !known_freqs
            .iter()
            .any(|known| (*known - frequency_hz).abs() <= f64::EPSILON)
        {
            continue;
        }
        let optimum_gamma = Complex::from_polar(values[2], values[3]);
        let noise_point = NoisePoint {
            frequency_hz,
            fmin_db: values[1],
            optimum_gamma,
            rn_ohm: values[4] * reference_impedance_ohm,
            optimum_admittance: reflection_to_impedance(optimum_gamma, reference_impedance_ohm)
                .reciprocal()
                .unwrap_or(Complex::ZERO),
        };
        if let Some(existing) = noise
            .iter_mut()
            .find(|existing| same_number(existing.frequency_hz, noise_point.frequency_hz))
        {
            *existing = noise_point;
        } else {
            noise.push(noise_point);
        }
    }
    noise.sort_by(|left, right| left.frequency_hz.total_cmp(&right.frequency_hz));

    Ok(SParameterBlock {
        kind,
        reference_impedance_ohm,
        source_frequency_unit,
        points,
        noise,
        raw: raw.to_string(),
    })
}

fn parse_s_parameter_row(
    values: &[f64],
    format: TouchstoneFormat,
    frequency_multiplier: f64,
    reference_impedance_ohm: f64,
) -> Result<SParameterPoint, SolveError> {
    if values.len() != 3 && values.len() != 9 {
        return Err(touchstone_error("S-parameter row must have 3 or 9 values"));
    }
    let frequency_hz = touchstone_frequency_hz(values[0], frequency_multiplier);
    let s11 = parse_touchstone_pair(values[1], values[2], format);
    let (s21, s12, s22) = if values.len() == 9 {
        (
            Some(parse_touchstone_pair(values[3], values[4], format)),
            Some(parse_touchstone_pair(values[5], values[6], format)),
            Some(parse_touchstone_pair(values[7], values[8], format)),
        )
    } else {
        (None, None, None)
    };
    Ok(SParameterPoint {
        frequency_hz,
        s11,
        s21,
        s12,
        s22,
        z_s11: reflection_to_impedance(s11, reference_impedance_ohm),
    })
}

fn touchstone_frequency_hz(value: f64, multiplier: f64) -> f64 {
    (value * multiplier).round()
}

fn parse_touchstone_pair(a: f64, b: f64, format: TouchstoneFormat) -> Complex {
    match format {
        TouchstoneFormat::RealImaginary => Complex::new(a, b),
        TouchstoneFormat::MagnitudeAngle => Complex::from_polar(a, b),
        TouchstoneFormat::DecibelAngle => Complex::from_polar(10.0_f64.powf(a / 20.0), b),
    }
}

fn parse_frequency_unit(unit: &str) -> Result<ScalarUnit, SolveError> {
    match unit.to_ascii_lowercase().as_str() {
        "hz" => Ok(ScalarUnit::Hertz),
        "khz" => Ok(ScalarUnit::KiloHertz),
        "mhz" => Ok(ScalarUnit::MegaHertz),
        "ghz" => Ok(ScalarUnit::GigaHertz),
        "thz" => Ok(ScalarUnit::TeraHertz),
        _ => Err(touchstone_error("unknown Touchstone frequency unit")),
    }
}

fn touchstone_error(reason: impl Into<String>) -> SolveError {
    SolveError::TouchstoneParseFailed {
        reason: reason.into(),
    }
}
