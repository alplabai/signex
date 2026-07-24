use crate::transmission_line_calculator::element_analysis::{
    apply_element, custom_impedance, open_stub_impedance_at, shorted_stub_impedance_at, shunt,
    summary_element, transmission_line_with_dielectric,
};
use crate::transmission_line_calculator::rust_rf_adapter::standing_wave_ratio;
use crate::transmission_line_calculator::s_parameter_analysis::{
    solve_frequency_result_variants, solve_noise_figure, solve_s_parameter_gain,
    solve_s_parameter_gain_variants, solve_s1p_reflection_variants, solve_stability_circles,
};
use crate::transmission_line_calculator::*;

pub(crate) const TAU: f64 = std::f64::consts::PI * 2.0;
pub(crate) const IMPEDANCE_ARC_SEGMENTS: usize = 50;

/// Solves the configured transmission-line network.
pub fn solve(
    load_impedance: Complex,
    elements: &[CircuitElement],
    settings: SolveSettings,
) -> Result<SolveResult, SolveError> {
    validate_settings(settings)?;

    let mut impedance = load_impedance;
    let mut steps = Vec::new();

    let load = CircuitElement {
        name: "Load".to_string(),
        kind: ElementKind::Load,
        value: load_impedance.re,
        enabled: true,
    };
    steps.push(step_for(load, impedance, settings.reference_impedance_ohm));

    for element in elements.iter().filter(|element| element.enabled) {
        validate_element(element)?;
        impedance = apply_element(impedance, element, settings)?;
        steps.push(step_for(
            element.clone(),
            impedance,
            settings.reference_impedance_ohm,
        ));
    }

    let normalized_impedance = impedance * (1.0 / settings.reference_impedance_ohm);
    let reflection_coefficient =
        impedance_to_reflection(impedance, settings.reference_impedance_ohm);
    let admittance = impedance.reciprocal().ok_or(SolveError::SingularNetwork {
        kind: ElementKind::Load,
    })?;
    let normalized_admittance = admittance * settings.reference_impedance_ohm;
    let gamma_mag = reflection_coefficient.magnitude();
    let return_loss_db = if gamma_mag <= f64::EPSILON {
        f64::INFINITY
    } else {
        -20.0 * gamma_mag.log10()
    };
    let vswr = standing_wave_ratio(reflection_coefficient);

    Ok(SolveResult {
        impedance,
        normalized_impedance,
        reflection_coefficient,
        admittance,
        normalized_admittance,
        return_loss_db,
        vswr,
        chart_x: reflection_coefficient.re,
        chart_y: reflection_coefficient.im,
        steps,
    })
}

/// Analyzes smith chart and returns the derived Smith-chart data.
pub fn analyze_smith_chart(
    circuit: &[SmithChartElement],
    settings: SmithChartSettings,
) -> Result<SmithChartAnalysis, SolveError> {
    let active_frequency_hz = select_active_frequency(circuit, settings.frequency_hz);
    let mut normalized = circuit.to_vec();
    let nominal_elements = normalize_tolerance_variant(&mut normalized, 1.0);
    let nominal = solve_smith_chart_nominal(
        &nominal_elements,
        active_frequency_hz,
        settings.show_ideal,
        settings.reference_impedance_ohm,
    )?;

    let tolerance_variants = tolerance_variants(circuit);
    let mut tolerance_results = Vec::new();
    for variant in &tolerance_variants {
        tolerance_results.push(solve_smith_chart_nominal(
            variant,
            active_frequency_hz,
            settings.show_ideal,
            settings.reference_impedance_ohm,
        )?);
    }
    let impedance_arcs = solve_impedance_arcs(
        std::iter::once(nominal_elements.as_slice())
            .chain(tolerance_variants.iter().map(Vec::as_slice)),
        active_frequency_hz,
        settings.show_ideal,
    )?;

    let frequency_result_variants =
        solve_frequency_result_variants(&nominal_elements, &settings, active_frequency_hz)?;
    let frequency_results = frequency_result_variants
        .last()
        .cloned()
        .unwrap_or_default();

    let s_parameter_gain =
        solve_s_parameter_gain(&nominal_elements, &settings, active_frequency_hz)?;
    let s1p_reflection_variants =
        solve_s1p_reflection_variants(&nominal_elements, &settings, active_frequency_hz)?;
    let s_parameter_gain_variants =
        solve_s_parameter_gain_variants(&nominal_elements, &settings, active_frequency_hz)?;
    let two_port_frequencies = frequency_results
        .iter()
        .map(|point| point.frequency_hz)
        .collect::<Vec<_>>();
    let two_port_s_parameters = solve_two_port_s_parameters(
        &nominal_elements,
        &two_port_frequencies,
        settings.show_ideal,
        settings.reference_impedance_ohm,
    )?;
    let noise_figure = solve_noise_figure(&nominal_elements, &settings, active_frequency_hz)?;
    let stability_circles =
        solve_stability_circles(&nominal_elements, &settings, active_frequency_hz);

    Ok(SmithChartAnalysis {
        nominal,
        tolerance_results,
        impedance_arcs,
        frequency_results,
        frequency_result_variants,
        s1p_reflection_variants,
        s_parameter_gain,
        s_parameter_gain_variants,
        two_port_s_parameters,
        noise_figure,
        stability_circles,
        active_frequency_hz,
    })
}

