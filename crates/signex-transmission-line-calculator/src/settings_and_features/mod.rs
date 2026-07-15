use super::*;
use iced::widget::rule;

pub(super) fn settings_and_features_section(
    state: &SmithChartState,
) -> Element<'_, SmithChartMessage> {
    section(
        "Settings & Features",
        vec![
            settings_group(
                "Analysis",
                vec![
                    explained_control(
                        input_row(
                            "Z₀",
                            "Ω",
                            &state.reference_ohm,
                            SmithChartMessage::ReferenceOhmChanged,
                        ),
                        "Sets the reference impedance used to normalize the circuit and calculate the reflection coefficient.",
                    ),
                    explained_control(
                        unit_input_row(
                            "Frequency",
                            &state.frequency_mhz,
                            state.frequency_unit,
                            SmithChartMessage::FrequencyMhzChanged,
                            SmithChartMessage::FrequencyUnitChanged,
                        ),
                        "Selects the center frequency at which the active result and Smith-chart marker are evaluated.",
                    ),
                    explained_control(
                        unit_input_row(
                            "Frequency span",
                            &state.span_mhz,
                            state.span_unit,
                            SmithChartMessage::SpanMhzChanged,
                            SmithChartMessage::SpanUnitChanged,
                        ),
                        "Sweeps from the center frequency minus this value to the center frequency plus this value.",
                    ),
                    explained_control(
                        input_row(
                            "Resolution",
                            "steps per side",
                            &state.resolution,
                            SmithChartMessage::ResolutionChanged,
                        ),
                        "Controls the number of sweep steps on each side of the center frequency; higher values produce smoother traces.",
                    ),
                ],
            ),
            rule::horizontal(1).into(),
            settings_group(
                "Chart Display",
                vec![
                    explained_control(
                        checkbox_row(
                            "Ideal components",
                            state.show_ideal,
                            SmithChartMessage::ShowIdealChanged,
                        ),
                        "Uses idealized component behavior instead of including configured parasitic values.",
                    ),
                    explained_control(
                        checkbox_row("Grid", state.show_grid, SmithChartMessage::ShowGridChanged),
                        "Shows or hides the normalized resistance and reactance grid on the Smith chart.",
                    ),
                    explained_control(
                        input_row(
                            "R grid labels",
                            "normalized values",
                            &state.resistance_label_list,
                            SmithChartMessage::ResistanceLabelListChanged,
                        ),
                        "Defines the normalized resistance values used for grid circles and their labels.",
                    ),
                    explained_control(
                        input_row(
                            "X grid labels",
                            "normalized values",
                            &state.reactance_label_list,
                            SmithChartMessage::ReactanceLabelListChanged,
                        ),
                        "Defines the positive and negative normalized reactance values used for grid arcs and labels.",
                    ),
                    explained_control(
                        checkbox_row(
                            "Admittance mirror",
                            state.show_admittance,
                            SmithChartMessage::ShowAdmittanceChanged,
                        ),
                        "Adds the mirrored admittance grid so parallel-network behavior can be read directly.",
                    ),
                    explained_control(
                        checkbox_row(
                            "Stacked layout",
                            state.stacked_layout,
                            SmithChartMessage::StackedLayoutChanged,
                        ),
                        "Stores the vertically stacked layout preference when URL state is reset or imported.",
                    ),
                ],
            ),
            rule::horizontal(1).into(),
            settings_group(
                "Markers & Overlays",
                vec![
                    explained_control(
                        checkbox_row(
                            "VSWR circles",
                            state.show_vswr,
                            SmithChartMessage::ShowVswrChanged,
                        ),
                        "Shows constant-VSWR guides for judging impedance matching quality.",
                    ),
                    explained_control(
                        checkbox_row("Q arcs", state.show_q, SmithChartMessage::ShowQChanged),
                        "Shows constant-Q arcs derived from the ratio between reactance and resistance.",
                    ),
                    explained_control(
                        checkbox_row(
                            "Stability circles",
                            state.show_stability_circles,
                            SmithChartMessage::ShowStabilityCirclesChanged,
                        ),
                        "Shows source and load stability circles when the active S-parameter data provides a two-port network.",
                    ),
                    explained_control(
                        input_row(
                            "Impedance markers",
                            "R,X or polar:mag,deg",
                            &state.marker_list,
                            SmithChartMessage::MarkerListChanged,
                        ),
                        "Adds fixed impedance markers using rectangular values or polar magnitude and angle entries.",
                    ),
                    explained_control(
                        input_row(
                            "Constant Q circles",
                            "values",
                            &state.q_circle_list,
                            SmithChartMessage::QCircleListChanged,
                        ),
                        "Adds custom constant-Q overlays for the listed Q-factor values.",
                    ),
                    explained_control(
                        input_row(
                            "Constant VSWR circles",
                            if state.vswr_circle_input_db {
                                "dB values"
                            } else {
                                "ratio values"
                            },
                            &state.vswr_circle_list,
                            SmithChartMessage::VswrCircleListChanged,
                        ),
                        "Adds custom matching-quality circles using VSWR ratios or return-loss values in dB.",
                    ),
                    explained_control(
                        checkbox_row(
                            "VSWR field is dB",
                            state.vswr_circle_input_db,
                            SmithChartMessage::VswrCircleInputDbChanged,
                        ),
                        "Interprets the custom VSWR-circle entries as decibel values and converts them to linear ratios before drawing.",
                    ),
                    explained_control(
                        input_row(
                            "Noise figure circles",
                            "dB values",
                            &state.noise_figure_circle_list,
                            SmithChartMessage::NoiseFigureCircleListChanged,
                        ),
                        "Draws constant noise-figure circles from two-port noise parameters for the requested dB values.",
                    ),
                    explained_control(
                        input_row(
                            "Input gain circles",
                            "dB values",
                            &state.gain_input_circle_list,
                            SmithChartMessage::GainInputCircleListChanged,
                        ),
                        "Draws constant available-gain circles in the source reflection-coefficient plane.",
                    ),
                    explained_control(
                        input_row(
                            "Output gain circles",
                            "dB values",
                            &state.gain_output_circle_list,
                            SmithChartMessage::GainOutputCircleListChanged,
                        ),
                        "Draws constant operating-gain circles in the load reflection-coefficient plane.",
                    ),
                ],
            ),
            rule::horizontal(1).into(),
            settings_group(
                "S-Parameter Traces",
                vec![
                    explained_control(
                        checkbox_row(
                            "S11 trace",
                            state.show_s11_trace,
                            SmithChartMessage::ShowS11TraceChanged,
                        ),
                        "Shows the input reflection coefficient across the imported S-parameter frequency range.",
                    ),
                    explained_control(
                        checkbox_row(
                            "S21 trace",
                            state.show_s21_trace,
                            SmithChartMessage::ShowS21TraceChanged,
                        ),
                        "Shows the forward transmission coefficient from port 1 to port 2.",
                    ),
                    explained_control(
                        checkbox_row(
                            "S12 trace",
                            state.show_s12_trace,
                            SmithChartMessage::ShowS12TraceChanged,
                        ),
                        "Shows the reverse transmission coefficient from port 2 to port 1.",
                    ),
                    explained_control(
                        checkbox_row(
                            "S22 trace",
                            state.show_s22_trace,
                            SmithChartMessage::ShowS22TraceChanged,
                        ),
                        "Shows the output reflection coefficient across the imported frequency range.",
                    ),
                    explained_control(
                        checkbox_row(
                            "Conjugate S traces",
                            state.conjugate_s_parameter_traces,
                            SmithChartMessage::ConjugateSParameterTracesChanged,
                        ),
                        "Mirrors all displayed S-parameter traces by using their complex conjugates.",
                    ),
                ],
            ),
        ],
    )
}

