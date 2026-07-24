use crate::transmission_line_calculator::{
    Complex, DEFAULT_REFERENCE_IMPEDANCE_OHM, GainCircle, NoiseFigureCircle, SParameterKind,
    SmithChartAnalysis, SmithChartElement, SmithChartSettings, SmithChartSvgOptions,
    SmithChartSvgTrace, SmithViewTransform, TransformerModel, analyze_smith_chart,
    render_smith_chart_svg, serialize_circuit_tokens, solve_frequency_points,
    solve_noise_figure_circles, solve_s_parameter_gain_circles, solve_two_port_s_parameters,
    split_circuit_tokens,
};
use iced::widget::text_editor;

use crate::transmission_line_calculator::tool::component_editor::{
    CircuitComponentField, CircuitComponentKind, CircuitEditorComponent,
};
use crate::transmission_line_calculator::tool::results::{
    CsvExportConfiguration, ResultDiagramKind,
};
use crate::transmission_line_calculator::tool::{
    MINIMUM_FREQUENCY_HZ, color_to_svg_hex, impedance_arc_chart_traces, s_parameter_chart_traces,
};

use super::state::{
    SHORTED_STUB_WARNING, SmithChartState, estimated_s21_db_for_export, magnitude_db,
    parse_csv_frequency,
};
use super::*;