/// Analyzes smith chart with runtime adjustments and returns the derived Smith-chart data.
pub fn analyze_smith_chart_with_runtime_adjustments(
    circuit: &[SmithChartElement],
    settings: SmithChartSettings,
    adjustments: &[RuntimeAdjustment],
) -> Result<SmithChartAnalysis, SolveError> {
    let adjusted = apply_runtime_adjustments(circuit, adjustments);
    analyze_smith_chart(&adjusted, settings)
}

/// Applies runtime adjustments and returns the resulting value.
pub fn apply_runtime_adjustments(
    circuit: &[SmithChartElement],
    adjustments: &[RuntimeAdjustment],
) -> Vec<SmithChartElement> {
    circuit
        .iter()
        .enumerate()
        .map(|(index, element)| {
            apply_runtime_adjustment(
                element.clone(),
                adjustments.get(index).copied().unwrap_or_default(),
            )
        })
        .collect()
}

/// Validates settings and rejects non-finite or invalid values.
fn validate_settings(settings: SolveSettings) -> Result<(), SolveError> {
    if !settings.frequency_hz.is_finite() || settings.frequency_hz <= 0.0 {
        return Err(SolveError::NonPositiveFrequency);
    }
    if !settings.reference_impedance_ohm.is_finite() || settings.reference_impedance_ohm <= 0.0 {
        return Err(SolveError::NonPositiveReferenceImpedance);
    }
    if !settings.velocity_factor.is_finite() || settings.velocity_factor <= 0.0 {
        return Err(SolveError::NonPositiveVelocityFactor);
    }
    Ok(())
}

/// Validates element and rejects non-finite or invalid values.
fn validate_element(element: &CircuitElement) -> Result<(), SolveError> {
    if (!element.value.is_finite() || element.value <= 0.0) && element.kind != ElementKind::Load {
        return Err(SolveError::NonPositiveElementValue { kind: element.kind });
    }
    Ok(())
}

/// Solves smith chart nominal from the supplied circuit and settings.
pub(super) fn solve_smith_chart_nominal(
    circuit: &[SmithChartElement],
    frequency_hz: f64,
    show_ideal: bool,
    fallback_reference_ohm: f64,
) -> Result<SolveResult, SolveError> {
    let mut impedance = starting_impedance(circuit, frequency_hz)?;
    let reference = circuit
        .iter()
        .find_map(|element| match element {
            SmithChartElement::SParameter(block) => Some(block.reference_impedance_ohm()),
            _ => None,
        })
        .unwrap_or(fallback_reference_ohm);
    let mut steps = vec![SolveStep {
        element: CircuitElement::load(impedance),
        impedance,
        normalized_impedance: impedance / reference,
        reflection_coefficient: impedance_to_reflection(impedance, reference),
    }];

    for element in circuit.iter().skip(1) {
        impedance = apply_smith_chart_element(impedance, element, frequency_hz, show_ideal)?;
        steps.push(SolveStep {
            element: summary_element(element),
            impedance,
            normalized_impedance: impedance / reference,
            reflection_coefficient: impedance_to_reflection(impedance, reference),
        });
    }

    let reflection_coefficient = impedance_to_reflection(impedance, reference);
    let admittance = impedance.reciprocal().ok_or(SolveError::SingularNetwork {
        kind: ElementKind::Load,
    })?;
    let gamma_mag = reflection_coefficient.magnitude();
    Ok(SolveResult {
        impedance,
        normalized_impedance: impedance / reference,
        reflection_coefficient,
        admittance,
        normalized_admittance: admittance * reference,
        return_loss_db: if gamma_mag <= f64::EPSILON {
            f64::INFINITY
        } else {
            -20.0 * gamma_mag.log10()
        },
        vswr: standing_wave_ratio(reflection_coefficient),
        chart_x: reflection_coefficient.re,
        chart_y: reflection_coefficient.im,
        steps,
    })
}

