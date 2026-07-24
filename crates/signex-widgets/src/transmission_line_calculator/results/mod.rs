use super::*;
use crate::transmission_line_calculator::{
    FrequencyScale, solve_frequency_points, solve_two_port_s_parameters,
};
use iced::widget::column;

mod csv_export_configuration;
mod estimated_s21_summary;
mod result_diagram_kind;

pub(super) use csv_export_configuration::{CSV_FREQUENCY_UNITS, CsvExportConfiguration};
pub(super) use estimated_s21_summary::EstimatedS21Summary;
pub use result_diagram_kind::ResultDiagramKind;

#[cfg(test)]
#[path = "../../../tests/transmission_line_calculator/results_tests.rs"]
mod tests;

/// Builds the result user-interface panel.
pub(super) fn result_panel(
    state: &SmithChartState,
    result: Result<SmithChartAnalysis, String>,
) -> Element<'static, SmithChartMessage> {
    match result {
        Ok(result) => {
            let estimated_s21 = estimated_s21_summary(&result);
            let frequency_unit = state.frequency_unit;
            let rows = vec![
                metric(
                    "Impedance",
                    format_complex_and_polar(result.nominal.impedance, "Ω"),
                ),
                metric(
                    "Normalized Z",
                    format_complex(result.nominal.normalized_impedance, ""),
                ),
                metric("Admittance", format_complex(result.nominal.admittance, "S")),
                metric(
                    "Reflection coefficient",
                    format_complex_and_polar(result.nominal.reflection_coefficient, ""),
                ),
                metric("Return loss", format_db(result.nominal.return_loss_db)),
                metric("VSWR", format_finite(result.nominal.vswr)),
                metric("Q", format_finite(quality_factor(result.nominal.impedance))),
                metric(
                    "Chart point",
                    format!(
                        "x {:.4}, y {:.4}",
                        result.nominal.chart_x, result.nominal.chart_y
                    ),
                ),
                metric(
                    "Tolerance solves",
                    result.tolerance_results.len().to_string(),
                ),
                metric("Sweep points", result.frequency_results.len().to_string()),
                metric(
                    "S1P traces",
                    result.s1p_reflection_variants.len().to_string(),
                ),
                metric("S2P gain points", result.s_parameter_gain.len().to_string()),
                metric(
                    "S2P gain traces",
                    result.s_parameter_gain_variants.len().to_string(),
                ),
                metric("Noise points", result.noise_figure.len().to_string()),
                metric(
                    "Estimated max S21",
                    estimated_s21
                        .as_ref()
                        .map(|summary| {
                            format!(
                                "{} at {}",
                                format_db(summary.max_db),
                                format_frequency(summary.frequency_hz, frequency_unit)
                            )
                        })
                        .unwrap_or_else(|| "n/a".to_string()),
                ),
                metric(
                    "Estimated 3 dB BW",
                    estimated_s21
                        .and_then(|summary| summary.bandwidth_hz)
                        .map(|bandwidth_hz| format_frequency(bandwidth_hz, frequency_unit))
                        .unwrap_or_else(|| "n/a".to_string()),
                ),
                metric(
                    "Stability circles",
                    result.stability_circles.len().to_string(),
                ),
                metric(
                    "Active frequency",
                    format_frequency(result.active_frequency_hz, frequency_unit),
                ),
            ];
            section("Final Results", rows)
        }
        Err(err) => container(text(err).size(13))
            .padding(12)
            .width(Length::Fill)
            .style(container::rounded_box)
            .into(),
    }
}

/// Builds the frequency plot user-interface panel.
pub(super) fn frequency_plot_panel<'a>(
    state: &'a SmithChartState,
    result: &Result<SmithChartAnalysis, String>,
) -> Element<'a, SmithChartMessage> {
    match result {
        Ok(result) => {
            let diagram_result = diagram_analysis(state, result).unwrap_or_else(|| result.clone());
            let mut impedance_tracks = Vec::new();
            push_plot_track(
                &mut impedance_tracks,
                "|Z| [Ω]",
                impedance_magnitude_points(&diagram_result),
                Color::from_rgb8(122, 167, 255),
            );

            let mut s11_tracks = Vec::new();
            push_plot_track(
                &mut s11_tracks,
                "|S11| [dB]",
                s11_db_points(&diagram_result),
                Color::from_rgb8(229, 184, 99),
            );

            let mut s21_tracks = Vec::new();
            let measured_s21 = state
                .active_circuit()
                .ok()
                .and_then(|circuit| {
                    circuit.into_iter().find_map(|element| match element {
                        SmithChartElement::SParameter(block) => Some(
                            diagram_result
                                .frequency_results
                                .iter()
                                .filter_map(|point| {
                                    block
                                        .interpolate(point.frequency_hz)
                                        .and_then(|sample| sample.s21)
                                        .and_then(|s21| {
                                            let magnitude = s21.magnitude();
                                            (magnitude > 0.0).then_some((
                                                point.frequency_hz,
                                                20.0 * magnitude.log10(),
                                            ))
                                        })
                                })
                                .filter(|(_, value)| value.is_finite())
                                .collect::<Vec<_>>(),
                        ),
                        _ => None,
                    })
                })
                .filter(|points| points.len() >= 2);
            let s21_db = measured_s21.unwrap_or_else(|| estimated_s21_points(&diagram_result));
            push_plot_track(
                &mut s21_tracks,
                "|S21| [dB]",
                s21_db,
                Color::from_rgb8(116, 203, 255),
            );

            section(
                "Final Result Diagrams",
                vec![
                    frequency_scale_controls(state),
                    result_diagram(
                        state,
                        ResultDiagramKind::ImpedanceMagnitude,
                        impedance_tracks,
                    ),
                    result_diagram(state, ResultDiagramKind::S11Db, s11_tracks),
                    result_diagram(state, ResultDiagramKind::S21Db, s21_tracks),
                ],
            )
        }
        Err(_) => section(
            "Final Result Diagrams",
            vec![
                text("Resolve input errors to plot frequency data.")
                    .size(12)
                    .into(),
            ],
        ),
    }
}

