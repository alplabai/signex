use super::analysis::{select_active_frequency, solve_smith_chart_nominal, tolerance_variants};
use super::rust_rf_adapter::{from_rf_complex, to_rf_complex};
use super::*;

const RF_CIRCLE_POINTS: usize = 17;

/// Collects the frequency ordered samples.
fn frequency_samples(
    circuit: &[SmithChartElement],
    settings: &SmithChartSettings,
    active_frequency_hz: f64,
) -> Vec<f64> {
    if let Some(block) = circuit.iter().find_map(|element| match element {
        SmithChartElement::SParameter(block) => Some(block),
        _ => None,
    }) {
        if settings.span_hz <= 0.0 {
            return vec![active_frequency_hz];
        }
        let min = active_frequency_hz - settings.span_hz;
        let max = active_frequency_hz + settings.span_hz;
        return block
            .points()
            .iter()
            .map(|point| point.frequency_hz)
            .filter(|frequency| *frequency >= min && *frequency <= max)
            .collect();
    }
    if settings.span_hz <= 0.0 || settings.resolution == 0 {
        return vec![active_frequency_hz];
    }
    let step = settings.span_hz / settings.resolution as f64;
    (-(settings.resolution as isize)..=(settings.resolution as isize))
        .map(|offset| active_frequency_hz + offset as f64 * step)
        .collect()
}

/// Solves frequency result variants from the supplied circuit and settings.
pub(super) fn solve_frequency_result_variants(
    circuit: &[SmithChartElement],
    settings: &SmithChartSettings,
    active_frequency_hz: f64,
) -> Result<Vec<Vec<FrequencyPointResult>>, SolveError> {
    let frequencies = frequency_samples(circuit, settings, active_frequency_hz);
    let mut variants = Vec::new();
    for variant in tolerance_variants_with_nominal(circuit) {
        let mut trace = Vec::new();
        for frequency_hz in frequencies.iter().copied() {
            let result = solve_smith_chart_nominal(
                &variant,
                frequency_hz,
                settings.show_ideal,
                settings.reference_impedance_ohm,
            )?;
            trace.push(FrequencyPointResult {
                frequency_hz,
                impedance: result.impedance,
                reflection_coefficient: result.reflection_coefficient,
            });
        }
        variants.push(trace);
    }
    Ok(variants)
}

/// Solves s parameter gain from the supplied circuit and settings.
pub(super) fn solve_s_parameter_gain(
    circuit: &[SmithChartElement],
    settings: &SmithChartSettings,
    active_frequency_hz: f64,
) -> Result<Vec<GainPoint>, SolveError> {
    let Some(block_index) = circuit
        .iter()
        .position(|element| matches!(element, SmithChartElement::SParameter(_)))
    else {
        return Ok(Vec::new());
    };
    let SmithChartElement::SParameter(block) = &circuit[block_index] else {
        return Ok(Vec::new());
    };
    if block.kind() != SParameterKind::S2P {
        return Ok(Vec::new());
    }
    let source_side = &circuit[..block_index];
    let load_side = circuit[block_index + 1..]
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    let frequencies = frequency_samples(circuit, settings, active_frequency_hz);
    solve_s_parameter_gain_for_sides(block, source_side, &load_side, &frequencies, settings)
}

/// Solves s parameter gain variants from the supplied circuit and settings.
pub(super) fn solve_s_parameter_gain_variants(
    circuit: &[SmithChartElement],
    settings: &SmithChartSettings,
    active_frequency_hz: f64,
) -> Result<Vec<Vec<GainPoint>>, SolveError> {
    let Some(block_index) = circuit
        .iter()
        .position(|element| matches!(element, SmithChartElement::SParameter(_)))
    else {
        return Ok(Vec::new());
    };
    let SmithChartElement::SParameter(block) = &circuit[block_index] else {
        return Ok(Vec::new());
    };
    if block.kind() != SParameterKind::S2P {
        return Ok(Vec::new());
    }
    let source_variants = gain_side_variants(&circuit[..block_index]);
    let load_side = circuit[block_index + 1..]
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    let load_variants = gain_side_variants(&load_side);
    let frequencies = frequency_samples(circuit, settings, active_frequency_hz);
    let mut out = Vec::new();
    for source_side in &source_variants {
        for load_side in &load_variants {
            out.push(solve_s_parameter_gain_for_sides(
                block,
                source_side,
                load_side,
                &frequencies,
                settings,
            )?);
        }
    }
    Ok(out)
}

/// Returns the nominal, low, and high gain-side variants.
fn gain_side_variants(side: &[SmithChartElement]) -> Vec<Vec<SmithChartElement>> {
    tolerance_variants_with_nominal(side)
}