/// Solves frequency points from the supplied circuit and settings.
pub(crate) fn solve_frequency_points(
    circuit: &[SmithChartElement],
    frequencies_hz: &[f64],
    show_ideal: bool,
    reference_impedance_ohm: f64,
) -> Result<Vec<FrequencyPointResult>, SolveError> {
    let mut normalized = circuit.to_vec();
    let nominal_elements = normalize_tolerance_variant(&mut normalized, 1.0);
    frequencies_hz
        .iter()
        .copied()
        .map(|frequency_hz| {
            solve_smith_chart_nominal(
                &nominal_elements,
                frequency_hz,
                show_ideal,
                reference_impedance_ohm,
            )
            .map(|result| FrequencyPointResult {
                frequency_hz,
                impedance: result.impedance,
                reflection_coefficient: result.reflection_coefficient,
            })
        })
        .collect()
}

/// Solves impedance arcs from the supplied circuit and settings.
fn solve_impedance_arcs<'a>(
    variants: impl IntoIterator<Item = &'a [SmithChartElement]>,
    frequency_hz: f64,
    show_ideal: bool,
) -> Result<Vec<ImpedanceArc>, SolveError> {
    let mut arcs = Vec::new();
    for (variant_index, circuit) in variants.into_iter().enumerate() {
        let mut impedance = starting_impedance(circuit, frequency_hz)?;
        for (element_index, element) in circuit.iter().enumerate().skip(1) {
            let next_impedance =
                apply_smith_chart_element(impedance, element, frequency_hz, show_ideal)?;
            let points =
                sample_impedance_arc(impedance, next_impedance, element, frequency_hz, show_ideal)?;
            arcs.push(ImpedanceArc {
                variant_index,
                element_index,
                element_name: summary_element(element).name,
                points,
            });
            impedance = next_impedance;
        }
    }
    Ok(arcs)
}

/// Samples impedance arc across the requested range.
fn sample_impedance_arc(
    start: Complex,
    end: Complex,
    element: &SmithChartElement,
    frequency_hz: f64,
    show_ideal: bool,
) -> Result<Vec<Complex>, SolveError> {
    let mut points = Vec::with_capacity(IMPEDANCE_ARC_SEGMENTS + 1);
    for index in 0..=IMPEDANCE_ARC_SEGMENTS {
        let ratio = index as f64 / IMPEDANCE_ARC_SEGMENTS as f64;
        let point = match element {
            SmithChartElement::TransmissionLine { .. }
            | SmithChartElement::OpenStub { .. }
            | SmithChartElement::ShortedStub { .. }
                if ratio <= f64::EPSILON =>
            {
                start
            }
            SmithChartElement::TransmissionLine { .. }
            | SmithChartElement::OpenStub { .. }
            | SmithChartElement::ShortedStub { .. } => {
                let partial = scale_distributed_element_length(element, ratio);
                apply_smith_chart_element(start, &partial, frequency_hz, show_ideal)?
            }
            _ if is_shunt_smith_chart_element(element) => {
                interpolate_admittance(start, end, ratio)?
            }
            _ => start + (end - start) * ratio,
        };
        if point.re.is_finite() && point.im.is_finite() {
            points.push(point);
        }
    }
    Ok(points)
}

/// Interpolates admittance between the available samples.
fn interpolate_admittance(start: Complex, end: Complex, ratio: f64) -> Result<Complex, SolveError> {
    let start_y = start.reciprocal().ok_or(SolveError::SingularNetwork {
        kind: ElementKind::Load,
    })?;
    let end_y = end.reciprocal().ok_or(SolveError::SingularNetwork {
        kind: ElementKind::Load,
    })?;
    (start_y + (end_y - start_y) * ratio)
        .reciprocal()
        .ok_or(SolveError::SingularNetwork {
            kind: ElementKind::Load,
        })
}