/// Re-solves final-result diagrams over their complete positive frequency range.
fn diagram_analysis(
    state: &SmithChartState,
    result: &SmithChartAnalysis,
) -> Option<SmithChartAnalysis> {
    let stop_frequency_hz = result
        .frequency_results
        .last()
        .map(|point| point.frequency_hz)
        .unwrap_or(result.active_frequency_hz);
    let frequencies_hz = diagram_frequencies(
        MINIMUM_FREQUENCY_HZ,
        stop_frequency_hz,
        result.frequency_results.len().max(2),
        state.result_frequency_scale,
    );
    let (circuit, settings) = state.solve_state().ok()?;
    let frequency_results = solve_frequency_points(
        &circuit,
        &frequencies_hz,
        settings.show_ideal,
        settings.reference_impedance_ohm,
    )
    .ok()?;
    let two_port_s_parameters = solve_two_port_s_parameters(
        &circuit,
        &frequencies_hz,
        settings.show_ideal,
        settings.reference_impedance_ohm,
    )
    .unwrap_or_default();

    let mut diagram_result = result.clone();
    diagram_result.frequency_results = frequency_results;
    diagram_result.two_port_s_parameters = two_port_s_parameters;
    Some(diagram_result)
}

/// Creates inclusive frequencies distributed according to the selected scale.
fn diagram_frequencies(
    start_frequency_hz: f64,
    stop_frequency_hz: f64,
    samples: usize,
    scale: FrequencyScale,
) -> Vec<f64> {
    if samples < 2
        || !start_frequency_hz.is_finite()
        || !stop_frequency_hz.is_finite()
        || start_frequency_hz <= 0.0
        || stop_frequency_hz <= start_frequency_hz
    {
        return Vec::new();
    }

    (0..samples)
        .map(|index| {
            scale.frequency_at(
                start_frequency_hz,
                stop_frequency_hz,
                index as f64 / (samples - 1) as f64,
            )
        })
        .collect()
}

/// Builds mutually exclusive checkbox controls for the diagram frequency scale.
fn frequency_scale_controls(state: &SmithChartState) -> Element<'_, SmithChartMessage> {
    let mut controls = row![text("Frequency scale")].spacing(12);
    for scale in FrequencyScale::ALL {
        controls = controls.push(
            checkbox(state.result_frequency_scale == scale)
                .label(scale.label())
                .on_toggle(move |_| SmithChartMessage::ResultFrequencyScaleChanged(scale)),
        );
    }
    controls.align_y(Alignment::Center).into()
}

/// Collects finite impedance-magnitude points for the final result diagram.
fn impedance_magnitude_points(result: &SmithChartAnalysis) -> Vec<(f64, f64)> {
    result
        .frequency_results
        .iter()
        .map(|point| (point.frequency_hz, point.impedance.magnitude()))
        .filter(|(_, value)| value.is_finite())
        .collect()
}

/// Collects finite S11 points, using a display floor for a perfect match.
fn s11_db_points(result: &SmithChartAnalysis) -> Vec<(f64, f64)> {
    result
        .frequency_results
        .iter()
        .filter_map(|point| {
            let magnitude = point.reflection_coefficient.magnitude();
            magnitude
                .is_finite()
                .then_some((point.frequency_hz, 20.0 * magnitude.max(1.0e-12).log10()))
        })
        .collect()
}

