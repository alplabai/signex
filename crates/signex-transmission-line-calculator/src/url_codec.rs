use crate::url_numbers::parse_url_number;
use crate::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub fn serialize_online_smith_chart_query(
    circuit: &[SmithChartElement],
    settings: &SmithChartSettings,
    overlays: &SmithChartOverlays,
) -> String {
    let default_settings = SmithChartSettings::default();
    let mut params = Vec::new();
    if !same_number(settings.frequency_hz, default_settings.frequency_hz) {
        params.push((
            "frequency".to_string(),
            encode_component(&format_number(
                settings.frequency_hz / settings.frequency_unit.multiplier(),
            )),
        ));
    }
    if settings.frequency_unit != default_settings.frequency_unit {
        params.push((
            "frequencyUnit".to_string(),
            settings
                .frequency_unit
                .online_smith_chart_frequency_symbol()
                .unwrap_or("MHz")
                .to_string(),
        ));
    }
    if !same_number(
        settings.reference_impedance_ohm,
        default_settings.reference_impedance_ohm,
    ) {
        params.push((
            "zo".to_string(),
            encode_component(&format_number(settings.reference_impedance_ohm)),
        ));
    }
    if !same_number(settings.span_hz, default_settings.span_hz) {
        params.push((
            "fSpan".to_string(),
            encode_component(&format_number(
                settings.span_hz / settings.span_unit.multiplier(),
            )),
        ));
    }
    if settings.span_unit != default_settings.span_unit {
        params.push((
            "fSpanUnit".to_string(),
            settings
                .span_unit
                .online_smith_chart_frequency_symbol()
                .unwrap_or("MHz")
                .to_string(),
        ));
    }
    if settings.resolution != default_settings.resolution {
        params.push(("fRes".to_string(), settings.resolution.to_string()));
    }
    if !is_online_smith_chart_default_circuit(circuit) {
        let circuit_value = circuit
            .iter()
            .map(serialize_smith_chart_element_token)
            .collect::<Vec<_>>()
            .join("__");
        params.push(("circuit".to_string(), encode_component(&circuit_value)));
    }
    push_scalar_list_param(&mut params, "vswrCircles", &overlays.vswr_circles);
    push_scalar_list_param(&mut params, "qCircles", &overlays.q_circles);
    push_scalar_list_param(&mut params, "gainInCircles", &overlays.gain_input_circles);
    push_scalar_list_param(&mut params, "gainOutCircles", &overlays.gain_output_circles);
    push_scalar_list_param(&mut params, "nfCircles", &overlays.noise_figure_circles);
    if !overlays.z_markers.is_empty() {
        params.push((
            "zMarkers".to_string(),
            overlays
                .z_markers
                .iter()
                .map(|marker| format!("{}_{}", format_number(marker.re), format_number(marker.im)))
                .collect::<Vec<_>>()
                .join("__"),
        ));
    }
    params
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn is_online_smith_chart_default_circuit(circuit: &[SmithChartElement]) -> bool {
    match circuit {
        [SmithChartElement::BlackBox { impedance, .. }] => {
            same_number(impedance.re, DEFAULT_REFERENCE_IMPEDANCE_OHM)
                && same_number(impedance.im, 0.0)
        }
        _ => false,
    }
}

pub fn parse_online_smith_chart_query(query: &str) -> Result<SmithChartSnapshot, UrlCodecError> {
    let params = parse_query_params(query);
    let has_legacy_scalar_keys = query_param(&params, "reference").is_some()
        || query_param(&params, "span").is_some()
        || query_param(&params, "resolution").is_some();
    let frequency_unit =
        parse_query_frequency_unit(&params, "frequencyUnit", ScalarUnit::MegaHertz)?;
    let span_unit = parse_query_frequency_unit(&params, "fSpanUnit", ScalarUnit::MegaHertz)?;
    let frequency_hz = if query_param(&params, "frequencyUnit").is_some() {
        parse_query_f64_with_unit(
            &params,
            "frequency",
            "frequencyUnit",
            ScalarUnit::MegaHertz.multiplier(),
            SmithChartSettings::default().frequency_hz,
        )?
    } else if has_legacy_scalar_keys {
        parse_query_f64(
            &params,
            "frequency",
            SmithChartSettings::default().frequency_hz,
        )?
    } else {
        parse_query_f64_with_unit(
            &params,
            "frequency",
            "frequencyUnit",
            ScalarUnit::MegaHertz.multiplier(),
            SmithChartSettings::default().frequency_hz,
        )?
    };
    let settings = SmithChartSettings {
        frequency_hz,
        frequency_unit,
        reference_impedance_ohm: parse_query_f64_alias(
            &params,
            "zo",
            "reference",
            SmithChartSettings::default().reference_impedance_ohm,
        )?,
        span_hz: if query_param(&params, "fSpan").is_some()
            || query_param(&params, "fSpanUnit").is_some()
        {
            parse_query_f64_with_unit(
                &params,
                "fSpan",
                "fSpanUnit",
                ScalarUnit::MegaHertz.multiplier(),
                SmithChartSettings::default().span_hz,
            )?
        } else {
            parse_query_f64(&params, "span", SmithChartSettings::default().span_hz)?
        },
        span_unit,
        resolution: parse_query_usize_alias(
            &params,
            "fRes",
            "resolution",
            SmithChartSettings::default().resolution,
        )?,
        show_ideal: SmithChartSettings::default().show_ideal,
    };
    let circuit = if let Some((_, value)) = params.iter().find(|(key, _)| key == "circuit") {
        let initial_circuit = normalize_online_smith_chart_query_circuit(
            parse_online_smith_chart_circuit_tokens_at_frequency(value, frequency_hz)?,
        );
        let active_frequency_hz = select_active_frequency(&initial_circuit, frequency_hz);
        if (active_frequency_hz - frequency_hz).abs() > f64::EPSILON {
            normalize_online_smith_chart_query_circuit(
                parse_online_smith_chart_circuit_tokens_at_frequency(value, active_frequency_hz)?,
            )
        } else {
            initial_circuit
        }
    } else {
        vec![SmithChartElement::BlackBox {
            impedance: Complex::new(DEFAULT_REFERENCE_IMPEDANCE_OHM, 0.0),
            tolerance_percent: None,
        }]
    };
    let overlays = SmithChartOverlays {
        z_markers: parse_query_markers(&params, "zMarkers")?,
        vswr_circles: parse_query_scalar_list(&params, "vswrCircles")?,
        q_circles: parse_query_scalar_list(&params, "qCircles")?,
        noise_figure_circles: parse_query_scalar_list(&params, "nfCircles")?,
        gain_input_circles: parse_query_scalar_list(&params, "gainInCircles")?,
        gain_output_circles: parse_query_scalar_list(&params, "gainOutCircles")?,
    };
    Ok(SmithChartSnapshot {
        circuit,
        settings,
        overlays,
    })
}

fn normalize_online_smith_chart_query_circuit(
    circuit: Vec<SmithChartElement>,
) -> Vec<SmithChartElement> {
    let Some(s1p_index) = circuit.iter().position(|element| {
        matches!(
            element,
            SmithChartElement::SParameter(SParameterBlock {
                kind: SParameterKind::S1P,
                ..
            })
        )
    }) else {
        return circuit;
    };
    if s1p_index == 0 {
        return circuit;
    }

    let black_box = circuit.iter().find_map(|element| match element {
        SmithChartElement::BlackBox {
            impedance,
            tolerance_percent,
        } => Some((*impedance, *tolerance_percent)),
        _ => None,
    });
    let load = circuit
        .iter()
        .find(|element| matches!(element, SmithChartElement::LoadTermination { .. }))
        .cloned()
        .unwrap_or_else(|| {
            let (impedance, tolerance_percent) =
                black_box.unwrap_or((Complex::new(DEFAULT_REFERENCE_IMPEDANCE_OHM, 0.0), None));
            SmithChartElement::LoadTermination {
                impedance,
                tolerance_percent,
            }
        });
    let sparam = circuit[s1p_index].clone();
    let mut matching_elements = circuit
        .iter()
        .enumerate()
        .filter(|(index, element)| {
            *index != s1p_index
                && !matches!(
                    element,
                    SmithChartElement::BlackBox { .. } | SmithChartElement::LoadTermination { .. }
                )
        })
        .map(|(_, element)| element.clone())
        .collect::<Vec<_>>();
    matching_elements.reverse();

    std::iter::once(sparam)
        .chain(matching_elements)
        .chain(std::iter::once(load))
        .collect()
}

pub fn serialize_online_smith_chart_circuit_tokens(circuit: &[SmithChartElement]) -> String {
    circuit
        .iter()
        .map(serialize_smith_chart_element_token)
        .collect::<Vec<_>>()
        .join("__")
}

pub fn parse_online_smith_chart_circuit_tokens(
    value: &str,
) -> Result<Vec<SmithChartElement>, UrlCodecError> {
    parse_online_smith_chart_circuit_tokens_at_frequency(
        value,
        SmithChartSettings::default().frequency_hz,
    )
}

pub fn split_online_smith_chart_circuit_tokens(value: &str) -> Vec<&str> {
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
            let (value, unit) = format_online_smith_chart_capacitance(*capacitance_f);
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
            let (value, unit) = format_online_smith_chart_capacitance(*capacitance_f);
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
            let (value, unit) = format_online_smith_chart_inductance(*inductance_h);
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
            let (value, unit) = format_online_smith_chart_inductance(*inductance_h);
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
            let (value, unit) = format_online_smith_chart_resistance(*resistance_ohm);
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
            let (value, unit) = format_online_smith_chart_resistance(*resistance_ohm);
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
            let (resistance_value, resistance_unit) =
                format_online_smith_chart_resistance(*resistance_ohm);
            let (inductance_value, inductance_unit) =
                format_online_smith_chart_inductance(*inductance_h);
            let (capacitance_value, capacitance_unit) =
                format_online_smith_chart_capacitance(*capacitance_f);
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
                    CustomPointUrlValue {
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
            let (value, unit) = format_online_smith_chart_length(*length_m);
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
            let (value, unit) = format_online_smith_chart_length(*length_m);
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
            let (value, unit) = format_online_smith_chart_length(*length_m);
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
            let (l1_value, l1_unit) = format_online_smith_chart_inductance(*l1_h);
            let (l2_value, l2_unit) = format_online_smith_chart_inductance(*l2_h);
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

fn parse_online_smith_chart_circuit_tokens_at_frequency(
    value: &str,
    frequency_hz: f64,
) -> Result<Vec<SmithChartElement>, UrlCodecError> {
    split_online_smith_chart_circuit_tokens(value)
        .into_iter()
        .map(|token| parse_smith_chart_element_token_at_frequency(token, frequency_hz))
        .collect()
}

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

fn parse_smith_chart_element_token_at_frequency(
    value: &str,
    frequency_hz: f64,
) -> Result<SmithChartElement, UrlCodecError> {
    let fields = value.split('_').collect::<Vec<_>>();
    let Some(kind) = fields.first().copied() else {
        return Err(UrlCodecError::InvalidCircuitToken {
            token: value.to_string(),
        });
    };
    match kind {
        "blackBox" => Ok(SmithChartElement::BlackBox {
            impedance: Complex::new(
                parse_token_f64(value, &fields, 1)?,
                parse_token_f64(value, &fields, 2)?,
            ),
            tolerance_percent: parse_token_optional(value, &fields, 3)?,
        }),
        "loadTerm" => Ok(SmithChartElement::LoadTermination {
            impedance: Complex::new(
                parse_token_f64(value, &fields, 1)?,
                parse_token_f64(value, &fields, 2)?,
            ),
            tolerance_percent: parse_token_optional(value, &fields, 3)?,
        }),
        "seriesCap" => Ok(SmithChartElement::SeriesCapacitor {
            capacitance_f: parse_unit_value(value, &fields, 1, 2)?,
            tolerance_percent: parse_token_optional(value, &fields, 3)?,
            esr_ohm: parse_token_f64(value, &fields, 4).unwrap_or(0.0),
            esl_h: parse_token_f64(value, &fields, 5).unwrap_or(0.0),
        }),
        "shortedCap" => Ok(SmithChartElement::ShuntCapacitor {
            capacitance_f: parse_unit_value(value, &fields, 1, 2)?,
            tolerance_percent: parse_token_optional(value, &fields, 3)?,
            esr_ohm: parse_token_f64(value, &fields, 4).unwrap_or(0.0),
            esl_h: parse_token_f64(value, &fields, 5).unwrap_or(0.0),
        }),
        "seriesInd" => Ok(SmithChartElement::SeriesInductor {
            inductance_h: parse_unit_value(value, &fields, 1, 2)?,
            tolerance_percent: parse_token_optional(value, &fields, 3)?,
            esr_ohm: parse_token_f64(value, &fields, 4).unwrap_or(0.0),
        }),
        "shortedInd" => Ok(SmithChartElement::ShuntInductor {
            inductance_h: parse_unit_value(value, &fields, 1, 2)?,
            tolerance_percent: parse_token_optional(value, &fields, 3)?,
            esr_ohm: parse_token_f64(value, &fields, 4).unwrap_or(0.0),
        }),
        "seriesRes" => Ok(SmithChartElement::SeriesResistor {
            resistance_ohm: parse_unit_value(value, &fields, 1, 2)?,
            tolerance_percent: parse_token_optional(value, &fields, 3)?,
            esl_h: parse_token_f64(value, &fields, 4).unwrap_or(0.0),
        }),
        "shortedRes" => Ok(SmithChartElement::ShuntResistor {
            resistance_ohm: parse_unit_value(value, &fields, 1, 2)?,
            tolerance_percent: parse_token_optional(value, &fields, 3)?,
            esl_h: parse_token_f64(value, &fields, 4).unwrap_or(0.0),
        }),
        "seriesRlc" => Ok(SmithChartElement::SeriesParallelRlc {
            resistance_ohm: parse_unit_value(value, &fields, 1, 2)?,
            inductance_h: parse_unit_value(value, &fields, 3, 4)?,
            capacitance_f: parse_unit_value(value, &fields, 5, 6)?,
        }),
        "custom" | "customZ" => {
            let interpolation = match fields.get(1).copied().unwrap_or("linear") {
                "sah" | "sample" | "sampleAndHold" | "stepped" => {
                    CustomInterpolation::SampleAndHold
                }
                _ => CustomInterpolation::Linear,
            };
            let raw_points =
                decode_component(fields.get(2).copied().unwrap_or_default()).map_err(|_| {
                    UrlCodecError::InvalidCircuitToken {
                        token: value.to_string(),
                    }
                })?;
            let points = if kind == "custom" {
                parse_custom_json_points(value, &raw_points)?
            } else {
                parse_legacy_custom_list_points(value, &raw_points)?
            };
            Ok(SmithChartElement::Custom {
                points,
                interpolation,
            })
        }
        "transmissionLine" => Ok(SmithChartElement::TransmissionLine {
            length_m: parse_length_value(value, &fields, 1, 2, 5, frequency_hz)?,
            tolerance_percent: parse_token_optional(value, &fields, 3)?,
            characteristic_impedance_ohm: parse_token_f64(value, &fields, 4)?,
            effective_dielectric: parse_token_f64(value, &fields, 5).unwrap_or(1.0),
        }),
        "stub" => Ok(SmithChartElement::OpenStub {
            length_m: parse_length_value(value, &fields, 1, 2, 5, frequency_hz)?,
            tolerance_percent: parse_token_optional(value, &fields, 3)?,
            characteristic_impedance_ohm: parse_token_f64(value, &fields, 4)?,
            effective_dielectric: parse_token_f64(value, &fields, 5).unwrap_or(1.0),
        }),
        "shortedStub" => Ok(SmithChartElement::ShortedStub {
            length_m: parse_length_value(value, &fields, 1, 2, 5, frequency_hz)?,
            tolerance_percent: parse_token_optional(value, &fields, 3)?,
            characteristic_impedance_ohm: parse_token_f64(value, &fields, 4)?,
            effective_dielectric: parse_token_f64(value, &fields, 5).unwrap_or(1.0),
        }),
        "transformer" => {
            if fields.len() >= 7 {
                Ok(SmithChartElement::Transformer {
                    l1_h: parse_unit_value(value, &fields, 1, 2)?,
                    l2_h: parse_unit_value(value, &fields, 3, 4)?,
                    coupling_or_turns_ratio: parse_token_f64(value, &fields, 5)?,
                    model: match fields.get(6).copied().unwrap_or("coupledInductor") {
                        "ideal" => TransformerModel::Ideal,
                        _ => TransformerModel::CoupledInductor,
                    },
                })
            } else {
                match fields.get(1).copied().unwrap_or("ideal") {
                    "coupled" | "coupledInductor" => Ok(SmithChartElement::Transformer {
                        model: TransformerModel::CoupledInductor,
                        l1_h: parse_token_f64(value, &fields, 2)?,
                        l2_h: parse_token_f64(value, &fields, 3)?,
                        coupling_or_turns_ratio: parse_token_f64(value, &fields, 4)?,
                    }),
                    _ => Ok(SmithChartElement::Transformer {
                        model: TransformerModel::Ideal,
                        coupling_or_turns_ratio: parse_token_f64(value, &fields, 2)?,
                        l1_h: parse_token_f64(value, &fields, 3).unwrap_or(0.0),
                        l2_h: parse_token_f64(value, &fields, 4).unwrap_or(0.0),
                    }),
                }
            }
        }
        "sparam" => parse_s_parameter_token(value, &fields).map(SmithChartElement::SParameter),
        _ => Err(UrlCodecError::InvalidCircuitToken {
            token: value.to_string(),
        }),
    }
}

fn parse_custom_json_points(
    token: &str,
    raw_points: &str,
) -> Result<Vec<CustomPoint>, UrlCodecError> {
    let values = serde_json::from_str::<BTreeMap<String, CustomPointUrlValue>>(raw_points)
        .map_err(|_| UrlCodecError::InvalidCircuitToken {
            token: token.to_string(),
        })?;
    values
        .into_iter()
        .map(|(frequency_hz, point)| {
            Ok(CustomPoint {
                frequency_hz: frequency_hz.parse::<f64>().map_err(|_| {
                    UrlCodecError::InvalidCircuitToken {
                        token: token.to_string(),
                    }
                })?,
                impedance: Complex::new(point.real, point.imaginary),
            })
        })
        .collect()
}

fn parse_legacy_custom_list_points(
    token: &str,
    raw_points: &str,
) -> Result<Vec<CustomPoint>, UrlCodecError> {
    raw_points
        .split(';')
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| {
            let parts = entry.split(',').collect::<Vec<_>>();
            if parts.len() != 3 {
                return Err(UrlCodecError::InvalidCircuitToken {
                    token: token.to_string(),
                });
            }
            Ok(CustomPoint {
                frequency_hz: parts[0].parse::<f64>().map_err(|_| {
                    UrlCodecError::InvalidCircuitToken {
                        token: token.to_string(),
                    }
                })?,
                impedance: Complex::new(
                    parts[1]
                        .parse::<f64>()
                        .map_err(|_| UrlCodecError::InvalidCircuitToken {
                            token: token.to_string(),
                        })?,
                    parts[2]
                        .parse::<f64>()
                        .map_err(|_| UrlCodecError::InvalidCircuitToken {
                            token: token.to_string(),
                        })?,
                ),
            })
        })
        .collect()
}

fn serialize_s_parameter_token(block: &SParameterBlock) -> String {
    let kind = match block.kind {
        SParameterKind::S1P => "s1p",
        SParameterKind::S2P => "s2p",
    };
    let freq_unit = block
        .source_frequency_unit
        .online_smith_chart_frequency_symbol()
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

fn parse_s_parameter_token(token: &str, fields: &[&str]) -> Result<SParameterBlock, UrlCodecError> {
    if fields.get(4).copied() == Some("tooLong") || fields.get(2).copied() == Some("tooLong") {
        return match fields.get(1).copied() {
            Some("s1p") => Ok(default_s1p_block()),
            Some("s2p") => Ok(default_s2p_block()),
            _ => Err(UrlCodecError::UnsupportedSParameterPayload),
        };
    }
    if fields.len() <= 3 {
        let raw = fields.get(2).copied().unwrap_or_default();
        let decoded = decode_component(raw).map_err(|_| UrlCodecError::InvalidCircuitToken {
            token: token.to_string(),
        })?;
        return parse_touchstone(&decoded).map_err(|error| UrlCodecError::TouchstoneParseFailed {
            reason: error.to_string(),
        });
    }
    let kind = fields.get(1).copied().unwrap_or_default();
    let freq_unit = fields.get(2).copied().unwrap_or("MHz");
    let reference = parse_token_f64(token, fields, 3)?;
    let mut raw = format!("# {freq_unit} S MA R {}", format_number(reference));
    match kind {
        "s1p" => {
            let mut index = 4;
            while index < fields.len() {
                if index + 2 >= fields.len() {
                    return Err(UrlCodecError::InvalidCircuitToken {
                        token: token.to_string(),
                    });
                }
                write!(
                    raw,
                    "\n{} {} {}",
                    fields[index],
                    format_number(parse_token_f64(token, fields, index + 1)?),
                    format_number(parse_token_f64(token, fields, index + 2)?)
                )
                .ok();
                index += 3;
            }
        }
        "s2p" => {
            let mut index = 4;
            while index < fields.len() {
                if fields[index] == "noise" {
                    break;
                }
                if index + 8 >= fields.len() {
                    return Err(UrlCodecError::InvalidCircuitToken {
                        token: token.to_string(),
                    });
                }
                write!(
                    raw,
                    "\n{} {} {} {} {} {} {} {} {}",
                    fields[index],
                    format_number(parse_token_f64(token, fields, index + 1)?),
                    format_number(parse_token_f64(token, fields, index + 2)?),
                    format_number(parse_token_f64(token, fields, index + 3)?),
                    format_number(parse_token_f64(token, fields, index + 4)?),
                    format_number(parse_token_f64(token, fields, index + 5)?),
                    format_number(parse_token_f64(token, fields, index + 6)?),
                    format_number(parse_token_f64(token, fields, index + 7)?),
                    format_number(parse_token_f64(token, fields, index + 8)?)
                )
                .ok();
                index += 9;
            }
            if fields.get(index).copied() == Some("noise") {
                raw.push_str("\n! Noise parameters");
                index += 1;
                while index < fields.len() {
                    if index + 4 >= fields.len() {
                        return Err(UrlCodecError::InvalidCircuitToken {
                            token: token.to_string(),
                        });
                    }
                    let rn_ohm = parse_token_f64(token, fields, index + 4)?;
                    write!(
                        raw,
                        "\n{} {} {} {} {}",
                        fields[index],
                        format_number(parse_token_f64(token, fields, index + 1)?),
                        format_number(parse_token_f64(token, fields, index + 2)?),
                        format_number(parse_token_f64(token, fields, index + 3)?),
                        format_number(rn_ohm / reference)
                    )
                    .ok();
                    index += 5;
                }
            }
        }
        _ => {
            return Err(UrlCodecError::InvalidCircuitToken {
                token: token.to_string(),
            });
        }
    }
    parse_touchstone(&raw).map_err(|error| UrlCodecError::TouchstoneParseFailed {
        reason: error.to_string(),
    })
}

fn token<const N: usize>(kind: &str, fields: [String; N]) -> String {
    std::iter::once(kind.to_string())
        .chain(fields)
        .collect::<Vec<_>>()
        .join("_")
}

fn parse_query_params(query: &str) -> Vec<(String, String)> {
    query
        .trim_start_matches('?')
        .split('&')
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            let key = decode_component(key).ok()?;
            let value = decode_component(value).ok()?;
            Some((key, value))
        })
        .collect()
}

fn query_param<'a>(params: &'a [(String, String)], name: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

fn parse_query_f64(
    params: &[(String, String)],
    name: &str,
    default: f64,
) -> Result<f64, UrlCodecError> {
    match query_param(params, name) {
        Some(value) => parse_url_number(value).map_err(|_| UrlCodecError::InvalidParameter {
            name: name.to_string(),
            value: value.to_string(),
        }),
        None => Ok(default),
    }
}

fn parse_query_f64_alias(
    params: &[(String, String)],
    primary_name: &str,
    legacy_name: &str,
    default: f64,
) -> Result<f64, UrlCodecError> {
    if query_param(params, primary_name).is_some() {
        parse_query_f64(params, primary_name, default)
    } else {
        parse_query_f64(params, legacy_name, default)
    }
}

fn parse_query_f64_with_unit(
    params: &[(String, String)],
    value_name: &str,
    unit_name: &str,
    default_multiplier: f64,
    default: f64,
) -> Result<f64, UrlCodecError> {
    let value = parse_query_f64(params, value_name, default / default_multiplier)?;
    let multiplier = query_param(params, unit_name)
        .map(query_frequency_unit_multiplier)
        .transpose()?
        .unwrap_or(default_multiplier);
    Ok(value * multiplier)
}

fn parse_query_frequency_unit(
    params: &[(String, String)],
    unit_name: &str,
    default: ScalarUnit,
) -> Result<ScalarUnit, UrlCodecError> {
    match query_param(params, unit_name) {
        Some(unit) => parse_online_smith_chart_frequency_unit(unit),
        None => Ok(default),
    }
}

fn parse_query_usize(
    params: &[(String, String)],
    name: &str,
    default: usize,
) -> Result<usize, UrlCodecError> {
    match query_param(params, name) {
        Some(value) => {
            if value.trim().is_empty() {
                return Ok(0);
            }
            value
                .parse::<usize>()
                .map_err(|_| UrlCodecError::InvalidParameter {
                    name: name.to_string(),
                    value: value.to_string(),
                })
        }
        None => Ok(default),
    }
}

fn parse_query_usize_alias(
    params: &[(String, String)],
    primary_name: &str,
    legacy_name: &str,
    default: usize,
) -> Result<usize, UrlCodecError> {
    if query_param(params, primary_name).is_some() {
        parse_query_usize(params, primary_name, default)
    } else {
        parse_query_usize(params, legacy_name, default)
    }
}

fn query_frequency_unit_multiplier(unit: &str) -> Result<f64, UrlCodecError> {
    Ok(parse_online_smith_chart_frequency_unit(unit)?.multiplier())
}

fn parse_online_smith_chart_frequency_unit(unit: &str) -> Result<ScalarUnit, UrlCodecError> {
    match unit {
        "Hz" => Ok(ScalarUnit::Hertz),
        "KHz" | "kHz" => Ok(ScalarUnit::KiloHertz),
        "MHz" => Ok(ScalarUnit::MegaHertz),
        "GHz" => Ok(ScalarUnit::GigaHertz),
        "THz" => Ok(ScalarUnit::TeraHertz),
        _ => Err(UrlCodecError::InvalidParameter {
            name: "frequency unit".to_string(),
            value: unit.to_string(),
        }),
    }
}

fn parse_query_scalar_list(
    params: &[(String, String)],
    name: &str,
) -> Result<Vec<f64>, UrlCodecError> {
    match params.iter().find(|(key, _)| key == name) {
        Some((_, value)) if value.trim().is_empty() => Ok(Vec::new()),
        Some((_, value)) => value
            .split('_')
            .map(|entry| {
                parse_url_number(entry).map_err(|_| UrlCodecError::InvalidParameter {
                    name: name.to_string(),
                    value: value.clone(),
                })
            })
            .collect(),
        None => Ok(Vec::new()),
    }
}

fn parse_query_markers(
    params: &[(String, String)],
    name: &str,
) -> Result<Vec<Complex>, UrlCodecError> {
    match params.iter().find(|(key, _)| key == name) {
        Some((_, value)) if value.trim().is_empty() => Ok(Vec::new()),
        Some((_, value)) => value
            .split("__")
            .filter(|entry| !entry.trim().is_empty())
            .map(|entry| {
                let fields = entry.split('_').collect::<Vec<_>>();
                if fields.len() != 2 {
                    return Err(UrlCodecError::InvalidParameter {
                        name: name.to_string(),
                        value: value.clone(),
                    });
                }
                let real =
                    parse_url_number(fields[0]).map_err(|_| UrlCodecError::InvalidParameter {
                        name: name.to_string(),
                        value: value.clone(),
                    })?;
                let imaginary =
                    parse_url_number(fields[1]).map_err(|_| UrlCodecError::InvalidParameter {
                        name: name.to_string(),
                        value: value.clone(),
                    })?;
                Ok(Complex::new(real, imaginary))
            })
            .collect(),
        None => Ok(Vec::new()),
    }
}

fn push_scalar_list_param(params: &mut Vec<(String, String)>, name: &str, values: &[f64]) {
    if values.is_empty() {
        return;
    }
    params.push((
        name.to_string(),
        values
            .iter()
            .map(|value| format_number(*value))
            .collect::<Vec<_>>()
            .join("_"),
    ));
}

fn parse_token_f64(token: &str, fields: &[&str], index: usize) -> Result<f64, UrlCodecError> {
    let value = fields
        .get(index)
        .ok_or_else(|| UrlCodecError::InvalidCircuitToken {
            token: token.to_string(),
        })?;
    parse_url_number(value).map_err(|_| UrlCodecError::InvalidCircuitToken {
        token: token.to_string(),
    })
}

fn parse_token_optional(
    token: &str,
    fields: &[&str],
    index: usize,
) -> Result<Option<f64>, UrlCodecError> {
    let Some(value) = fields.get(index).copied() else {
        return Ok(None);
    };
    if value.is_empty() || value == "-" || value == "none" {
        Ok(None)
    } else {
        let parsed = value
            .parse::<f64>()
            .map_err(|_| UrlCodecError::InvalidCircuitToken {
                token: token.to_string(),
            })?;
        if parsed == 0.0 {
            Ok(None)
        } else {
            Ok(Some(parsed))
        }
    }
}

fn parse_unit_value(
    token: &str,
    fields: &[&str],
    value_index: usize,
    unit_index: usize,
) -> Result<f64, UrlCodecError> {
    let value = parse_token_f64(token, fields, value_index)?;
    let unit = fields.get(unit_index).copied().unwrap_or_default();
    Ok(value
        * online_smith_chart_unit_multiplier(unit).ok_or_else(|| {
            UrlCodecError::InvalidCircuitToken {
                token: token.to_string(),
            }
        })?)
}

fn parse_length_value(
    token: &str,
    fields: &[&str],
    value_index: usize,
    unit_index: usize,
    effective_dielectric_index: usize,
    frequency_hz: f64,
) -> Result<f64, UrlCodecError> {
    let value = parse_token_f64(token, fields, value_index)?;
    let unit = fields.get(unit_index).copied().unwrap_or_default();
    let unit = match unit {
        "m" => ScalarUnit::Meter,
        "mm" => ScalarUnit::MilliMeter,
        "um" | "µm" => ScalarUnit::MicroMeter,
        "λ" => ScalarUnit::Wavelength,
        "deg" => ScalarUnit::Degree,
        _ => {
            return Err(UrlCodecError::InvalidCircuitToken {
                token: token.to_string(),
            });
        }
    };
    let effective_dielectric =
        parse_token_f64(token, fields, effective_dielectric_index).unwrap_or(1.0);
    length_to_meters(value, unit, frequency_hz, effective_dielectric).map_err(|_| {
        UrlCodecError::InvalidCircuitToken {
            token: token.to_string(),
        }
    })
}

fn online_smith_chart_unit_multiplier(unit: &str) -> Option<f64> {
    match unit {
        "H" => Some(ScalarUnit::Henry.multiplier()),
        "mH" => Some(ScalarUnit::MilliHenry.multiplier()),
        "uH" | "µH" => Some(ScalarUnit::MicroHenry.multiplier()),
        "nH" => Some(ScalarUnit::NanoHenry.multiplier()),
        "pH" => Some(ScalarUnit::PicoHenry.multiplier()),
        "fH" => Some(ScalarUnit::FemtoHenry.multiplier()),
        "F" => Some(ScalarUnit::Farad.multiplier()),
        "mF" => Some(ScalarUnit::MilliFarad.multiplier()),
        "uF" | "µF" => Some(ScalarUnit::MicroFarad.multiplier()),
        "nF" => Some(ScalarUnit::NanoFarad.multiplier()),
        "pF" => Some(ScalarUnit::PicoFarad.multiplier()),
        "fF" => Some(ScalarUnit::FemtoFarad.multiplier()),
        "Mohm" | "MOhm" | "MΩ" => Some(ScalarUnit::MegaOhm.multiplier()),
        "Kohm" | "kOhm" | "KΩ" | "kΩ" => Some(ScalarUnit::KiloOhm.multiplier()),
        "ohm" | "Ohm" | "Ω" => Some(ScalarUnit::Ohm.multiplier()),
        "mohm" | "mOhm" | "mΩ" => Some(ScalarUnit::MilliOhm.multiplier()),
        "m" => Some(ScalarUnit::Meter.multiplier()),
        "mm" => Some(ScalarUnit::MilliMeter.multiplier()),
        "um" | "µm" => Some(ScalarUnit::MicroMeter.multiplier()),
        _ => None,
    }
}

fn format_online_smith_chart_capacitance(value_f: f64) -> (String, String) {
    format_online_smith_chart_scaled_unit(
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

fn format_online_smith_chart_inductance(value_h: f64) -> (String, String) {
    format_online_smith_chart_scaled_unit(
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

fn format_online_smith_chart_resistance(value_ohm: f64) -> (String, String) {
    format_online_smith_chart_scaled_unit(
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

fn format_online_smith_chart_length(value_m: f64) -> (String, String) {
    format_online_smith_chart_scaled_unit(
        value_m,
        &[
            ("m", ScalarUnit::Meter.multiplier()),
            ("mm", ScalarUnit::MilliMeter.multiplier()),
            ("um", ScalarUnit::MicroMeter.multiplier()),
        ],
        "m",
    )
}

fn format_online_smith_chart_scaled_unit(
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

fn format_optional(value: Option<f64>) -> String {
    value.map(format_number).unwrap_or_else(|| "0".to_string())
}

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

pub(crate) fn same_number(left: f64, right: f64) -> bool {
    (left - right).abs() <= f64::EPSILON * left.abs().max(right.abs()).max(1.0)
}

fn encode_component(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn decode_component(value: &str) -> Result<String, UrlCodecError> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err(UrlCodecError::InvalidParameter {
                        name: "percent-encoding".to_string(),
                        value: value.to_string(),
                    });
                }
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).map_err(|_| {
                    UrlCodecError::InvalidParameter {
                        name: "percent-encoding".to_string(),
                        value: value.to_string(),
                    }
                })?;
                let decoded =
                    u8::from_str_radix(hex, 16).map_err(|_| UrlCodecError::InvalidParameter {
                        name: "percent-encoding".to_string(),
                        value: value.to_string(),
                    })?;
                out.push(decoded);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| UrlCodecError::InvalidParameter {
        name: "utf8".to_string(),
        value: value.to_string(),
    })
}
