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
    let vswr = if gamma_mag >= 1.0 {
        f64::INFINITY
    } else {
        (1.0 + gamma_mag) / (1.0 - gamma_mag)
    };

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
fn solve_smith_chart_nominal(
    circuit: &[SmithChartElement],
    frequency_hz: f64,
    show_ideal: bool,
    fallback_reference_ohm: f64,
) -> Result<SolveResult, SolveError> {
    let mut impedance = starting_impedance(circuit, frequency_hz)?;
    let reference = circuit
        .iter()
        .find_map(|element| match element {
            SmithChartElement::SParameter(block) => Some(block.reference_impedance_ohm),
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
        vswr: if gamma_mag >= 1.0 {
            f64::INFINITY
        } else {
            (1.0 + gamma_mag) / (1.0 - gamma_mag)
        },
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

/// Solves true two-port S-parameters for a supported passive circuit chain.
///
/// The first circuit element is the terminating load used by the Smith-chart
/// impedance walk. Every following element is converted to ABCD form and
/// prepended in the same load-to-source order used by the impedance solver.
/// Circuits containing measured S-parameter blocks or unsupported coupled
/// transformers return an empty set so callers can use their measured data or
/// an explicit fallback.
pub fn solve_two_port_s_parameters(
    circuit: &[SmithChartElement],
    frequencies_hz: &[f64],
    show_ideal: bool,
    reference_impedance_ohm: f64,
) -> Result<Vec<TwoPortSParameterPoint>, SolveError> {
    if !reference_impedance_ohm.is_finite() || reference_impedance_ohm <= 0.0 {
        return Err(SolveError::NonPositiveReferenceImpedance);
    }
    if circuit
        .iter()
        .any(|element| matches!(element, SmithChartElement::SParameter(_)))
    {
        return Ok(Vec::new());
    }

    let reference_impedance = Complex::new(reference_impedance_ohm, 0.0);
    let mut points = Vec::with_capacity(frequencies_hz.len());
    for frequency_hz in frequencies_hz.iter().copied() {
        if !frequency_hz.is_finite() || frequency_hz <= 0.0 {
            return Err(SolveError::NonPositiveFrequency);
        }

        let mut total = AbcdMatrix::identity();
        for element in circuit.iter().skip(1) {
            let Some(element_matrix) = smith_chart_element_abcd(element, frequency_hz, show_ideal)?
            else {
                return Ok(Vec::new());
            };
            total = element_matrix.cascade(total);
        }
        let s_parameters = total
            .to_s_parameters(reference_impedance, reference_impedance)
            .map_err(|_| SolveError::SingularNetwork {
                kind: ElementKind::SParameter,
            })?;
        points.push(TwoPortSParameterPoint {
            frequency_hz,
            s_parameters,
        });
    }
    Ok(points)
}

/// Converts a Smith-chart element to its physical ABCD representation.
fn smith_chart_element_abcd(
    element: &SmithChartElement,
    frequency_hz: f64,
    show_ideal: bool,
) -> Result<Option<AbcdMatrix>, SolveError> {
    let omega = TAU * frequency_hz;
    let matrix = match element {
        SmithChartElement::BlackBox { .. } | SmithChartElement::LoadTermination { .. } => {
            AbcdMatrix::identity()
        }
        SmithChartElement::SeriesCapacitor {
            capacitance_f,
            esr_ohm,
            esl_h,
            ..
        } => AbcdMatrix::series_impedance(Complex::new(
            if show_ideal { 0.0 } else { *esr_ohm },
            omega * if show_ideal { 0.0 } else { *esl_h } - 1.0 / (omega * capacitance_f),
        )),
        SmithChartElement::ShuntCapacitor {
            capacitance_f,
            esr_ohm,
            esl_h,
            ..
        } => {
            let impedance = Complex::new(
                if show_ideal { 0.0 } else { *esr_ohm },
                omega * if show_ideal { 0.0 } else { *esl_h } - 1.0 / (omega * capacitance_f),
            );
            let Some(admittance) = impedance.reciprocal() else {
                return Ok(None);
            };
            AbcdMatrix::shunt_admittance(admittance)
        }
        SmithChartElement::SeriesInductor {
            inductance_h,
            esr_ohm,
            ..
        } => AbcdMatrix::series_impedance(Complex::new(
            if show_ideal { 0.0 } else { *esr_ohm },
            omega * inductance_h,
        )),
        SmithChartElement::ShuntInductor {
            inductance_h,
            esr_ohm,
            ..
        } => {
            let impedance = Complex::new(
                if show_ideal { 0.0 } else { *esr_ohm },
                omega * inductance_h,
            );
            let Some(admittance) = impedance.reciprocal() else {
                return Ok(None);
            };
            AbcdMatrix::shunt_admittance(admittance)
        }
        SmithChartElement::SeriesResistor {
            resistance_ohm,
            esl_h,
            ..
        } => AbcdMatrix::series_impedance(Complex::new(
            *resistance_ohm,
            omega * if show_ideal { 0.0 } else { *esl_h },
        )),
        SmithChartElement::ShuntResistor {
            resistance_ohm,
            esl_h,
            ..
        } => {
            let impedance = Complex::new(
                *resistance_ohm,
                omega * if show_ideal { 0.0 } else { *esl_h },
            );
            let Some(admittance) = impedance.reciprocal() else {
                return Ok(None);
            };
            AbcdMatrix::shunt_admittance(admittance)
        }
        SmithChartElement::SeriesParallelRlc {
            resistance_ohm,
            inductance_h,
            capacitance_f,
        } => {
            let denominator = 1.0 - omega * omega * inductance_h * capacitance_f;
            let reactance = (omega * inductance_h) / denominator;
            let branch_admittance = Complex::new(1.0 / resistance_ohm, -1.0 / reactance);
            let impedance = branch_admittance.reciprocal().unwrap_or(Complex::ZERO);
            AbcdMatrix::series_impedance(impedance)
        }
        SmithChartElement::Custom {
            points,
            interpolation,
        } => AbcdMatrix::series_impedance(custom_impedance(points, frequency_hz, *interpolation)),
        SmithChartElement::TransmissionLine {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            ..
        } => AbcdMatrix::lossless_transmission_line(
            *characteristic_impedance_ohm,
            electrical_length_rad_at(*length_m, frequency_hz, *effective_dielectric),
        )
        .map_err(|_| SolveError::SingularNetwork {
            kind: ElementKind::TransmissionLine,
        })?,
        SmithChartElement::OpenStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            ..
        } => {
            let impedance = open_stub_impedance_at(
                *length_m,
                *characteristic_impedance_ohm,
                frequency_hz,
                *effective_dielectric,
            )?;
            let Some(admittance) = impedance.reciprocal() else {
                return Ok(None);
            };
            AbcdMatrix::shunt_admittance(admittance)
        }
        SmithChartElement::ShortedStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            ..
        } => {
            let impedance = shorted_stub_impedance_at(
                *length_m,
                *characteristic_impedance_ohm,
                frequency_hz,
                *effective_dielectric,
            );
            let Some(admittance) = impedance.reciprocal() else {
                return Ok(None);
            };
            AbcdMatrix::shunt_admittance(admittance)
        }
        SmithChartElement::Transformer {
            model: TransformerModel::Ideal,
            coupling_or_turns_ratio,
            ..
        } => {
            let ratio = if *coupling_or_turns_ratio <= 0.0 {
                1.0
            } else {
                *coupling_or_turns_ratio
            };
            AbcdMatrix::ideal_transformer(ratio).map_err(|_| SolveError::SingularNetwork {
                kind: ElementKind::IdealTransformer,
            })?
        }
        SmithChartElement::Transformer {
            model: TransformerModel::CoupledInductor,
            ..
        }
        | SmithChartElement::SParameter(_) => return Ok(None),
    };
    Ok(Some(matrix))
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
        .points
        .iter()
        .map(|point| point.frequency_hz)
        .min_by(f64::total_cmp);
    let maximum = block
        .points
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
fn tolerance_variants(circuit: &[SmithChartElement]) -> Vec<Vec<SmithChartElement>> {
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
            .points
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
fn solve_frequency_result_variants(
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
fn solve_s_parameter_gain(
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
    if block.kind != SParameterKind::S2P {
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
fn solve_s_parameter_gain_variants(
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
    if block.kind != SParameterKind::S2P {
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
fn solve_s1p_reflection_variants(
    circuit: &[SmithChartElement],
    settings: &SmithChartSettings,
    active_frequency_hz: f64,
) -> Result<Vec<Vec<S1pReflectionPoint>>, SolveError> {
    let Some(block_index) = circuit.iter().position(|element| {
        matches!(
            element,
            SmithChartElement::SParameter(block) if block.kind == SParameterKind::S1P
        )
    }) else {
        return Ok(Vec::new());
    };
    let SmithChartElement::SParameter(block) = &circuit[block_index] else {
        return Ok(Vec::new());
    };
    let reference_impedance = s1p_reference_impedance(circuit, block.reference_impedance_ohm);
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
            &load_side,
            frequency_hz,
            settings.show_ideal,
            settings.reference_impedance_ohm,
        )
        .map(|result| result.impedance)
        .unwrap_or(Complex::new(settings.reference_impedance_ohm, 0.0));
        let source_reference = block
            .port_reference_impedances_ohm
            .first()
            .copied()
            .unwrap_or(block.reference_impedance_ohm);
        let load_reference = block
            .port_reference_impedances_ohm
            .get(1)
            .copied()
            .unwrap_or(block.reference_impedance_ohm);
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
fn solve_stability_circles(
    circuit: &[SmithChartElement],
    settings: &SmithChartSettings,
    active_frequency_hz: f64,
) -> Vec<StabilityCircle> {
    let Some(block) = circuit.iter().find_map(|element| match element {
        SmithChartElement::SParameter(block) if block.kind == SParameterKind::S2P => Some(block),
        _ => None,
    }) else {
        return Vec::new();
    };

    let frequencies = frequency_samples(circuit, settings, active_frequency_hz);
    let mut out = Vec::new();
    for frequency_hz in frequencies {
        let Some(point) = block.interpolate(frequency_hz) else {
            continue;
        };
        let Some(s12) = point.s12 else {
            continue;
        };
        let Some(s21) = point.s21 else {
            continue;
        };
        let Some(s22) = point.s22 else {
            continue;
        };
        let s11 = point.s11;
        let delta = s11 * s22 - s12 * s21;
        let source_denominator = s11.magnitude().powi(2) - delta.magnitude().powi(2);
        let load_denominator = s22.magnitude().powi(2) - delta.magnitude().powi(2);
        if source_denominator.abs() <= f64::EPSILON || load_denominator.abs() <= f64::EPSILON {
            continue;
        }
        let product_magnitude = (s12 * s21).magnitude();
        out.push(StabilityCircle {
            frequency_hz,
            source_center: (s11 - delta * s22.conjugate()).conjugate() / source_denominator,
            source_radius: product_magnitude / source_denominator.abs(),
            load_center: (s22 - delta * s11.conjugate()).conjugate() / load_denominator,
            load_radius: product_magnitude / load_denominator.abs(),
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
        SmithChartElement::SParameter(block) if block.kind == SParameterKind::S2P => Some(block),
        _ => None,
    }) else {
        return Vec::new();
    };
    let Some(point) = block.interpolate(active_frequency_hz) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for target_gain_db in input_targets_db
        .iter()
        .copied()
        .filter(|value| value.is_finite())
    {
        if let Some(circle) = gain_circle(
            point.s11,
            GainCirclePort::Input,
            active_frequency_hz,
            target_gain_db,
        ) {
            out.push(circle);
        }
    }
    if let Some(s22) = point.s22 {
        for target_gain_db in output_targets_db
            .iter()
            .copied()
            .filter(|value| value.is_finite())
        {
            if let Some(circle) = gain_circle(
                s22,
                GainCirclePort::Output,
                active_frequency_hz,
                target_gain_db,
            ) {
                out.push(circle);
            }
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
    targets_db
        .iter()
        .copied()
        .filter_map(|target_noise_figure_db| {
            noise_figure_circle(noise, block.reference_impedance_ohm, target_noise_figure_db).map(
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
    s_parameter: Complex,
    port: GainCirclePort,
    frequency_hz: f64,
    target_gain_db: f64,
) -> Option<GainCircle> {
    let magnitude_squared = s_parameter.magnitude().powi(2);
    let denominator = 1.0 - magnitude_squared;
    if denominator.abs() <= f64::EPSILON {
        return None;
    }
    let normalized_gain = 10.0_f64.powf(target_gain_db / 10.0) * denominator;
    if normalized_gain > 1.0 {
        return None;
    }
    let circle_denominator = 1.0 - magnitude_squared * (1.0 - normalized_gain);
    if circle_denominator.abs() <= f64::EPSILON {
        return None;
    }
    let center = s_parameter.conjugate() * (normalized_gain / circle_denominator);
    let radius = (1.0 - normalized_gain).sqrt() * denominator.abs() / circle_denominator.abs();
    radius.is_finite().then_some(GainCircle {
        frequency_hz,
        port,
        target_gain_db,
        center,
        radius,
    })
}

/// Computes the noise figure circle geometry.
fn noise_figure_circle(
    noise: &NoisePoint,
    reference_impedance_ohm: f64,
    target_noise_figure_db: f64,
) -> Option<(Complex, f64)> {
    let fmin_linear = 10.0_f64.powf(noise.fmin_db / 10.0);
    let target_linear = 10.0_f64.powf(target_noise_figure_db / 10.0);
    let normalized_noise_resistance = noise.rn_ohm / reference_impedance_ohm;
    if normalized_noise_resistance <= f64::EPSILON || target_linear < fmin_linear {
        return None;
    }
    let gamma_opt = noise.optimum_gamma;
    let one_plus_gamma_squared = (gamma_opt.re + 1.0).powi(2) + gamma_opt.im * gamma_opt.im;
    let noise_parameter = ((target_linear - fmin_linear) * one_plus_gamma_squared)
        / (4.0 * normalized_noise_resistance);
    let denominator = noise_parameter + 1.0;
    if denominator <= f64::EPSILON {
        return None;
    }
    let radius_term = noise_parameter * (denominator - gamma_opt.magnitude().powi(2));
    if radius_term < 0.0 {
        return None;
    }
    Some((gamma_opt / denominator, radius_term.sqrt() / denominator))
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
        let Some(noise) = exact_noise_point_at(block, frequency_hz) else {
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
        let source_y = source_z.reciprocal().unwrap_or(Complex::ZERO);
        let delta = source_y - noise.optimum_admittance;
        let noise_factor_linear = 10.0_f64.powf(noise.fmin_db / 10.0)
            + (noise.rn_ohm / source_y.re) * delta.magnitude().powi(2);
        out.push(NoiseFigurePoint {
            frequency_hz,
            noise_factor_linear,
        });
    }
    Ok(out)
}

/// Selects active noise frequency from the available candidates.
fn select_active_noise_frequency(block: &SParameterBlock, requested_hz: f64) -> f64 {
    block
        .noise
        .iter()
        .find(|point| point.frequency_hz >= requested_hz)
        .or_else(|| block.noise.last())
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
        .noise
        .iter()
        .map(|point| point.frequency_hz)
        .filter(|frequency| *frequency >= min && *frequency <= max)
        .collect()
}

/// Computes the custom complex impedance.
fn custom_impedance(
    points: &[CustomPoint],
    frequency_hz: f64,
    interpolation: CustomInterpolation,
) -> Complex {
    if points.is_empty() {
        return Complex::ZERO;
    }
    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a.frequency_hz.total_cmp(&b.frequency_hz));
    if frequency_hz <= sorted[0].frequency_hz {
        return sorted[0].impedance;
    }
    if frequency_hz >= sorted[sorted.len() - 1].frequency_hz {
        return sorted[sorted.len() - 1].impedance;
    }
    for pair in sorted.windows(2) {
        let [left, right] = pair else {
            continue;
        };
        if frequency_hz >= left.frequency_hz && frequency_hz < right.frequency_hz {
            if interpolation == CustomInterpolation::SampleAndHold {
                return left.impedance;
            }
            let ratio =
                (frequency_hz - left.frequency_hz) / (right.frequency_hz - left.frequency_hz);
            return left.impedance + (right.impedance - left.impedance) * ratio;
        }
    }
    Complex::ZERO
}

/// Returns the noise-parameter point whose frequency exactly matches the request.
fn exact_noise_point_at(block: &SParameterBlock, frequency_hz: f64) -> Option<&NoisePoint> {
    block
        .noise
        .iter()
        .find(|point| same_number(point.frequency_hz, frequency_hz))
}

/// Creates a solve-step summary for a circuit element.
fn summary_element(element: &SmithChartElement) -> CircuitElement {
    let (name, kind, value) = match element {
        SmithChartElement::BlackBox { impedance, .. } => {
            ("Black Box", ElementKind::Load, impedance.re)
        }
        SmithChartElement::LoadTermination { impedance, .. } => {
            ("Load Termination", ElementKind::LoadTerm, impedance.re)
        }
        SmithChartElement::SeriesCapacitor { capacitance_f, .. } => (
            "Series Capacitor",
            ElementKind::SeriesCapacitor,
            *capacitance_f,
        ),
        SmithChartElement::ShuntCapacitor { capacitance_f, .. } => (
            "Shorted Capacitor",
            ElementKind::ShuntCapacitor,
            *capacitance_f,
        ),
        SmithChartElement::SeriesInductor { inductance_h, .. } => (
            "Series Inductor",
            ElementKind::SeriesInductor,
            *inductance_h,
        ),
        SmithChartElement::ShuntInductor { inductance_h, .. } => (
            "Shorted Inductor",
            ElementKind::ShuntInductor,
            *inductance_h,
        ),
        SmithChartElement::SeriesResistor { resistance_ohm, .. } => (
            "Series Resistor",
            ElementKind::SeriesResistor,
            *resistance_ohm,
        ),
        SmithChartElement::ShuntResistor { resistance_ohm, .. } => (
            "Shorted Resistor",
            ElementKind::ShuntResistor,
            *resistance_ohm,
        ),
        SmithChartElement::SeriesParallelRlc { resistance_ohm, .. } => {
            ("Parallel RLC", ElementKind::SeriesRlc, *resistance_ohm)
        }
        SmithChartElement::Custom { .. } => ("Custom Z(f)", ElementKind::Custom, 0.0),
        SmithChartElement::TransmissionLine { length_m, .. } => (
            "Transmission Line",
            ElementKind::TransmissionLine,
            *length_m,
        ),
        SmithChartElement::OpenStub { length_m, .. } => ("Stub", ElementKind::OpenStub, *length_m),
        SmithChartElement::ShortedStub { length_m, .. } => {
            ("Shorted Stub", ElementKind::ShortedStub, *length_m)
        }
        SmithChartElement::Transformer { model, l1_h, .. } => match model {
            TransformerModel::Ideal => ("Ideal Transformer", ElementKind::IdealTransformer, 0.0),
            TransformerModel::CoupledInductor => {
                ("Transformer", ElementKind::CoupledTransformer, *l1_h)
            }
        },
        SmithChartElement::SParameter(_) => ("S-Parameter", ElementKind::SParameter, 0.0),
    };
    CircuitElement::new(name, kind, value)
}

/// Computes electrical length in radians at the supplied frequency.
fn electrical_length_rad_at(length_m: f64, frequency_hz: f64, effective_dielectric: f64) -> f64 {
    TAU * frequency_hz * length_m * effective_dielectric.sqrt() / SPEED_OF_LIGHT_M_PER_S
}

/// Transforms a load through a transmission line with an effective dielectric.
fn transmission_line_with_dielectric(
    load: Complex,
    length_m: f64,
    characteristic_impedance_ohm: f64,
    frequency_hz: f64,
    effective_dielectric: f64,
) -> Result<Complex, SolveError> {
    let z0 = Complex::new(characteristic_impedance_ohm, 0.0);
    let beta_l = electrical_length_rad_at(length_m, frequency_hz, effective_dielectric);
    let j_tan = Complex::new(0.0, beta_l.tan());
    let denominator = z0 + load * j_tan;
    if denominator.magnitude() <= f64::EPSILON {
        return Err(SolveError::SingularNetwork {
            kind: ElementKind::TransmissionLine,
        });
    }
    Ok(z0 * ((load + z0 * j_tan) / denominator))
}

/// Opens stub impedance at for the requested workflow.
fn open_stub_impedance_at(
    length_m: f64,
    characteristic_impedance_ohm: f64,
    frequency_hz: f64,
    effective_dielectric: f64,
) -> Result<Complex, SolveError> {
    let tan = electrical_length_rad_at(length_m, frequency_hz, effective_dielectric).tan();
    if tan.abs() <= f64::EPSILON {
        return Err(SolveError::SingularNetwork {
            kind: ElementKind::OpenStub,
        });
    }
    Ok(Complex::new(0.0, -characteristic_impedance_ohm / tan))
}

/// Computes shorted-stub impedance at the supplied frequency.
fn shorted_stub_impedance_at(
    length_m: f64,
    characteristic_impedance_ohm: f64,
    frequency_hz: f64,
    effective_dielectric: f64,
) -> Complex {
    Complex::new(
        0.0,
        characteristic_impedance_ohm
            * electrical_length_rad_at(length_m, frequency_hz, effective_dielectric).tan(),
    )
}

/// Applies element and returns the resulting value.
fn apply_element(
    impedance: Complex,
    element: &CircuitElement,
    settings: SolveSettings,
) -> Result<Complex, SolveError> {
    let omega = TAU * settings.frequency_hz;
    let kind = element.kind;
    let value = element.value;
    let result = match kind {
        ElementKind::Load => impedance,
        ElementKind::SeriesResistor => impedance + Complex::new(value, 0.0),
        ElementKind::ShuntResistor => shunt(impedance, Complex::new(value, 0.0))?,
        ElementKind::SeriesCapacitor => impedance + Complex::new(0.0, -1.0 / (omega * value)),
        ElementKind::ShuntCapacitor => shunt(impedance, Complex::new(0.0, -1.0 / (omega * value)))?,
        ElementKind::SeriesInductor => impedance + Complex::new(0.0, omega * value),
        ElementKind::ShuntInductor => shunt(impedance, Complex::new(0.0, omega * value))?,
        ElementKind::TransmissionLine => {
            transmission_line(impedance, value, settings.reference_impedance_ohm, settings)?
        }
        ElementKind::OpenStub => {
            let stub = open_stub_impedance(value, settings.reference_impedance_ohm, settings)?;
            shunt(impedance, stub)?
        }
        ElementKind::ShortedStub => {
            let stub = shorted_stub_impedance(value, settings.reference_impedance_ohm, settings)?;
            shunt(impedance, stub)?
        }
        ElementKind::SeriesRlc
        | ElementKind::Custom
        | ElementKind::IdealTransformer
        | ElementKind::CoupledTransformer
        | ElementKind::SParameter
        | ElementKind::LoadTerm => impedance,
    };
    Ok(result)
}

/// Combines an impedance with a shunt branch in parallel.
fn shunt(a: Complex, b: Complex) -> Result<Complex, SolveError> {
    let sum = a + b;
    if sum.magnitude() <= f64::EPSILON {
        return Err(SolveError::SingularNetwork {
            kind: ElementKind::ShuntResistor,
        });
    }
    Ok((a * b) / sum)
}

/// Converts physical line length to electrical length in radians.
fn electrical_length_rad(length_m: f64, settings: SolveSettings) -> f64 {
    TAU * settings.frequency_hz * length_m / (SPEED_OF_LIGHT_M_PER_S * settings.velocity_factor)
}

/// Transforms a load impedance through a lossless transmission line.
fn transmission_line(
    load: Complex,
    length_m: f64,
    characteristic_impedance_ohm: f64,
    settings: SolveSettings,
) -> Result<Complex, SolveError> {
    let z0 = Complex::new(characteristic_impedance_ohm, 0.0);
    let beta_l = electrical_length_rad(length_m, settings);
    let j_tan = Complex::new(0.0, beta_l.tan());
    let denominator = z0 + load * j_tan;
    if denominator.magnitude() <= f64::EPSILON {
        return Err(SolveError::SingularNetwork {
            kind: ElementKind::TransmissionLine,
        });
    }
    Ok(z0 * ((load + z0 * j_tan) / denominator))
}

/// Opens stub impedance for the requested workflow.
fn open_stub_impedance(
    length_m: f64,
    characteristic_impedance_ohm: f64,
    settings: SolveSettings,
) -> Result<Complex, SolveError> {
    let tan = electrical_length_rad(length_m, settings).tan();
    if tan.abs() <= f64::EPSILON {
        return Err(SolveError::SingularNetwork {
            kind: ElementKind::OpenStub,
        });
    }
    Ok(Complex::new(0.0, -characteristic_impedance_ohm / tan))
}

/// Computes the shorted stub complex impedance.
fn shorted_stub_impedance(
    length_m: f64,
    characteristic_impedance_ohm: f64,
    settings: SolveSettings,
) -> Result<Complex, SolveError> {
    Ok(Complex::new(
        0.0,
        characteristic_impedance_ohm * electrical_length_rad(length_m, settings).tan(),
    ))
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
