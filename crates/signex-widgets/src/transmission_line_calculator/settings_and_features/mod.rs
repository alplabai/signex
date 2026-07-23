use super::*;

/// Builds the analysis user-interface section.
pub(super) fn analysis_section(state: &SmithChartState) -> Element<'_, SmithChartMessage> {
    section(
        "Analysis Sweep",
        vec![
            text(
                "Set the normalization and frequency range used by the chart and result diagrams.",
            )
            .size(11)
            .into(),
            input_row(
                "Z₀",
                "Ω",
                &state.reference_ohm,
                SmithChartMessage::ReferenceOhmChanged,
            ),
            unit_input_row(
                "Center frequency",
                &state.frequency_mhz,
                state.frequency_unit,
                SmithChartMessage::FrequencyMhzChanged,
                SmithChartMessage::FrequencyUnitChanged,
            ),
            unit_input_row(
                "Sweep ±",
                &state.span_mhz,
                state.span_unit,
                SmithChartMessage::SpanMhzChanged,
                SmithChartMessage::SpanUnitChanged,
            ),
            input_row(
                "Resolution",
                "steps per side",
                &state.resolution,
                SmithChartMessage::ResolutionChanged,
            ),
        ],
    )
}

/// Builds the chart control user-interface rows.
pub(super) fn chart_control_rows(state: &SmithChartState) -> Vec<Element<'_, SmithChartMessage>> {
    let is_planar = state.diagram_mode != SmithChartDiagramMode::ThreeDimensional;
    let has_s_parameters = has_s_parameter_component(state);
    let mut display = iced::widget::Row::new()
        .push(text("Display"))
        .push(checkbox_row(
            "Grid",
            state.show_grid,
            SmithChartMessage::ShowGridChanged,
        ))
        .spacing(14)
        .align_y(Alignment::Center);
    if state.diagram_mode == SmithChartDiagramMode::TwoDimensional {
        display = display.push(checkbox_row(
            "Admittance overlay",
            state.show_admittance,
            SmithChartMessage::ShowAdmittanceChanged,
        ));
    }
    if is_planar {
        display = display
            .push(checkbox_row(
                "VSWR",
                state.show_vswr,
                SmithChartMessage::ShowVswrChanged,
            ))
            .push(checkbox_row(
                "Q arcs",
                state.show_q,
                SmithChartMessage::ShowQChanged,
            ));
        if has_s_parameters {
            display = display.push(checkbox_row(
                "Stability circles",
                state.show_stability_circles,
                SmithChartMessage::ShowStabilityCirclesChanged,
            ));
        }
    }

    let mut rows = vec![display.into()];
    if has_s_parameters {
        rows.push(s_parameter_trace_controls(state));
    }
    rows
}

/// Builds the chart overlays user-interface section.
pub(super) fn chart_overlays_section(state: &SmithChartState) -> Element<'_, SmithChartMessage> {
    let is_planar = state.diagram_mode != SmithChartDiagramMode::ThreeDimensional;
    let has_s_parameters = has_s_parameter_component(state);
    let mut rows = vec![input_row(
        "Z markers",
        "R,X or polar:mag,deg",
        &state.marker_list,
        SmithChartMessage::MarkerListChanged,
    )];

    if is_planar && state.show_q {
        rows.push(input_row(
            "Custom Q",
            "values",
            &state.q_circle_list,
            SmithChartMessage::QCircleListChanged,
        ));
    }
    if is_planar && state.show_vswr {
        rows.push(input_row(
            "Custom VSWR",
            if state.vswr_circle_input_db {
                "dB values"
            } else {
                "ratio values"
            },
            &state.vswr_circle_list,
            SmithChartMessage::VswrCircleListChanged,
        ));
        rows.push(checkbox_row(
            "Interpret custom VSWR values as dB",
            state.vswr_circle_input_db,
            SmithChartMessage::VswrCircleInputDbChanged,
        ));
    }
    if is_planar && has_s_parameters {
        rows.push(text("Two-port circles").size(13).into());
        rows.push(input_row(
            "Noise figure",
            "dB values",
            &state.noise_figure_circle_list,
            SmithChartMessage::NoiseFigureCircleListChanged,
        ));
        rows.push(input_row(
            "Input gain",
            "dB values",
            &state.gain_input_circle_list,
            SmithChartMessage::GainInputCircleListChanged,
        ));
        rows.push(input_row(
            "Output gain",
            "dB values",
            &state.gain_output_circle_list,
            SmithChartMessage::GainOutputCircleListChanged,
        ));
    }

    section("Markers & Advanced Guides", rows)
}

/// Builds the s parameter trace contextual controls.
fn s_parameter_trace_controls(state: &SmithChartState) -> Element<'_, SmithChartMessage> {
    iced::widget::Row::new()
        .push(text("S-parameter traces"))
        .push(checkbox_row(
            "S11",
            state.show_s11_trace,
            SmithChartMessage::ShowS11TraceChanged,
        ))
        .push(checkbox_row(
            "S21",
            state.show_s21_trace,
            SmithChartMessage::ShowS21TraceChanged,
        ))
        .push(checkbox_row(
            "S12",
            state.show_s12_trace,
            SmithChartMessage::ShowS12TraceChanged,
        ))
        .push(checkbox_row(
            "S22",
            state.show_s22_trace,
            SmithChartMessage::ShowS22TraceChanged,
        ))
        .push(checkbox_row(
            "Conjugate",
            state.conjugate_s_parameter_traces,
            SmithChartMessage::ConjugateSParameterTracesChanged,
        ))
        .spacing(14)
        .align_y(Alignment::Center)
        .into()
}

/// Returns whether the edited circuit contains an S-parameter component.
fn has_s_parameter_component(state: &SmithChartState) -> bool {
    state
        .circuit_components
        .iter()
        .any(|component| component.kind == CircuitComponentKind::SParameters)
}