impl SmithChartState {
    /// Handles an input event and returns the resulting action, if any.
    pub fn update(&mut self, message: SmithChartMessage) {
        match message {
            SmithChartMessage::AddCircuitComponent(kind) => {
                self.circuit_components
                    .push(CircuitEditorComponent::new(kind));
                if kind == CircuitComponentKind::ShortedStub {
                    self.warn_shorted_stub_once();
                }
            }
            SmithChartMessage::CircuitComponentFieldChanged {
                index,
                field,
                value,
            } => {
                if let Some(component) = self.circuit_components.get_mut(index) {
                    component.set_field(field, value);
                }
            }
            SmithChartMessage::CircuitInterpolationChanged(index, value) => {
                if let Some(component) = self.circuit_components.get_mut(index) {
                    component.interpolation = value;
                }
            }
            SmithChartMessage::CircuitTransformerModelChanged(index, value) => {
                if let Some(component) = self.circuit_components.get_mut(index) {
                    component.transformer_model = value;
                }
            }
            SmithChartMessage::MoveCircuitComponentLeft(index) => {
                if index > 1 && index < self.circuit_components.len() {
                    self.circuit_components.swap(index - 1, index);
                }
            }
            SmithChartMessage::MoveCircuitComponentRight(index) => {
                if index > 0 && index + 1 < self.circuit_components.len() {
                    self.circuit_components.swap(index, index + 1);
                }
            }
            SmithChartMessage::RemoveCircuitComponent(index) => {
                if index > 0 && index < self.circuit_components.len() {
                    self.circuit_components.remove(index);
                }
            }
            SmithChartMessage::LoadReChanged(value) => {
                self.load_re = value.clone();
                self.set_black_box_field(CircuitComponentField::Primary, value);
            }
            SmithChartMessage::LoadImChanged(value) => {
                self.load_im = value.clone();
                self.set_black_box_field(CircuitComponentField::Secondary, value);
            }
            SmithChartMessage::LoadToleranceChanged(value) => {
                self.load_tolerance = value.clone();
                self.set_black_box_field(CircuitComponentField::Tolerance, value);
            }
            SmithChartMessage::LoadReSliderChanged(value) => self.load_re_slider = value,
            SmithChartMessage::LoadImSliderChanged(value) => self.load_im_slider = value,
            SmithChartMessage::FrequencyMhzChanged(value) => self.frequency_mhz = value,
            SmithChartMessage::FrequencyUnitChanged(value) => self.frequency_unit = value,
            SmithChartMessage::ReferenceOhmChanged(value) => self.reference_ohm = value,
            SmithChartMessage::SpanMhzChanged(value) => self.span_mhz = value,
            SmithChartMessage::SpanUnitChanged(value) => self.span_unit = value,
            SmithChartMessage::ResolutionChanged(value) => self.resolution = value,
            SmithChartMessage::ShowIdealChanged(value) => self.show_ideal = value,
            SmithChartMessage::DiagramModeChanged(value) => self.diagram_mode = value,
            SmithChartMessage::SmithSphereRotationChanged { yaw, pitch } => {
                self.smith_sphere_yaw = yaw;
                self.smith_sphere_pitch = pitch;
            }
            SmithChartMessage::ResetSmithSphereRotation => {
                self.smith_sphere_yaw = -0.65;
                self.smith_sphere_pitch = 0.35;
            }
            SmithChartMessage::SmithViewTransformChanged(value) => {
                self.smith_view_transform = value;
            }
            SmithChartMessage::ResetSmithViewTransform => {
                self.smith_view_transform = SmithViewTransform::identity();
            }
            SmithChartMessage::ShowGridChanged(value) => self.show_grid = value,
            SmithChartMessage::ShowAdmittanceChanged(value) => self.show_admittance = value,
            SmithChartMessage::ShowVswrChanged(value) => self.show_vswr = value,
            SmithChartMessage::ShowQChanged(value) => self.show_q = value,
            SmithChartMessage::ShowStabilityCirclesChanged(value) => {
                self.show_stability_circles = value
            }
            SmithChartMessage::ShowS11TraceChanged(value) => self.show_s11_trace = value,
            SmithChartMessage::ShowS21TraceChanged(value) => self.show_s21_trace = value,
            SmithChartMessage::ShowS12TraceChanged(value) => self.show_s12_trace = value,
            SmithChartMessage::ShowS22TraceChanged(value) => self.show_s22_trace = value,
            SmithChartMessage::ConjugateSParameterTracesChanged(value) => {
                self.conjugate_s_parameter_traces = value
            }
            SmithChartMessage::VswrCircleInputDbChanged(value) => self.vswr_circle_input_db = value,
            SmithChartMessage::ResistanceLabelListChanged(value) => {
                self.resistance_label_list = value
            }
            SmithChartMessage::ReactanceLabelListChanged(value) => {
                self.reactance_label_list = value
            }
            SmithChartMessage::MarkerListChanged(value) => self.marker_list = value,
            SmithChartMessage::QCircleListChanged(value) => self.q_circle_list = value,
            SmithChartMessage::VswrCircleListChanged(value) => self.vswr_circle_list = value,
            SmithChartMessage::NoiseFigureCircleListChanged(value) => {
                self.noise_figure_circle_list = value
            }
            SmithChartMessage::GainInputCircleListChanged(value) => {
                self.gain_input_circle_list = value
            }
            SmithChartMessage::GainOutputCircleListChanged(value) => {
                self.gain_output_circle_list = value
            }
            SmithChartMessage::SeriesResistanceEnabled(value) => {
                self.series_resistance_enabled = value
            }
            SmithChartMessage::SeriesResistanceChanged(value) => self.series_resistance_ohm = value,
            SmithChartMessage::SeriesResistanceEslNhChanged(value) => {
                self.series_resistance_esl_nh = value
            }
            SmithChartMessage::SeriesResistanceToleranceChanged(value) => {
                self.series_resistance_tolerance = value
            }
            SmithChartMessage::SeriesResistanceSliderChanged(value) => {
                self.series_resistance_slider = value
            }
            SmithChartMessage::ShuntResistanceEnabled(value) => {
                self.shunt_resistance_enabled = value
            }
            SmithChartMessage::ShuntResistanceChanged(value) => self.shunt_resistance_ohm = value,
            SmithChartMessage::ShuntResistanceEslNhChanged(value) => {
                self.shunt_resistance_esl_nh = value
            }
            SmithChartMessage::ShuntResistanceToleranceChanged(value) => {
                self.shunt_resistance_tolerance = value
            }
            SmithChartMessage::ShuntResistanceSliderChanged(value) => {
                self.shunt_resistance_slider = value
            }
            SmithChartMessage::SeriesInductanceEnabled(value) => {
                self.series_inductance_enabled = value
            }
            SmithChartMessage::SeriesInductanceNhChanged(value) => {
                self.series_inductance_nh = value
            }
            SmithChartMessage::SeriesInductanceEsrChanged(value) => {
                self.series_inductance_esr = value
            }
            SmithChartMessage::SeriesInductanceToleranceChanged(value) => {
                self.series_inductance_tolerance = value
            }
            SmithChartMessage::SeriesInductanceSliderChanged(value) => {
                self.series_inductance_slider = value
            }
            SmithChartMessage::ShuntInductanceEnabled(value) => {
                self.shunt_inductance_enabled = value
            }
            SmithChartMessage::ShuntInductanceNhChanged(value) => self.shunt_inductance_nh = value,
            SmithChartMessage::ShuntInductanceEsrChanged(value) => {
                self.shunt_inductance_esr = value
            }
            SmithChartMessage::ShuntInductanceToleranceChanged(value) => {
                self.shunt_inductance_tolerance = value
            }
            SmithChartMessage::ShuntInductanceSliderChanged(value) => {
                self.shunt_inductance_slider = value
            }
            SmithChartMessage::SeriesCapacitanceEnabled(value) => {
                self.series_capacitance_enabled = value
            }
            SmithChartMessage::SeriesCapacitancePfChanged(value) => {
                self.series_capacitance_pf = value
            }
            SmithChartMessage::SeriesCapacitanceEsrChanged(value) => {
                self.series_capacitance_esr = value
            }
            SmithChartMessage::SeriesCapacitanceEslNhChanged(value) => {
                self.series_capacitance_esl_nh = value
            }
            SmithChartMessage::SeriesCapacitanceToleranceChanged(value) => {
                self.series_capacitance_tolerance = value
            }
            SmithChartMessage::SeriesCapacitanceSliderChanged(value) => {
                self.series_capacitance_slider = value
            }
            SmithChartMessage::ShuntCapacitanceEnabled(value) => {
                self.shunt_capacitance_enabled = value
            }
            SmithChartMessage::ShuntCapacitancePfChanged(value) => {
                self.shunt_capacitance_pf = value
            }
            SmithChartMessage::ShuntCapacitanceEsrChanged(value) => {
                self.shunt_capacitance_esr = value
            }
            SmithChartMessage::ShuntCapacitanceEslNhChanged(value) => {
                self.shunt_capacitance_esl_nh = value
            }
            SmithChartMessage::ShuntCapacitanceToleranceChanged(value) => {
                self.shunt_capacitance_tolerance = value
            }
            SmithChartMessage::ShuntCapacitanceSliderChanged(value) => {
                self.shunt_capacitance_slider = value
            }
            SmithChartMessage::RlcEnabled(value) => self.rlc_enabled = value,
            SmithChartMessage::RlcResistanceChanged(value) => self.rlc_resistance_ohm = value,
            SmithChartMessage::RlcInductanceNhChanged(value) => self.rlc_inductance_nh = value,
            SmithChartMessage::RlcCapacitancePfChanged(value) => self.rlc_capacitance_pf = value,
            SmithChartMessage::RlcResistanceSliderChanged(value) => {
                self.rlc_resistance_slider = value
            }
            SmithChartMessage::LineEnabled(value) => self.line_enabled = value,
            SmithChartMessage::LineLengthMmChanged(value) => self.line_length_mm = value,
            SmithChartMessage::LineImpedanceChanged(value) => self.line_impedance_ohm = value,
            SmithChartMessage::LineEeffChanged(value) => self.line_eeff = value,
            SmithChartMessage::LineToleranceChanged(value) => self.line_tolerance = value,
            SmithChartMessage::LineLengthSliderChanged(value) => self.line_length_slider = value,
            SmithChartMessage::OpenStubEnabled(value) => self.open_stub_enabled = value,
            SmithChartMessage::OpenStubLengthMmChanged(value) => self.open_stub_length_mm = value,
            SmithChartMessage::OpenStubImpedanceChanged(value) => {
                self.open_stub_impedance_ohm = value
            }
            SmithChartMessage::OpenStubEeffChanged(value) => self.open_stub_eeff = value,
            SmithChartMessage::OpenStubToleranceChanged(value) => self.open_stub_tolerance = value,
            SmithChartMessage::OpenStubLengthSliderChanged(value) => {
                self.open_stub_length_slider = value
            }
            SmithChartMessage::ShortStubEnabled(value) => {
                self.short_stub_enabled = value;
                if value {
                    self.warn_shorted_stub_once();
                }
            }
            SmithChartMessage::ShortStubLengthMmChanged(value) => self.short_stub_length_mm = value,
            SmithChartMessage::ShortStubImpedanceChanged(value) => {
                self.short_stub_impedance_ohm = value
            }
            SmithChartMessage::ShortStubEeffChanged(value) => self.short_stub_eeff = value,
            SmithChartMessage::ShortStubToleranceChanged(value) => {
                self.short_stub_tolerance = value
            }
            SmithChartMessage::ShortStubLengthSliderChanged(value) => {
                self.short_stub_length_slider = value
            }
            SmithChartMessage::TransformerEnabled(value) => self.transformer_enabled = value,
            SmithChartMessage::TransformerModelChanged(value) => self.transformer_model = value,
            SmithChartMessage::TransformerRatioChanged(value) => self.transformer_ratio = value,
            SmithChartMessage::TransformerL1NhChanged(value) => self.transformer_l1_nh = value,
            SmithChartMessage::TransformerL2NhChanged(value) => self.transformer_l2_nh = value,
            SmithChartMessage::TransformerCouplingChanged(value) => {
                self.transformer_coupling = value
            }
            SmithChartMessage::CustomEnabled(value) => self.custom_enabled = value,
            SmithChartMessage::CustomInterpolationChanged(value) => {
                self.custom_interpolation = value
            }
            SmithChartMessage::CustomPointsChanged(value) => self.custom_points = value,
            SmithChartMessage::SParameterEnabled(value) => {
                self.s_parameter_enabled = value;
                if value {
                    self.sync_s_parameter_sweep_from_text();
                }
            }
            SmithChartMessage::SParameterTextChanged(value) => {
                self.s_parameter_text = value;
                self.s_parameter_content = text_editor::Content::with_text(&self.s_parameter_text);
                self.sync_s_parameter_sweep_from_text();
            }
            SmithChartMessage::SParameterTextAction(action) => {
                self.s_parameter_content.perform(action);
                self.s_parameter_text = self.s_parameter_content.text();
                self.sync_s_parameter_sweep_from_text();
            }
            SmithChartMessage::ImportSParameterFile => {}
            SmithChartMessage::SParameterFileLoaded(result) => match result {
                Ok(Some(text)) => {
                    self.s_parameter_enabled = true;
                    self.s_parameter_text = text.clone();
                    self.s_parameter_content =
                        text_editor::Content::with_text(&self.s_parameter_text);
                    self.sync_s_parameter_sweep_from_text();
                    if let Some(component) = self
                        .circuit_components
                        .iter_mut()
                        .find(|component| component.kind == CircuitComponentKind::SParameters)
                    {
                        component.primary = text.replace('\n', "|");
                    } else {
                        let mut component =
                            CircuitEditorComponent::new(CircuitComponentKind::SParameters);
                        component.primary = text.replace('\n', "|");
                        self.circuit_components.push(component);
                    }
                    self.file_status = Some("Imported Touchstone data".to_string());
                }
                Ok(None) => {
                    self.file_status = Some("Touchstone import cancelled".to_string());
                }
                Err(err) => {
                    self.file_status = Some(err);
                }
            },
            SmithChartMessage::OrderedCircuitEnabled(value) => {
                self.ordered_circuit_enabled = value;
                self.ordered_circuit_error = None;
            }
            SmithChartMessage::OrderedCircuitTokensChanged(value) => {
                self.ordered_circuit_tokens = value;
                self.ordered_circuit_error = None;
            }
            SmithChartMessage::OrderedCircuitRowChanged(index, value) => {
                let mut rows = self.ordered_circuit_rows();
                if let Some(row) = rows.get_mut(index) {
                    *row = value;
                    self.set_ordered_circuit_rows(rows);
                }
            }
            SmithChartMessage::AddOrderedCircuitRow(token) => {
                let mut rows = self.ordered_circuit_rows();
                rows.push(token.to_string());
                self.set_ordered_circuit_rows(rows);
                if token.starts_with("shortedStub") {
                    self.warn_shorted_stub_once();
                }
            }
            SmithChartMessage::MoveOrderedCircuitRowUp(index) => {
                let mut rows = self.ordered_circuit_rows();
                if index > 0 && index < rows.len() {
                    rows.swap(index - 1, index);
                    self.set_ordered_circuit_rows(rows);
                }
            }
            SmithChartMessage::MoveOrderedCircuitRowDown(index) => {
                let mut rows = self.ordered_circuit_rows();
                if index + 1 < rows.len() {
                    rows.swap(index, index + 1);
                    self.set_ordered_circuit_rows(rows);
                }
            }
            SmithChartMessage::RemoveOrderedCircuitRow(index) => {
                let mut rows = self.ordered_circuit_rows();
                if index < rows.len() {
                    rows.remove(index);
                    self.set_ordered_circuit_rows(rows);
                }
            }
            SmithChartMessage::LoadCompactCircuitIntoOrderedRows => match self.compact_circuit() {
                Ok(circuit) => {
                    self.ordered_circuit_tokens = serialize_circuit_tokens(&circuit);
                    self.ordered_circuit_enabled = true;
                    self.ordered_circuit_error = None;
                }
                Err(err) => self.ordered_circuit_error = Some(err),
            },
            SmithChartMessage::OpenCsvExport(kind) => self.open_csv_export(kind),
            SmithChartMessage::CsvExportStartFrequencyChanged(value) => {
                if let Some(configuration) = &mut self.csv_export_configuration {
                    configuration.start_frequency = value;
                    configuration.error = None;
                }
            }
            SmithChartMessage::CsvExportStopFrequencyChanged(value) => {
                if let Some(configuration) = &mut self.csv_export_configuration {
                    configuration.stop_frequency = value;
                    configuration.error = None;
                }
            }
            SmithChartMessage::CsvExportStartFrequencyUnitChanged(unit) => {
                if let Some(configuration) = &mut self.csv_export_configuration {
                    configuration.set_start_frequency_unit(unit);
                }
            }
            SmithChartMessage::CsvExportStopFrequencyUnitChanged(unit) => {
                if let Some(configuration) = &mut self.csv_export_configuration {
                    configuration.set_stop_frequency_unit(unit);
                }
            }
            SmithChartMessage::CsvExportOutputFrequencyUnitChanged(unit) => {
                if let Some(configuration) = &mut self.csv_export_configuration {
                    configuration.set_output_frequency_unit(unit);
                }
            }
            SmithChartMessage::CsvExportSamplesChanged(value) => {
                if let Some(configuration) = &mut self.csv_export_configuration {
                    configuration.samples = value;
                    configuration.error = None;
                }
            }
            SmithChartMessage::ResultFrequencyScaleChanged(scale) => {
                self.result_frequency_scale = scale;
            }
            SmithChartMessage::CancelCsvExport => self.csv_export_configuration = None,
            SmithChartMessage::SaveCsvFile => {}
            SmithChartMessage::CsvFileSaved(result) => match result {
                Ok(Some(path)) => {
                    self.csv_export_configuration = None;
                    self.file_status = Some(format!("Saved CSV to {path}"));
                }
                Ok(None) => self.file_status = Some("CSV save cancelled".to_string()),
                Err(err) => {
                    if let Some(configuration) = &mut self.csv_export_configuration {
                        configuration.error = Some(err.clone());
                    }
                    self.file_status = Some(err);
                }
            },
            SmithChartMessage::SaveSvgFile => {}
            SmithChartMessage::SvgFileSaved(result) => match result {
                Ok(Some(path)) => self.file_status = Some(format!("Saved SVG to {path}")),
                Ok(None) => self.file_status = Some("SVG save cancelled".to_string()),
                Err(err) => self.file_status = Some(err),
            },
            SmithChartMessage::Reset => {
                self.reset_state();
            }
        }
    }

