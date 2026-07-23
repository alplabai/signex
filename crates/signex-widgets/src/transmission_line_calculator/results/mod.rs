use super::*;
use iced::widget::column;

mod csv_export_configuration;
mod estimated_s21_summary;
mod result_diagram_kind;

pub(super) use csv_export_configuration::CsvExportConfiguration;
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
            let mut impedance_tracks = Vec::new();
            let impedance_magnitude = result
                .frequency_results
                .iter()
                .map(|point| (point.frequency_hz, point.impedance.magnitude()))
                .filter(|(_, value)| value.is_finite())
                .collect::<Vec<_>>();
            push_plot_track(
                &mut impedance_tracks,
                "|Z| [Ω]",
                impedance_magnitude,
                Color::from_rgb8(122, 167, 255),
            );

            let mut s11_tracks = Vec::new();
            let s11_db = result
                .frequency_results
                .iter()
                .filter_map(|point| {
                    let magnitude = point.reflection_coefficient.magnitude();
                    (magnitude > 0.0).then_some((point.frequency_hz, 20.0 * magnitude.log10()))
                })
                .filter(|(_, value)| value.is_finite())
                .collect::<Vec<_>>();
            push_plot_track(
                &mut s11_tracks,
                "|S11| [dB]",
                s11_db,
                Color::from_rgb8(229, 184, 99),
            );

            let mut s21_tracks = Vec::new();
            let measured_s21 = state
                .active_circuit()
                .ok()
                .and_then(|circuit| {
                    circuit.into_iter().find_map(|element| match element {
                        SmithChartElement::SParameter(block) => Some(
                            block
                                .points()
                                .into_iter()
                                .filter_map(|point| {
                                    point.s21.and_then(|s21| {
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
            let s21_db = measured_s21.unwrap_or_else(|| estimated_s21_points(result));
            push_plot_track(
                &mut s21_tracks,
                "|S21| [dB]",
                s21_db,
                Color::from_rgb8(116, 203, 255),
            );

            section(
                "Final Result Diagrams",
                vec![
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
        canvas(FrequencyPlotCanvas { tracks })
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
                text("Start frequency").width(Length::Fixed(116.0)),
                text_input("", &configuration.start_frequency_mhz)
                    .on_input(SmithChartMessage::CsvExportStartFrequencyChanged)
                    .width(Length::Fixed(140.0)),
                text("MHz"),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
            row![
                text("Stop frequency").width(Length::Fixed(116.0)),
                text_input("", &configuration.stop_frequency_mhz)
                    .on_input(SmithChartMessage::CsvExportStopFrequencyChanged)
                    .width(Length::Fixed(140.0)),
                text("MHz"),
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