/// Returns whether an element transforms admittance in a shunt branch.
fn is_shunt_smith_chart_element(element: &SmithChartElement) -> bool {
    matches!(
        element,
        SmithChartElement::ShuntCapacitor { .. }
            | SmithChartElement::ShuntInductor { .. }
            | SmithChartElement::ShuntResistor { .. }
            | SmithChartElement::OpenStub { .. }
            | SmithChartElement::ShortedStub { .. }
    )
}

/// Scales distributed element length by the supplied factor.
fn scale_distributed_element_length(element: &SmithChartElement, ratio: f64) -> SmithChartElement {
    match element {
        SmithChartElement::TransmissionLine {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        } => SmithChartElement::TransmissionLine {
            length_m: length_m * ratio,
            characteristic_impedance_ohm: *characteristic_impedance_ohm,
            effective_dielectric: *effective_dielectric,
            tolerance_percent: *tolerance_percent,
        },
        SmithChartElement::OpenStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        } => SmithChartElement::OpenStub {
            length_m: length_m * ratio,
            characteristic_impedance_ohm: *characteristic_impedance_ohm,
            effective_dielectric: *effective_dielectric,
            tolerance_percent: *tolerance_percent,
        },
        SmithChartElement::ShortedStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        } => SmithChartElement::ShortedStub {
            length_m: length_m * ratio,
            characteristic_impedance_ohm: *characteristic_impedance_ohm,
            effective_dielectric: *effective_dielectric,
            tolerance_percent: *tolerance_percent,
        },
        other => other.clone(),
    }
}

/// Applies smith chart element and returns the resulting value.
fn apply_smith_chart_element(
    impedance: Complex,
    element: &SmithChartElement,
    frequency_hz: f64,
    show_ideal: bool,
) -> Result<Complex, SolveError> {
    let omega = TAU * frequency_hz;
    match element {
        SmithChartElement::BlackBox { .. }
        | SmithChartElement::LoadTermination { .. }
        | SmithChartElement::SParameter(_) => Ok(impedance),
        SmithChartElement::SeriesCapacitor {
            capacitance_f,
            esr_ohm,
            esl_h,
            ..
        } => Ok(impedance
            + Complex::new(
                if show_ideal { 0.0 } else { *esr_ohm },
                omega * if show_ideal { 0.0 } else { *esl_h } - 1.0 / (omega * capacitance_f),
            )),
        SmithChartElement::ShuntCapacitor {
            capacitance_f,
            esr_ohm,
            esl_h,
            ..
        } => shunt(
            impedance,
            Complex::new(
                if show_ideal { 0.0 } else { *esr_ohm },
                omega * if show_ideal { 0.0 } else { *esl_h } - 1.0 / (omega * capacitance_f),
            ),
        ),
        SmithChartElement::SeriesInductor {
            inductance_h,
            esr_ohm,
            ..
        } => Ok(impedance
            + Complex::new(
                if show_ideal { 0.0 } else { *esr_ohm },
                omega * inductance_h,
            )),
        SmithChartElement::ShuntInductor {
            inductance_h,
            esr_ohm,
            ..
        } => shunt(
            impedance,
            Complex::new(
                if show_ideal { 0.0 } else { *esr_ohm },
                omega * inductance_h,
            ),
        ),
        SmithChartElement::SeriesResistor {
            resistance_ohm,
            esl_h,
            ..
        } => Ok(impedance
            + Complex::new(
                *resistance_ohm,
                omega * if show_ideal { 0.0 } else { *esl_h },
            )),
        SmithChartElement::ShuntResistor {
            resistance_ohm,
            esl_h,
            ..
        } => shunt(
            impedance,
            Complex::new(
                *resistance_ohm,
                omega * if show_ideal { 0.0 } else { *esl_h },
            ),
        ),
        SmithChartElement::SeriesParallelRlc {
            resistance_ohm,
            inductance_h,
            capacitance_f,
        } => {
            let denominator = 1.0 - omega * omega * inductance_h * capacitance_f;
            let v = (omega * inductance_h) / denominator;
            let branch_admittance = Complex::new(1.0 / resistance_ohm, -1.0 / v);
            Ok(impedance + branch_admittance.reciprocal().unwrap_or(Complex::ZERO))
        }
        SmithChartElement::Custom {
            points,
            interpolation,
        } => Ok(impedance + custom_impedance(points, frequency_hz, *interpolation)),
        SmithChartElement::TransmissionLine {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            ..
        } => transmission_line_with_dielectric(
            impedance,
            *length_m,
            *characteristic_impedance_ohm,
            frequency_hz,
            *effective_dielectric,
        ),
        SmithChartElement::OpenStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            ..
        } => {
            let stub = open_stub_impedance_at(
                *length_m,
                *characteristic_impedance_ohm,
                frequency_hz,
                *effective_dielectric,
            )?;
            shunt(impedance, stub)
        }
        SmithChartElement::ShortedStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            ..
        } => {
            let stub = shorted_stub_impedance_at(
                *length_m,
                *characteristic_impedance_ohm,
                frequency_hz,
                *effective_dielectric,
            );
            shunt(impedance, stub)
        }
        SmithChartElement::Transformer {
            model,
            l1_h,
            l2_h,
            coupling_or_turns_ratio,
        } => match model {
            TransformerModel::Ideal => {
                let ratio = if *coupling_or_turns_ratio <= 0.0 {
                    1.0
                } else {
                    *coupling_or_turns_ratio
                };
                Ok(impedance * (ratio * ratio))
            }
            TransformerModel::CoupledInductor => {
                let w = omega * l1_h;
                let t = omega * l2_h;
                let mutual = coupling_or_turns_ratio * (w * t).sqrt();
                let first = Complex::new(impedance.re, impedance.im + w - mutual);
                let inv = first.reciprocal().ok_or(SolveError::SingularNetwork {
                    kind: ElementKind::CoupledTransformer,
                })?;
                let middle = Complex::new(inv.re, inv.im - 1.0 / mutual)
                    .reciprocal()
                    .ok_or(SolveError::SingularNetwork {
                        kind: ElementKind::CoupledTransformer,
                    })?;
                Ok(Complex::new(middle.re, middle.im + t - mutual))
            }
        },
    }
}

