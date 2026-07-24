use std::{fs, path::Path};

use rust_rf::{FrequencyUnit, io::Touchstone};

use crate::transmission_line_calculator::{
    Complex, NoisePoint, SParameterBlock, SParameterKind, SParameterPoint, ScalarUnit, SolveError,
    reflection_to_impedance,
    rust_rf_adapter::{from_rf_complex, map_touchstone_parse_error},
};

pub fn parse_touchstone(raw: &str) -> Result<SParameterBlock, SolveError> {
    let rank_hint = infer_version_one_rank(raw)?;
    let touchstone =
        Touchstone::from_reader(raw.as_bytes(), rank_hint).map_err(map_touchstone_parse_error)?;
    block_from_touchstone(&touchstone, raw)
}

pub fn read_touchstone(path: impl AsRef<Path>) -> Result<SParameterBlock, SolveError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| SolveError::TouchstoneReadFailed {
        reason: format!("{}: {error}", path.display()),
    })?;
    let raw = decode_touchstone(&bytes);
    let rank_hint = infer_version_one_rank(&raw)?;
    let touchstone =
        Touchstone::from_reader(raw.as_bytes(), rank_hint).map_err(map_touchstone_parse_error)?;
    block_from_touchstone(&touchstone, &raw)
}

fn block_from_touchstone(
    touchstone: &Touchstone,
    raw: &str,
) -> Result<SParameterBlock, SolveError> {
    let kind = match touchstone.rank {
        1 => SParameterKind::S1P,
        2 => SParameterKind::S2P,
        ports => {
            return Err(touchstone_error(format!(
                "the transmission-line calculator supports one- and two-port data, not {ports}-port data"
            )));
        }
    };
    let references = port_references(touchstone)?;
    let reference_impedance_ohm = references[0];
    let scattering = touchstone.s_parameters();
    if touchstone
        .frequencies_hz()
        .iter()
        .any(|frequency| !frequency.is_finite() || *frequency < 0.0)
        || scattering
            .iter()
            .any(|value| !value.re.is_finite() || !value.im.is_finite())
    {
        return Err(touchstone_error(
            "network frequencies and S-parameters must be finite and frequencies must not be negative",
        ));
    }
    let default_version_two_order = touchstone.rank == 2
        && touchstone.version != "1.0"
        && !raw.lines().any(|line| {
            line.trim()
                .to_ascii_lowercase()
                .starts_with("[two-port data order]")
        });
    let mut points = touchstone
        .frequencies_hz()
        .iter()
        .enumerate()
        .map(|(index, frequency_hz)| {
            let s11 = from_rf_complex(scattering[(index, 0, 0)]);
            SParameterPoint {
                frequency_hz: frequency_hz.round(),
                s11,
                s21: (touchstone.rank == 2).then(|| {
                    let (row, column) = if default_version_two_order {
                        (0, 1)
                    } else {
                        (1, 0)
                    };
                    from_rf_complex(scattering[(index, row, column)])
                }),
                s12: (touchstone.rank == 2).then(|| {
                    let (row, column) = if default_version_two_order {
                        (1, 0)
                    } else {
                        (0, 1)
                    };
                    from_rf_complex(scattering[(index, row, column)])
                }),
                s22: (touchstone.rank == 2).then(|| from_rf_complex(scattering[(index, 1, 1)])),
                z_s11: reflection_to_impedance(s11, reference_impedance_ohm),
            }
        })
        .collect::<Vec<_>>();
    replace_duplicate_points(&mut points);

    let network = touchstone.network().map_err(map_touchstone_parse_error)?;
    let declares_noise_data = raw
        .lines()
        .any(|line| line.trim().to_ascii_lowercase().starts_with("[noise data]"));
    if declares_noise_data && network.noise.is_none() {
        return Err(touchstone_error(
            "a noise-data section must contain at least one complete record",
        ));
    }
    if network.noise.is_some() && kind != SParameterKind::S2P {
        return Err(touchstone_error(
            "noise data is only valid for two-port documents",
        ));
    }
    let mut noise = network
        .noise
        .as_ref()
        .map(|parameters| {
            (0..parameters.frequency.points())
                .map(|index| {
                    let optimum_gamma = from_rf_complex(parameters.optimal_reflection[index]);
                    let optimum_admittance =
                        reflection_to_impedance(optimum_gamma, reference_impedance_ohm)
                            .reciprocal()
                            .unwrap_or(Complex::ZERO);
                    NoisePoint {
                        frequency_hz: parameters.frequency.values_hz()[index].round(),
                        fmin_db: parameters.minimum_noise_figure_db[index],
                        optimum_gamma,
                        rn_ohm: parameters.equivalent_noise_resistance[index],
                        optimum_admittance,
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    replace_duplicate_noise(&mut noise);

    SParameterBlock::from_samples(
        kind,
        references,
        scalar_frequency_unit(touchstone.frequency_unit),
        points,
        noise,
        raw.to_owned(),
    )
}

fn infer_version_one_rank(raw: &str) -> Result<usize, SolveError> {
    if raw.lines().any(|line| {
        line.trim_start_matches('\u{feff}')
            .trim()
            .to_ascii_lowercase()
            .starts_with("[number of ports]")
    }) {
        return Ok(0);
    }

    let mut option_seen = false;
    for line in raw.lines() {
        let content = line
            .trim_start_matches('\u{feff}')
            .split_once('!')
            .map_or(line, |(content, _)| content)
            .trim();
        if content.is_empty() {
            continue;
        }
        if content.starts_with('#') {
            option_seen = true;
            continue;
        }
        if option_seen && !content.starts_with('[') {
            let values = content.split_whitespace().count();
            return match values {
                3 => Ok(1),
                9 => Ok(2),
                _ => Err(touchstone_error(format!(
                    "cannot infer a one- or two-port rank from a {values}-value Version 1 record"
                ))),
            };
        }
    }

    Err(touchstone_error(
        "file contains no Version 1 network record from which to infer a port count",
    ))
}

fn port_references(touchstone: &Touchstone) -> Result<Vec<f64>, SolveError> {
    let references = if touchstone.reference_impedances.is_empty() {
        vec![touchstone.resistance; touchstone.rank]
    } else {
        touchstone.reference_impedances.clone()
    };
    references
        .into_iter()
        .map(|reference| {
            if reference.im != 0.0 || !reference.re.is_finite() || reference.re <= 0.0 {
                Err(touchstone_error(
                    "port reference impedances must be finite, real, and positive",
                ))
            } else {
                Ok(reference.re)
            }
        })
        .collect()
}

fn replace_duplicate_points(points: &mut Vec<SParameterPoint>) {
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

fn replace_duplicate_noise(noise: &mut Vec<NoisePoint>) {
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

const fn scalar_frequency_unit(unit: FrequencyUnit) -> ScalarUnit {
    match unit {
        FrequencyUnit::Hz => ScalarUnit::Hertz,
        FrequencyUnit::KHz => ScalarUnit::KiloHertz,
        FrequencyUnit::MHz => ScalarUnit::MegaHertz,
        FrequencyUnit::GHz => ScalarUnit::GigaHertz,
        FrequencyUnit::THz => ScalarUnit::TeraHertz,
    }
}

fn decode_touchstone(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

fn touchstone_error(reason: impl Into<String>) -> SolveError {
    SolveError::TouchstoneParseFailed {
        reason: reason.into(),
    }
}