    /// Resets state to its default state.
    fn reset_state(&mut self) {
        *self = Self::new();
    }

    /// Shows the shorted-stub topology warning at most once per state.
    fn warn_shorted_stub_once(&mut self) {
        if !self.short_stub_warning_seen {
            self.short_stub_warning_seen = true;
            self.file_status = Some(SHORTED_STUB_WARNING.to_string());
        }
    }

    /// Solves the active circuit and renders its configured Smith chart as SVG.
    pub fn generated_svg_export(&self) -> Result<String, String> {
        let result = self.solve()?;
        let (gain_circles, noise_figure_circles) = self.overlay_circles().unwrap_or_default();
        Ok(render_smith_chart_svg(
            Some(&result.nominal),
            SmithChartSvgOptions {
                width: 900.0,
                height: 900.0,
                reference_impedance_ohm: parse_field("reference impedance", &self.reference_ohm)
                    .ok()
                    .filter(|value| *value > f64::EPSILON)
                    .unwrap_or(DEFAULT_REFERENCE_IMPEDANCE_OHM),
                show_grid: self.show_grid,
                show_admittance: self.show_admittance,
                show_vswr: self.show_vswr,
                show_q: self.show_q,
                resistance_labels: parse_scalar_list(&self.resistance_label_list)
                    .unwrap_or_else(|_| vec![0.2, 0.5, 1.0, 2.0, 5.0]),
                reactance_labels: parse_scalar_list(&self.reactance_label_list)
                    .unwrap_or_else(|_| vec![-2.0, -1.0, -0.5, 0.5, 1.0, 2.0]),
                z_markers: parse_marker_list(&self.marker_list).unwrap_or_default(),
                vswr_circles: parse_vswr_circle_list(
                    &self.vswr_circle_list,
                    self.vswr_circle_input_db,
                )
                .unwrap_or_default(),
                q_circles: parse_scalar_list(&self.q_circle_list).unwrap_or_default(),
                stability_circles: result.stability_circles.clone(),
                gain_circles,
                noise_figure_circles,
                impedance_arc_traces: impedance_arc_chart_traces(
                    &result,
                    parse_field("reference impedance", &self.reference_ohm)
                        .ok()
                        .filter(|value| *value > f64::EPSILON)
                        .unwrap_or(DEFAULT_REFERENCE_IMPEDANCE_OHM),
                )
                .into_iter()
                .map(|trace| SmithChartSvgTrace {
                    label: trace.label,
                    color: color_to_svg_hex(trace.color),
                    points: trace
                        .points
                        .into_iter()
                        .filter(|point| point.magnitude() <= 1.001)
                        .collect(),
                })
                .collect(),
                s_parameter_traces: s_parameter_chart_traces(self)
                    .into_iter()
                    .map(|trace| SmithChartSvgTrace {
                        label: trace.label.to_string(),
                        color: color_to_svg_hex(trace.color),
                        points: trace.points,
                    })
                    .collect(),
            },
        ))
    }