/// Builds a variant list that always starts with the nominal value.
fn tolerance_variants_with_nominal(circuit: &[SmithChartElement]) -> Vec<Vec<SmithChartElement>> {
    let mut variants = tolerance_variants(circuit);
    variants.push(circuit.to_vec());
    variants
}

/// Solves s1p reflection variants from the supplied circuit and settings.
pub(super) fn solve_s1p_reflection_variants(
    circuit: &[SmithChartElement],
    settings: &SmithChartSettings,
    active_frequency_hz: f64,
) -> Result<Vec<Vec<S1pReflectionPoint>>, SolveError> {
    let Some(block_index) = circuit.iter().position(|element| {
        matches!(
            element,
            SmithChartElement::SParameter(block) if block.kind() == SParameterKind::S1P
        )
    }) else {
        return Ok(Vec::new());
    };
    let SmithChartElement::SParameter(block) = &circuit[block_index] else {
        return Ok(Vec::new());
    };
    let reference_impedance = s1p_reference_impedance(circuit, block.reference_impedance_ohm());
    let frequencies = frequency_samples(circuit, settings, active_frequency_hz);
    let mut out = Vec::new();
    for variant in tolerance_variants_with_nominal(circuit) {
        let mut trace = Vec::new();
        for frequency_hz in frequencies.iter().copied() {
            let result = solve_smith_chart_nominal(
                &variant,
                frequency_hz,
                settings.show_ideal,
                settings.reference_impedance_ohm,
            )?;
            let reflection_coefficient =
                impedance_to_reflection_complex(result.impedance, reference_impedance);
            trace.push(S1pReflectionPoint {
                frequency_hz,
                reflection_coefficient,
                magnitude: reflection_coefficient.magnitude(),
                angle_degrees: reflection_coefficient.phase_degrees(),
            });
        }
        out.push(trace);
    }
    Ok(out)
}

/// Computes the s1p reference complex impedance.
fn s1p_reference_impedance(circuit: &[SmithChartElement], fallback_reference_ohm: f64) -> Complex {
    circuit
        .iter()
        .find_map(|element| match element {
            SmithChartElement::LoadTermination { impedance, .. }
            | SmithChartElement::BlackBox { impedance, .. } => Some(*impedance),
            _ => None,
        })
        .unwrap_or(Complex::new(fallback_reference_ohm, 0.0))
}

/// Converts impedance to reflection form using a complex reference impedance.
fn impedance_to_reflection_complex(impedance: Complex, reference_impedance: Complex) -> Complex {
    (impedance - reference_impedance) / (impedance + reference_impedance)
}

/// Solves s parameter gain for sides from the supplied circuit and settings.
fn solve_s_parameter_gain_for_sides(
    block: &SParameterBlock,
    source_side: &[SmithChartElement],
    load_side: &[SmithChartElement],
    frequencies: &[f64],
    settings: &SmithChartSettings,
) -> Result<Vec<GainPoint>, SolveError> {
    let mut out = Vec::new();
    for frequency_hz in frequencies.iter().copied() {
        let Some(point) = block.interpolate(frequency_hz) else {
            continue;
        };
        let Some(s_parameters) = point.s_parameter_matrix() else {
            continue;
        };
        let source_z = solve_smith_chart_nominal(
            source_side,
            frequency_hz,
            settings.show_ideal,
            settings.reference_impedance_ohm,
        )
        .map(|result| result.impedance)
        .unwrap_or(Complex::new(settings.reference_impedance_ohm, 0.0));
        let load_z = solve_smith_chart_nominal(
            load_side,
            frequency_hz,
            settings.show_ideal,
            settings.reference_impedance_ohm,
        )
        .map(|result| result.impedance)
        .unwrap_or(Complex::new(settings.reference_impedance_ohm, 0.0));
        let source_reference = block
            .port_reference_impedances_ohm()
            .first()
            .copied()
            .unwrap_or(block.reference_impedance_ohm());
        let load_reference = block
            .port_reference_impedances_ohm()
            .get(1)
            .copied()
            .unwrap_or(block.reference_impedance_ohm());
        let gamma_source = impedance_to_reflection(source_z, source_reference);
        let gamma_load = impedance_to_reflection(load_z, load_reference);
        let Some(transducer_gain_linear) = s_parameters.transducer_gain(gamma_source, gamma_load)
        else {
            continue;
        };
        out.push(GainPoint {
            frequency_hz,
            transducer_gain_linear,
        });
    }
    Ok(out)
}