/// Selects active frequency from the available candidates.
pub(crate) fn select_active_frequency(circuit: &[SmithChartElement], requested_hz: f64) -> f64 {
    let Some(block) = circuit.iter().find_map(|element| match element {
        SmithChartElement::SParameter(block) => Some(block),
        _ => None,
    }) else {
        return requested_hz;
    };
    let minimum = block
        .points()
        .iter()
        .map(|point| point.frequency_hz)
        .min_by(f64::total_cmp);
    let maximum = block
        .points()
        .iter()
        .map(|point| point.frequency_hz)
        .max_by(f64::total_cmp);
    minimum
        .zip(maximum)
        .map(|(minimum, maximum)| requested_hz.clamp(minimum, maximum))
        .unwrap_or(requested_hz)
}

/// Computes the starting complex impedance.
fn starting_impedance(
    circuit: &[SmithChartElement],
    frequency_hz: f64,
) -> Result<Complex, SolveError> {
    match circuit.first() {
        Some(SmithChartElement::BlackBox { impedance, .. })
        | Some(SmithChartElement::LoadTermination { impedance, .. }) => Ok(*impedance),
        Some(SmithChartElement::SParameter(block)) => block
            .interpolate(frequency_hz)
            .map(|point| point.z_s11)
            .ok_or(SolveError::MissingSourceElement),
        _ => Err(SolveError::MissingSourceElement),
    }
}

/// Normalizes tolerance variant to the canonical representation.
fn normalize_tolerance_variant(
    elements: &mut [SmithChartElement],
    scale: f64,
) -> Vec<SmithChartElement> {
    elements
        .iter()
        .cloned()
        .map(|element| apply_tolerance_scale(element, scale))
        .collect()
}

/// Builds the lower, nominal, and upper variants for an optional tolerance.
pub(super) fn tolerance_variants(circuit: &[SmithChartElement]) -> Vec<Vec<SmithChartElement>> {
    let mut variants = vec![circuit.to_vec()];
    let mut found_tolerance = false;
    for index in 0..circuit.len() {
        let Some(tolerance) = tolerance_percent(&circuit[index]) else {
            continue;
        };
        found_tolerance = true;
        let hi = 1.0 + tolerance / 100.0;
        let lo = 1.0 - tolerance / 100.0;
        let mut low_variants = variants.clone();
        for variant in &mut variants {
            variant[index] = apply_tolerance_scale(variant[index].clone(), hi);
        }
        for variant in &mut low_variants {
            variant[index] = apply_tolerance_scale(variant[index].clone(), lo);
        }
        variants.extend(low_variants);
    }
    if found_tolerance {
        variants
    } else {
        Vec::new()
    }
}