    /// Generates CSV samples for the currently selected result diagram.
    pub fn generated_csv_export(&self) -> Result<(String, String), String> {
        let configuration = self
            .csv_export_configuration
            .as_ref()
            .ok_or_else(|| "Select a result diagram to export first.".to_string())?;
        let start_frequency =
            parse_csv_frequency("start frequency", &configuration.start_frequency)?;
        let stop_frequency = parse_csv_frequency("stop frequency", &configuration.stop_frequency)?;
        let start_frequency_hz = start_frequency * configuration.start_frequency_unit.multiplier();
        let stop_frequency_hz = stop_frequency * configuration.stop_frequency_unit.multiplier();
        if stop_frequency_hz <= start_frequency_hz {
            return Err("Stop frequency must be greater than start frequency.".to_string());
        }
        let samples = configuration
            .samples
            .trim()
            .parse::<usize>()
            .map_err(|_| "Samples must be a whole number.".to_string())?;
        if !(2..=100_000).contains(&samples) {
            return Err("Samples must be between 2 and 100000.".to_string());
        }

        let output_frequency_multiplier = configuration.output_frequency_unit.multiplier();
        let step_hz = (stop_frequency_hz - start_frequency_hz) / (samples - 1) as f64;
        let frequencies_hz = (0..samples)
            .map(|index| start_frequency_hz + index as f64 * step_hz)
            .collect::<Vec<_>>();
        let (circuit, settings) = self.solve_state()?;
        let results = solve_frequency_points(
            &circuit,
            &frequencies_hz,
            settings.show_ideal,
            settings.reference_impedance_ohm,
        )
        .map_err(|err| err.to_string())?;
        let two_port_s_parameters = solve_two_port_s_parameters(
            &circuit,
            &frequencies_hz,
            settings.show_ideal,
            settings.reference_impedance_ohm,
        )
        .map_err(|err| err.to_string())?;
        let s_parameter_block = circuit.iter().find_map(|element| match element {
            SmithChartElement::SParameter(block) => Some(block),
            _ => None,
        });

        let frequency_symbol = configuration
            .output_frequency_unit
            .frequency_symbol()
            .unwrap_or("Hz");
        let mut csv = format!(
            "Frequency [{frequency_symbol}],{}\r\n",
            configuration.kind.value_label()
        );
        for (index, result) in results.into_iter().enumerate() {
            let value = match configuration.kind {
                ResultDiagramKind::ImpedanceMagnitude => result.impedance.magnitude(),
                ResultDiagramKind::S11Db => magnitude_db(result.reflection_coefficient),
                ResultDiagramKind::S21Db => s_parameter_block
                    .and_then(|block| block.interpolate(result.frequency_hz))
                    .and_then(|point| point.s21)
                    .or_else(|| {
                        two_port_s_parameters
                            .get(index)
                            .map(|point| point.s_parameters.s21)
                    })
                    .map(magnitude_db)
                    .unwrap_or_else(|| estimated_s21_db_for_export(result.reflection_coefficient)),
            };
            csv.push_str(&format!(
                "{:.12},{value:.12}\r\n",
                result.frequency_hz / output_frequency_multiplier
            ));
        }
        Ok((configuration.kind.file_name().to_string(), csv))
    }