/// Solves stability circles from the supplied circuit and settings.
pub(super) fn solve_stability_circles(
    circuit: &[SmithChartElement],
    settings: &SmithChartSettings,
    active_frequency_hz: f64,
) -> Vec<StabilityCircle> {
    let Some(block) = circuit.iter().find_map(|element| match element {
        SmithChartElement::SParameter(block) if block.kind() == SParameterKind::S2P => Some(block),
        _ => None,
    }) else {
        return Vec::new();
    };

    let frequencies = frequency_samples(circuit, settings, active_frequency_hz);
    let mut out = Vec::new();
    for frequency_hz in frequencies {
        let Some(network) = block.network_at(frequency_hz) else {
            continue;
        };
        let Some((source_center, source_radius)) = network
            .stability_circle(0, RF_CIRCLE_POINTS)
            .ok()
            .and_then(|points| circle_geometry(points.row(0)))
        else {
            continue;
        };
        let Some((load_center, load_radius)) = network
            .stability_circle(1, RF_CIRCLE_POINTS)
            .ok()
            .and_then(|points| circle_geometry(points.row(0)))
        else {
            continue;
        };
        out.push(StabilityCircle {
            frequency_hz,
            source_center,
            source_radius,
            load_center,
            load_radius,
        });
    }
    out
}

/// Solves s parameter gain circles from the supplied circuit and settings.
pub fn solve_s_parameter_gain_circles(
    circuit: &[SmithChartElement],
    settings: &SmithChartSettings,
    input_targets_db: &[f64],
    output_targets_db: &[f64],
) -> Vec<GainCircle> {
    let active_frequency_hz = select_active_frequency(circuit, settings.frequency_hz);
    let Some(block) = circuit.iter().find_map(|element| match element {
        SmithChartElement::SParameter(block) if block.kind() == SParameterKind::S2P => Some(block),
        _ => None,
    }) else {
        return Vec::new();
    };
    let Some(network) = block.network_at(active_frequency_hz) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for target_gain_db in input_targets_db
        .iter()
        .copied()
        .filter(|value| value.is_finite())
    {
        if let Some(circle) = gain_circle(
            &network,
            GainCirclePort::Input,
            active_frequency_hz,
            target_gain_db,
        ) {
            out.push(circle);
        }
    }
    for target_gain_db in output_targets_db
        .iter()
        .copied()
        .filter(|value| value.is_finite())
    {
        if let Some(circle) = gain_circle(
            &network,
            GainCirclePort::Output,
            active_frequency_hz,
            target_gain_db,
        ) {
            out.push(circle);
        }
    }
    out
}

/// Solves noise figure circles from the supplied circuit and settings.
pub fn solve_noise_figure_circles(
    circuit: &[SmithChartElement],
    settings: &SmithChartSettings,
    targets_db: &[f64],
) -> Vec<NoiseFigureCircle> {
    let Some(block) = circuit.iter().find_map(|element| match element {
        SmithChartElement::SParameter(block) => Some(block),
        _ => None,
    }) else {
        return Vec::new();
    };
    let active_noise_frequency_hz = select_active_noise_frequency(block, settings.frequency_hz);
    let Some(noise) = exact_noise_point_at(block, active_noise_frequency_hz) else {
        return Vec::new();
    };
    let Some(noise_index) = block.network().noise.as_ref().and_then(|parameters| {
        parameters
            .frequency
            .values_hz()
            .iter()
            .position(|frequency| same_number(*frequency, active_noise_frequency_hz))
    }) else {
        return Vec::new();
    };
    targets_db
        .iter()
        .copied()
        .filter_map(|target_noise_figure_db| {
            noise_figure_circle(block, &noise, noise_index, target_noise_figure_db).map(
                |(center, radius)| NoiseFigureCircle {
                    frequency_hz: active_noise_frequency_hz,
                    target_noise_figure_db,
                    center,
                    radius,
                },
            )
        })
        .collect()
}

/// Computes the gain circle geometry.
fn gain_circle(
    network: &rust_rf::Network,
    port: GainCirclePort,
    frequency_hz: f64,
    target_gain_db: f64,
) -> Option<GainCircle> {
    let port_index = match port {
        GainCirclePort::Input => 0,
        GainCirclePort::Output => 1,
    };
    let s_parameter = from_rf_complex(network.s[(0, port_index, port_index)]);
    let magnitude_squared = s_parameter.magnitude().powi(2);
    let denominator = 1.0 - magnitude_squared;
    if denominator.abs() <= f64::EPSILON {
        return None;
    }
    let normalized_gain = 10.0_f64.powf(target_gain_db / 10.0) * denominator;
    if normalized_gain > 1.0 {
        return None;
    }
    let (center, radius) = network
        .gain_circle(port_index, target_gain_db, RF_CIRCLE_POINTS)
        .ok()
        .and_then(|points| circle_geometry(points.row(0)))?;
    Some(GainCircle {
        frequency_hz,
        port,
        target_gain_db,
        center,
        radius,
    })
}