/// Builds one frequency-result plot and its CSV export controls.
fn result_diagram<'a>(
    state: &'a SmithChartState,
    kind: ResultDiagramKind,
    tracks: Vec<PlotTrack>,
) -> Element<'a, SmithChartMessage> {
    let content: Element<'a, SmithChartMessage> = if tracks.is_empty() {
        text("Increase the frequency span to display this diagram.")
            .size(12)
            .into()
    } else {
        canvas(FrequencyPlotCanvas {
            tracks,
            frequency_scale: state.result_frequency_scale,
        })
        .width(Length::Fill)
        .height(Length::Fixed(240.0))
        .into()
    };
    let mut diagram = column![
        text(kind.title()).size(14),
        content,
        button(text("Export to CSV...")).on_press(SmithChartMessage::OpenCsvExport(kind)),
    ]
    .spacing(6);
    if let Some(configuration) = state
        .csv_export_configuration
        .as_ref()
        .filter(|configuration| configuration.kind == kind)
    {
        let mut export_editor = column![
            text("CSV export range").size(13),
            row![
                text("CSV output unit").width(Length::Fixed(116.0)),
                pick_list(
                    CSV_FREQUENCY_UNITS,
                    Some(configuration.output_frequency_unit),
                    SmithChartMessage::CsvExportOutputFrequencyUnitChanged,
                )
                .width(Length::Fixed(92.0)),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
            row![
                text("Start frequency").width(Length::Fixed(116.0)),
                text_input("", &configuration.start_frequency)
                    .on_input(SmithChartMessage::CsvExportStartFrequencyChanged)
                    .width(Length::Fixed(140.0)),
                pick_list(
                    CSV_FREQUENCY_UNITS,
                    Some(configuration.start_frequency_unit),
                    SmithChartMessage::CsvExportStartFrequencyUnitChanged,
                )
                .width(Length::Fixed(92.0)),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
            row![
                text("Stop frequency").width(Length::Fixed(116.0)),
                text_input("", &configuration.stop_frequency)
                    .on_input(SmithChartMessage::CsvExportStopFrequencyChanged)
                    .width(Length::Fixed(140.0)),
                pick_list(
                    CSV_FREQUENCY_UNITS,
                    Some(configuration.stop_frequency_unit),
                    SmithChartMessage::CsvExportStopFrequencyUnitChanged,
                )
                .width(Length::Fixed(92.0)),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
            row![
                text("Samples").width(Length::Fixed(116.0)),
                text_input("", &configuration.samples)
                    .on_input(SmithChartMessage::CsvExportSamplesChanged)
                    .width(Length::Fixed(140.0)),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
            text("The start and stop frequencies are included in the exported samples.").size(11),
        ]
        .spacing(6);
        if let Some(error) = &configuration.error {
            export_editor = export_editor.push(
                text(error.clone())
                    .size(12)
                    .color(Color::from_rgb8(210, 75, 65)),
            );
        }
        export_editor = export_editor.push(
            row![
                button(text("Save CSV...")).on_press(SmithChartMessage::SaveCsvFile),
                button(text("Cancel")).on_press(SmithChartMessage::CancelCsvExport),
            ]
            .spacing(8),
        );
        diagram = diagram.push(
            container(export_editor)
                .padding(10)
                .width(Length::Fill)
                .style(container::rounded_box),
        );
    }
    diagram.into()
}

/// Summarizes peak estimated S21 and its 3 dB bandwidth.
pub(super) fn estimated_s21_summary(result: &SmithChartAnalysis) -> Option<EstimatedS21Summary> {
    let points = estimated_s21_points(result);
    if points.is_empty() {
        return None;
    }
    let (max_index, (frequency_hz, max_db)) = points
        .iter()
        .copied()
        .enumerate()
        .max_by(|(_, lhs), (_, rhs)| lhs.1.total_cmp(&rhs.1))?;
    let threshold_db = max_db - 3.0;
    let lower = points[..=max_index]
        .iter()
        .rev()
        .find(|(_, value)| *value < threshold_db)
        .map(|(frequency_hz, _)| *frequency_hz);
    let upper = points[max_index..]
        .iter()
        .find(|(_, value)| *value < threshold_db)
        .map(|(frequency_hz, _)| *frequency_hz);
    Some(EstimatedS21Summary {
        max_db,
        frequency_hz,
        bandwidth_hz: lower.zip(upper).map(|(lower, upper)| upper - lower),
    })
}

/// Estimates forward transmission in decibels from reflected power.
fn estimated_s21_db(reflection_coefficient: Complex) -> Option<f64> {
    let reflected_power = reflection_coefficient.magnitude().powi(2);
    if !reflected_power.is_finite() {
        return None;
    }
    let transmitted_power = (1.0 - reflected_power).clamp(1.0e-12, 1.0);
    Some(10.0 * transmitted_power.log10())
}

/// Computes the estimated S21 sample points.
fn estimated_s21_points(result: &SmithChartAnalysis) -> Vec<(f64, f64)> {
    if !result.two_port_s_parameters.is_empty() {
        return result
            .two_port_s_parameters
            .iter()
            .filter_map(|point| {
                let magnitude = point.s_parameters.s21.magnitude();
                magnitude
                    .is_finite()
                    .then_some((point.frequency_hz, 20.0 * magnitude.max(1.0e-12).log10()))
            })
            .collect();
    }
    result
        .frequency_results
        .iter()
        .filter_map(|point| {
            estimated_s21_db(point.reflection_coefficient).map(|value| (point.frequency_hz, value))
        })
        .collect()
}

/// Builds a label/value row for the results panel.
fn metric(label: &'static str, value: String) -> Element<'static, SmithChartMessage> {
    row![
        text(label).width(Length::Fixed(140.0)),
        text(value).width(Length::Fill),
    ]
    .spacing(10)
    .into()
}