    /// Opens CSV export for the requested workflow.
    fn open_csv_export(&mut self, kind: ResultDiagramKind) {
        match self.solve() {
            Ok(result) => {
                let stop_frequency_hz = result
                    .frequency_results
                    .last()
                    .map(|point| point.frequency_hz)
                    .unwrap_or(result.active_frequency_hz);
                let samples = result.frequency_results.len().max(2);
                self.csv_export_configuration = Some(CsvExportConfiguration::new(
                    kind,
                    MINIMUM_FREQUENCY_HZ,
                    stop_frequency_hz,
                    samples,
                ));
            }
            Err(err) => self.file_status = Some(err),
        }
    }

    /// Solves the configured transmission-line network.
    pub(in crate::transmission_line_calculator::tool) fn solve(
        &self,
    ) -> Result<SmithChartAnalysis, String> {
        let (circuit, settings) = self.solve_state()?;
        analyze_smith_chart(&circuit, settings).map_err(|err| err.to_string())
    }

    /// Computes the overlay circle geometry.
    pub(in crate::transmission_line_calculator::tool) fn overlay_circles(
        &self,
    ) -> Result<(Vec<GainCircle>, Vec<NoiseFigureCircle>), String> {
        let (circuit, settings) = self.solve_state()?;
        let gain_input_circles = parse_scalar_list(&self.gain_input_circle_list)?;
        let gain_output_circles = parse_scalar_list(&self.gain_output_circle_list)?;
        let noise_figure_circles = parse_scalar_list(&self.noise_figure_circle_list)?;
        Ok((
            solve_s_parameter_gain_circles(
                &circuit,
                &settings,
                &gain_input_circles,
                &gain_output_circles,
            ),
            solve_noise_figure_circles(&circuit, &settings, &noise_figure_circles),
        ))
    }