fn settings_group<'a>(
    title: &'static str,
    controls: Vec<Element<'a, SmithChartMessage>>,
) -> Element<'a, SmithChartMessage> {
    let mut content = iced::widget::column![text(title).size(16)].spacing(12);
    for control in controls {
        content = content.push(control);
    }
    content.into()
}

fn explained_control<'a>(
    control: Element<'a, SmithChartMessage>,
    explanation: &'static str,
) -> Element<'a, SmithChartMessage> {
    iced::widget::column![control, text(explanation).size(11).width(Length::Fill)]
        .spacing(3)
        .into()
}

pub(super) fn url_section(state: &SmithChartState) -> Element<'_, SmithChartMessage> {
    let mut rows = vec![
        input_row(
            "Query",
            "",
            &state.url_query,
            SmithChartMessage::UrlQueryChanged,
        ),
        row![
            button(text("Export")).on_press(SmithChartMessage::ExportUrlQuery),
            button(text("Copy URL")).on_press(SmithChartMessage::CopyUrl),
            button(text("Import")).on_press(SmithChartMessage::ImportUrlQuery),
            button(text("Clear URL State")).on_press(SmithChartMessage::ClearImportedUrlState),
        ]
        .spacing(8)
        .into(),
    ];
    if let Some(error) = &state.url_error {
        rows.push(
            text(error.clone())
                .size(12)
                .color(Color::from_rgb8(210, 75, 65))
                .into(),
        );
    }
    section("OnlineSmithChart URL", rows)
}

pub(super) fn svg_export_section(state: &SmithChartState) -> Element<'_, SmithChartMessage> {
    let mut rows = vec![
        button(text("Save Smith Chart as SVG..."))
            .on_press(SmithChartMessage::SaveSvgFile)
            .into(),
    ];
    if let Some(status) = &state.file_status {
        rows.push(text(status.clone()).size(12).into());
    }
    section("SVG Export", rows)
}