/// Computes the noise figure circle geometry.
fn noise_figure_circle(
    block: &SParameterBlock,
    noise: &NoisePoint,
    noise_index: usize,
    target_noise_figure_db: f64,
) -> Option<(Complex, f64)> {
    let fmin_linear = 10.0_f64.powf(noise.fmin_db / 10.0);
    let target_linear = 10.0_f64.powf(target_noise_figure_db / 10.0);
    if !target_noise_figure_db.is_finite()
        || noise.rn_ohm / block.reference_impedance_ohm() <= f64::EPSILON
        || target_linear < fmin_linear
    {
        return None;
    }
    block
        .network()
        .noise_figure_circle(target_noise_figure_db, RF_CIRCLE_POINTS)
        .ok()
        .and_then(|points| circle_geometry(points.row(noise_index)))
}

/// Solves noise figure from the supplied circuit and settings.
pub(crate) fn solve_noise_figure(
    circuit: &[SmithChartElement],
    settings: &SmithChartSettings,
    active_frequency_hz: f64,
) -> Result<Vec<NoiseFigurePoint>, SolveError> {
    let Some(block_index) = circuit
        .iter()
        .position(|element| matches!(element, SmithChartElement::SParameter(_)))
    else {
        return Ok(Vec::new());
    };
    let SmithChartElement::SParameter(block) = &circuit[block_index] else {
        return Ok(Vec::new());
    };
    let source_side = &circuit[..block_index];
    let frequencies = noise_frequency_samples(block, settings, active_frequency_hz);
    let mut out = Vec::new();
    for frequency_hz in frequencies {
        if exact_noise_point_at(block, frequency_hz).is_none() {
            continue;
        }
        let source_z = solve_smith_chart_nominal(
            source_side,
            frequency_hz,
            settings.show_ideal,
            settings.reference_impedance_ohm,
        )
        .map(|result| result.impedance)
        .unwrap_or(Complex::new(settings.reference_impedance_ohm, 0.0));
        let Some(noise_index) = block.network().noise.as_ref().and_then(|parameters| {
            parameters
                .frequency
                .values_hz()
                .iter()
                .position(|candidate| same_number(*candidate, frequency_hz))
        }) else {
            continue;
        };
        let Some(noise_factor_linear) = block
            .network()
            .noise_factor(to_rf_complex(source_z))
            .ok()
            .and_then(|values| values.get(noise_index).copied())
        else {
            continue;
        };
        out.push(NoiseFigurePoint {
            frequency_hz,
            noise_factor_linear,
        });
    }
    Ok(out)
}

fn circle_geometry(points: ndarray::ArrayView1<'_, rust_rf::Complex64>) -> Option<(Complex, f64)> {
    let unique_count = points.len().checked_sub(1)?;
    if unique_count < 2 {
        return None;
    }
    let center = points
        .iter()
        .take(unique_count)
        .copied()
        .sum::<rust_rf::Complex64>()
        / unique_count as f64;
    let radius = points
        .iter()
        .take(unique_count)
        .map(|point| (*point - center).norm())
        .sum::<f64>()
        / unique_count as f64;
    (center.re.is_finite() && center.im.is_finite() && radius.is_finite())
        .then_some((from_rf_complex(center), radius))
}

/// Selects active noise frequency from the available candidates.
fn select_active_noise_frequency(block: &SParameterBlock, requested_hz: f64) -> f64 {
    let noise = block.noise();
    noise
        .iter()
        .find(|point| point.frequency_hz >= requested_hz)
        .or_else(|| noise.last())
        .map(|point| point.frequency_hz)
        .unwrap_or(requested_hz)
}

/// Collects the noise frequency ordered samples.
pub(crate) fn noise_frequency_samples(
    block: &SParameterBlock,
    settings: &SmithChartSettings,
    active_frequency_hz: f64,
) -> Vec<f64> {
    let min = active_frequency_hz - settings.span_hz;
    let max = active_frequency_hz + settings.span_hz;
    block
        .noise()
        .iter()
        .map(|point| point.frequency_hz)
        .filter(|frequency| *frequency >= min && *frequency <= max)
        .collect()
}

/// Returns the noise-parameter point whose frequency exactly matches the request.
fn exact_noise_point_at(block: &SParameterBlock, frequency_hz: f64) -> Option<NoisePoint> {
    block
        .noise()
        .into_iter()
        .find(|point| same_number(point.frequency_hz, frequency_hz))
}