    /// Solves state from the supplied circuit and settings.
    pub(in crate::transmission_line_calculator::tool) fn solve_state(
        &self,
    ) -> Result<(Vec<SmithChartElement>, SmithChartSettings), String> {
        let circuit = self.active_circuit()?;
        let resolution = parse_optional("resolution", &self.resolution)?
            .round()
            .max(1.0) as usize;
        Ok((
            circuit,
            SmithChartSettings {
                frequency_hz: parse_field("frequency", &self.frequency_mhz)?
                    * self.frequency_unit.multiplier(),
                frequency_unit: self.frequency_unit,
                reference_impedance_ohm: parse_field("reference impedance", &self.reference_ohm)?,
                span_hz: parse_optional("span", &self.span_mhz)? * self.span_unit.multiplier(),
                span_unit: self.span_unit,
                resolution,
                show_ideal: self.show_ideal,
            },
        ))
    }

    /// Synchronizes s parameter sweep from text with the current source data.
    fn sync_s_parameter_sweep_from_text(&mut self) {
        let Ok(block) = parse_touchstone_input(&self.s_parameter_text) else {
            return;
        };
        self.sync_s_parameter_sweep(&block);
    }

    /// Synchronizes s parameter sweep with the current source data.
    fn sync_s_parameter_sweep(
        &mut self,
        block: &crate::transmission_line_calculator::SParameterBlock,
    ) {
        let points = block.points();
        let Some(first) = points.first() else {
            return;
        };
        let middle = &points[points.len() / 2];
        let last = points.last().unwrap_or(first);
        let unit = block.source_frequency_unit;
        let multiplier = unit.multiplier();
        let span_hz = 2.0
            * (last.frequency_hz - middle.frequency_hz)
                .abs()
                .max((middle.frequency_hz - first.frequency_hz).abs());

        self.frequency_unit = unit;
        self.span_unit = unit;
        self.frequency_mhz = format_number(middle.frequency_hz / multiplier);
        self.span_mhz = format_number(span_hz / multiplier);
    }