/// Returns the optional tolerance percentage carried by an element.
fn tolerance_percent(element: &SmithChartElement) -> Option<f64> {
    match element {
        SmithChartElement::BlackBox {
            tolerance_percent, ..
        }
        | SmithChartElement::LoadTermination {
            tolerance_percent, ..
        }
        | SmithChartElement::SeriesCapacitor {
            tolerance_percent, ..
        }
        | SmithChartElement::ShuntCapacitor {
            tolerance_percent, ..
        }
        | SmithChartElement::SeriesInductor {
            tolerance_percent, ..
        }
        | SmithChartElement::ShuntInductor {
            tolerance_percent, ..
        }
        | SmithChartElement::SeriesResistor {
            tolerance_percent, ..
        }
        | SmithChartElement::ShuntResistor {
            tolerance_percent, ..
        }
        | SmithChartElement::TransmissionLine {
            tolerance_percent, ..
        }
        | SmithChartElement::OpenStub {
            tolerance_percent, ..
        }
        | SmithChartElement::ShortedStub {
            tolerance_percent, ..
        } => tolerance_percent.filter(|value| *value > 0.0),
        _ => None,
    }
}

/// Applies tolerance scale and returns the resulting value.
fn apply_tolerance_scale(element: SmithChartElement, scale: f64) -> SmithChartElement {
    match element {
        SmithChartElement::BlackBox {
            impedance,
            tolerance_percent,
        } => SmithChartElement::BlackBox {
            impedance: impedance * scale,
            tolerance_percent,
        },
        SmithChartElement::LoadTermination {
            impedance,
            tolerance_percent,
        } => SmithChartElement::LoadTermination {
            impedance: impedance * scale,
            tolerance_percent,
        },
        SmithChartElement::SeriesCapacitor {
            capacitance_f,
            esr_ohm,
            esl_h,
            tolerance_percent,
        } => SmithChartElement::SeriesCapacitor {
            capacitance_f: capacitance_f * scale,
            esr_ohm,
            esl_h,
            tolerance_percent,
        },
        SmithChartElement::ShuntCapacitor {
            capacitance_f,
            esr_ohm,
            esl_h,
            tolerance_percent,
        } => SmithChartElement::ShuntCapacitor {
            capacitance_f: capacitance_f * scale,
            esr_ohm,
            esl_h,
            tolerance_percent,
        },
        SmithChartElement::SeriesInductor {
            inductance_h,
            esr_ohm,
            tolerance_percent,
        } => SmithChartElement::SeriesInductor {
            inductance_h: inductance_h * scale,
            esr_ohm,
            tolerance_percent,
        },
        SmithChartElement::ShuntInductor {
            inductance_h,
            esr_ohm,
            tolerance_percent,
        } => SmithChartElement::ShuntInductor {
            inductance_h: inductance_h * scale,
            esr_ohm,
            tolerance_percent,
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm,
            esl_h,
            tolerance_percent,
        } => SmithChartElement::SeriesResistor {
            resistance_ohm: resistance_ohm * scale,
            esl_h,
            tolerance_percent,
        },
        SmithChartElement::ShuntResistor {
            resistance_ohm,
            esl_h,
            tolerance_percent,
        } => SmithChartElement::ShuntResistor {
            resistance_ohm: resistance_ohm * scale,
            esl_h,
            tolerance_percent,
        },
        SmithChartElement::TransmissionLine {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        } => SmithChartElement::TransmissionLine {
            length_m: length_m * scale,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        },
        SmithChartElement::OpenStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        } => SmithChartElement::OpenStub {
            length_m: length_m * scale,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        },
        SmithChartElement::ShortedStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        } => SmithChartElement::ShortedStub {
            length_m: length_m * scale,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        },
        other => other,
    }
}