    /// Converts the ordered component-editor cards into solver elements.
    pub(in crate::transmission_line_calculator::tool) fn active_circuit(
        &self,
    ) -> Result<Vec<SmithChartElement>, String> {
        self.circuit_components
            .iter()
            .map(CircuitEditorComponent::to_element)
            .collect()
    }

    /// Updates black box field with the supplied value.
    fn set_black_box_field(&mut self, field: CircuitComponentField, value: String) {
        if let Some(component) = self.circuit_components.first_mut() {
            if component.kind == CircuitComponentKind::BlackBox {
                component.set_field(field, value);
            }
        }
    }

    /// Builds the ordered circuit user-interface rows.
    fn ordered_circuit_rows(&self) -> Vec<String> {
        split_circuit_tokens(&self.ordered_circuit_tokens)
            .into_iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    /// Updates ordered circuit rows with the supplied value.
    fn set_ordered_circuit_rows(&mut self, rows: Vec<String>) {
        self.ordered_circuit_tokens = rows.join("__");
        self.ordered_circuit_enabled = true;
        self.ordered_circuit_error = None;
    }

    /// Builds the legacy compact circuit representation from the current controls.
    pub(super) fn compact_circuit(&self) -> Result<Vec<SmithChartElement>, String> {
        let black_box_impedance = Complex::new(
            parse_field("load resistance", &self.load_re)?,
            parse_field("load reactance", &self.load_im)?,
        );
        let black_box_tolerance = optional_tolerance("load tolerance", &self.load_tolerance)?;
        let mut circuit = vec![SmithChartElement::BlackBox {
            impedance: black_box_impedance,
            tolerance_percent: black_box_tolerance,
        }];

        if self.series_resistance_enabled {
            circuit.push(SmithChartElement::SeriesResistor {
                resistance_ohm: parse_field("series resistance", &self.series_resistance_ohm)?,
                esl_h: parse_optional("series resistance ESL", &self.series_resistance_esl_nh)?
                    * 1.0e-9,
                tolerance_percent: optional_tolerance(
                    "series resistance tolerance",
                    &self.series_resistance_tolerance,
                )?,
            });
        }
        if self.shunt_resistance_enabled {
            circuit.push(SmithChartElement::ShuntResistor {
                resistance_ohm: parse_field("shunt resistance", &self.shunt_resistance_ohm)?,
                esl_h: parse_optional("shunt resistance ESL", &self.shunt_resistance_esl_nh)?
                    * 1.0e-9,
                tolerance_percent: optional_tolerance(
                    "shunt resistance tolerance",
                    &self.shunt_resistance_tolerance,
                )?,
            });
        }
        if self.series_inductance_enabled {
            circuit.push(SmithChartElement::SeriesInductor {
                inductance_h: parse_field("series inductance", &self.series_inductance_nh)?
                    * 1.0e-9,
                esr_ohm: parse_optional("series inductance ESR", &self.series_inductance_esr)?,
                tolerance_percent: optional_tolerance(
                    "series inductance tolerance",
                    &self.series_inductance_tolerance,
                )?,
            });
        }
        if self.shunt_inductance_enabled {
            circuit.push(SmithChartElement::ShuntInductor {
                inductance_h: parse_field("shunt inductance", &self.shunt_inductance_nh)? * 1.0e-9,
                esr_ohm: parse_optional("shunt inductance ESR", &self.shunt_inductance_esr)?,
                tolerance_percent: optional_tolerance(
                    "shunt inductance tolerance",
                    &self.shunt_inductance_tolerance,
                )?,
            });
        }
        if self.series_capacitance_enabled {
            circuit.push(SmithChartElement::SeriesCapacitor {
                capacitance_f: parse_field("series capacitance", &self.series_capacitance_pf)?
                    * 1.0e-12,
                esr_ohm: parse_optional("series capacitance ESR", &self.series_capacitance_esr)?,
                esl_h: parse_optional("series capacitance ESL", &self.series_capacitance_esl_nh)?
                    * 1.0e-9,
                tolerance_percent: optional_tolerance(
                    "series capacitance tolerance",
                    &self.series_capacitance_tolerance,
                )?,
            });
        }
        if self.shunt_capacitance_enabled {
            circuit.push(SmithChartElement::ShuntCapacitor {
                capacitance_f: parse_field("shunt capacitance", &self.shunt_capacitance_pf)?
                    * 1.0e-12,
                esr_ohm: parse_optional("shunt capacitance ESR", &self.shunt_capacitance_esr)?,
                esl_h: parse_optional("shunt capacitance ESL", &self.shunt_capacitance_esl_nh)?
                    * 1.0e-9,
                tolerance_percent: optional_tolerance(
                    "shunt capacitance tolerance",
                    &self.shunt_capacitance_tolerance,
                )?,
            });
        }
        if self.rlc_enabled {
            circuit.push(SmithChartElement::SeriesParallelRlc {
                resistance_ohm: parse_field("parallel RLC resistance", &self.rlc_resistance_ohm)?,
                inductance_h: parse_field("parallel RLC inductance", &self.rlc_inductance_nh)?
                    * 1.0e-9,
                capacitance_f: parse_field("parallel RLC capacitance", &self.rlc_capacitance_pf)?
                    * 1.0e-12,
            });
        }
        if self.line_enabled {
            circuit.push(SmithChartElement::TransmissionLine {
                length_m: parse_field("transmission line length", &self.line_length_mm)? * 1.0e-3,
                characteristic_impedance_ohm: parse_field(
                    "transmission line impedance",
                    &self.line_impedance_ohm,
                )?,
                effective_dielectric: parse_field(
                    "transmission line effective dielectric",
                    &self.line_eeff,
                )?,
                tolerance_percent: optional_tolerance(
                    "transmission line tolerance",
                    &self.line_tolerance,
                )?,
            });
        }
        if self.open_stub_enabled {
            circuit.push(SmithChartElement::OpenStub {
                length_m: parse_field("open stub length", &self.open_stub_length_mm)? * 1.0e-3,
                characteristic_impedance_ohm: parse_field(
                    "open stub impedance",
                    &self.open_stub_impedance_ohm,
                )?,
                effective_dielectric: parse_field(
                    "open stub effective dielectric",
                    &self.open_stub_eeff,
                )?,
                tolerance_percent: optional_tolerance(
                    "open stub tolerance",
                    &self.open_stub_tolerance,
                )?,
            });
        }
        if self.short_stub_enabled {
            circuit.push(SmithChartElement::ShortedStub {
                length_m: parse_field("shorted stub length", &self.short_stub_length_mm)? * 1.0e-3,
                characteristic_impedance_ohm: parse_field(
                    "shorted stub impedance",
                    &self.short_stub_impedance_ohm,
                )?,
                effective_dielectric: parse_field(
                    "shorted stub effective dielectric",
                    &self.short_stub_eeff,
                )?,
                tolerance_percent: optional_tolerance(
                    "shorted stub tolerance",
                    &self.short_stub_tolerance,
                )?,
            });
        }
        if self.transformer_enabled {
            circuit.push(SmithChartElement::Transformer {
                model: self.transformer_model,
                l1_h: parse_optional("transformer L1", &self.transformer_l1_nh)? * 1.0e-9,
                l2_h: parse_optional("transformer L2", &self.transformer_l2_nh)? * 1.0e-9,
                coupling_or_turns_ratio: match self.transformer_model {
                    TransformerModel::CoupledInductor => {
                        parse_field("transformer coupling", &self.transformer_coupling)?
                    }
                    TransformerModel::Ideal => {
                        parse_field("transformer turns ratio", &self.transformer_ratio)?
                    }
                },
            });
        }
        if self.custom_enabled {
            circuit.push(SmithChartElement::Custom {
                points: parse_custom_points(&self.custom_points)?,
                interpolation: self.custom_interpolation,
            });
        }
        if self.s_parameter_enabled {
            let block = parse_touchstone_input(&self.s_parameter_text)?;
            let s_parameter = SmithChartElement::SParameter(block.clone());
            if block.kind() == SParameterKind::S1P {
                let load = SmithChartElement::LoadTermination {
                    impedance: black_box_impedance,
                    tolerance_percent: black_box_tolerance,
                };
                return Ok(std::iter::once(s_parameter)
                    .chain(circuit.into_iter().skip(1))
                    .chain(std::iter::once(load))
                    .collect());
            }
            circuit.push(s_parameter);
            circuit.push(SmithChartElement::LoadTermination {
                impedance: Complex::new(DEFAULT_REFERENCE_IMPEDANCE_OHM, 0.0),
                tolerance_percent: None,
            });
        }

        Ok(circuit)
    }
}