/// Applies runtime adjustment and returns the resulting value.
fn apply_runtime_adjustment(
    element: SmithChartElement,
    adjustment: RuntimeAdjustment,
) -> SmithChartElement {
    match element {
        SmithChartElement::BlackBox {
            impedance,
            tolerance_percent,
        } => SmithChartElement::BlackBox {
            impedance: Complex::new(
                scale_by_percent(impedance.re, adjustment.real_slider_percent),
                scale_by_percent(impedance.im, adjustment.imaginary_slider_percent),
            ),
            tolerance_percent,
        },
        SmithChartElement::LoadTermination {
            impedance,
            tolerance_percent,
        } => SmithChartElement::LoadTermination {
            impedance: Complex::new(
                scale_by_percent(impedance.re, adjustment.real_slider_percent),
                scale_by_percent(impedance.im, adjustment.imaginary_slider_percent),
            ),
            tolerance_percent,
        },
        SmithChartElement::SeriesCapacitor {
            capacitance_f,
            esr_ohm,
            esl_h,
            tolerance_percent,
        } => SmithChartElement::SeriesCapacitor {
            capacitance_f: scale_by_percent(capacitance_f, adjustment.value_slider_percent),
            esr_ohm,
            esl_h,
            tolerance_percent,
        },
        SmithChartElement::ShuntCapacitor {
            capacitance_f,
            esr_ohm,
            esl_h,
            tolerance_percent,
        } => SmithChartElement::ShuntCapacitor {
            capacitance_f: scale_by_percent(capacitance_f, adjustment.value_slider_percent),
            esr_ohm,
            esl_h,
            tolerance_percent,
        },
        SmithChartElement::SeriesInductor {
            inductance_h,
            esr_ohm,
            tolerance_percent,
        } => SmithChartElement::SeriesInductor {
            inductance_h: scale_by_percent(inductance_h, adjustment.value_slider_percent),
            esr_ohm,
            tolerance_percent,
        },
        SmithChartElement::ShuntInductor {
            inductance_h,
            esr_ohm,
            tolerance_percent,
        } => SmithChartElement::ShuntInductor {
            inductance_h: scale_by_percent(inductance_h, adjustment.value_slider_percent),
            esr_ohm,
            tolerance_percent,
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm,
            esl_h,
            tolerance_percent,
        } => SmithChartElement::SeriesResistor {
            resistance_ohm: scale_by_percent(resistance_ohm, adjustment.value_slider_percent),
            esl_h,
            tolerance_percent,
        },
        SmithChartElement::ShuntResistor {
            resistance_ohm,
            esl_h,
            tolerance_percent,
        } => SmithChartElement::ShuntResistor {
            resistance_ohm: scale_by_percent(resistance_ohm, adjustment.value_slider_percent),
            esl_h,
            tolerance_percent,
        },
        SmithChartElement::SeriesParallelRlc {
            resistance_ohm,
            inductance_h,
            capacitance_f,
        } => SmithChartElement::SeriesParallelRlc {
            resistance_ohm: scale_by_percent(resistance_ohm, adjustment.value_slider_percent),
            inductance_h,
            capacitance_f,
        },
        SmithChartElement::TransmissionLine {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        } => SmithChartElement::TransmissionLine {
            length_m: scale_by_percent(length_m, adjustment.value_slider_percent),
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        },
        SmithChartElement::OpenStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        } => SmithChartElement::OpenStub {
            length_m: scale_by_percent(length_m, adjustment.value_slider_percent),
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        },
        SmithChartElement::ShortedStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        } => SmithChartElement::ShortedStub {
            length_m: scale_by_percent(length_m, adjustment.value_slider_percent),
            characteristic_impedance_ohm,
            effective_dielectric,
            tolerance_percent,
        },
        other => other,
    }
}

/// Scales by percent by the supplied factor.
fn scale_by_percent(value: f64, percent: Option<f64>) -> f64 {
    match percent {
        Some(percent) if percent != 0.0 => value * (1.0 + percent / 100.0),
        _ => value,
    }
}

/// Creates a solve step describing an element transformation.
fn step_for(
    element: CircuitElement,
    impedance: Complex,
    reference_impedance_ohm: f64,
) -> SolveStep {
    SolveStep {
        element,
        impedance,
        normalized_impedance: impedance * (1.0 / reference_impedance_ohm),
        reflection_coefficient: impedance_to_reflection(impedance, reference_impedance_ohm),
    }
}
