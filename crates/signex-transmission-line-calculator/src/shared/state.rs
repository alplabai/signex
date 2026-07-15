use crate::{
    Complex, CustomInterpolation, DEFAULT_REFERENCE_IMPEDANCE_OHM, GainCircle, NoiseFigureCircle,
    SParameterKind, ScalarUnit, SmithChartAnalysis, SmithChartElement, SmithChartOverlays,
    SmithChartSettings, SmithChartSvgOptions, SmithChartSvgTrace, TransformerModel,
    analyze_smith_chart, parse_online_smith_chart_query, render_smith_chart_svg,
    serialize_online_smith_chart_circuit_tokens, serialize_online_smith_chart_query,
    solve_frequency_points, solve_noise_figure_circles, solve_s_parameter_gain_circles,
    split_online_smith_chart_circuit_tokens,
};
use iced::widget::text_editor;

use crate::tool::component_editor::{
    CircuitComponentField, CircuitComponentKind, CircuitEditorComponent,
};
use crate::tool::results::{CsvExportConfiguration, ResultDiagramKind};
use crate::tool::{color_to_svg_hex, impedance_arc_chart_traces, s_parameter_chart_traces};

use super::*;

#[derive(Debug, Clone)]
pub struct SmithChartState {
    pub circuit_components: Vec<CircuitEditorComponent>,
    pub load_re: String,
    pub load_im: String,
    pub load_tolerance: String,
    pub load_re_slider: String,
    pub load_im_slider: String,
    pub frequency_mhz: String,
    pub frequency_unit: ScalarUnit,
    pub reference_ohm: String,
    pub span_mhz: String,
    pub span_unit: ScalarUnit,
    pub resolution: String,
    pub show_ideal: bool,
    pub diagram_mode: SmithChartDiagramMode,
    pub smith_sphere_yaw: f32,
    pub smith_sphere_pitch: f32,
    pub show_grid: bool,
    pub show_admittance: bool,
    pub stacked_layout: bool,
    pub show_vswr: bool,
    pub show_q: bool,
    pub show_stability_circles: bool,
    pub show_s11_trace: bool,
    pub show_s21_trace: bool,
    pub show_s12_trace: bool,
    pub show_s22_trace: bool,
    pub conjugate_s_parameter_traces: bool,
    pub vswr_circle_input_db: bool,
    pub resistance_label_list: String,
    pub reactance_label_list: String,
    pub marker_list: String,
    pub q_circle_list: String,
    pub vswr_circle_list: String,
    pub noise_figure_circle_list: String,
    pub gain_input_circle_list: String,
    pub gain_output_circle_list: String,
    pub series_resistance_enabled: bool,
    pub series_resistance_ohm: String,
    pub series_resistance_esl_nh: String,
    pub series_resistance_tolerance: String,
    pub series_resistance_slider: String,
    pub shunt_resistance_enabled: bool,
    pub shunt_resistance_ohm: String,
    pub shunt_resistance_esl_nh: String,
    pub shunt_resistance_tolerance: String,
    pub shunt_resistance_slider: String,
    pub series_inductance_enabled: bool,
    pub series_inductance_nh: String,
    pub series_inductance_esr: String,
    pub series_inductance_tolerance: String,
    pub series_inductance_slider: String,
    pub shunt_inductance_enabled: bool,
    pub shunt_inductance_nh: String,
    pub shunt_inductance_esr: String,
    pub shunt_inductance_tolerance: String,
    pub shunt_inductance_slider: String,
    pub series_capacitance_enabled: bool,
    pub series_capacitance_pf: String,
    pub series_capacitance_esr: String,
    pub series_capacitance_esl_nh: String,
    pub series_capacitance_tolerance: String,
    pub series_capacitance_slider: String,
    pub shunt_capacitance_enabled: bool,
    pub shunt_capacitance_pf: String,
    pub shunt_capacitance_esr: String,
    pub shunt_capacitance_esl_nh: String,
    pub shunt_capacitance_tolerance: String,
    pub shunt_capacitance_slider: String,
    pub rlc_enabled: bool,
    pub rlc_resistance_ohm: String,
    pub rlc_inductance_nh: String,
    pub rlc_capacitance_pf: String,
    pub rlc_resistance_slider: String,
    pub line_enabled: bool,
    pub line_length_mm: String,
    pub line_impedance_ohm: String,
    pub line_eeff: String,
    pub line_tolerance: String,
    pub line_length_slider: String,
    pub open_stub_enabled: bool,
    pub open_stub_length_mm: String,
    pub open_stub_impedance_ohm: String,
    pub open_stub_eeff: String,
    pub open_stub_tolerance: String,
    pub open_stub_length_slider: String,
    pub short_stub_enabled: bool,
    pub short_stub_length_mm: String,
    pub short_stub_impedance_ohm: String,
    pub short_stub_eeff: String,
    pub short_stub_tolerance: String,
    pub short_stub_length_slider: String,
    pub short_stub_warning_seen: bool,
    pub transformer_enabled: bool,
    pub transformer_model: TransformerModel,
    pub transformer_ratio: String,
    pub transformer_l1_nh: String,
    pub transformer_l2_nh: String,
    pub transformer_coupling: String,
    pub custom_enabled: bool,
    pub custom_interpolation: CustomInterpolation,
    pub custom_points: String,
    pub s_parameter_enabled: bool,
    pub s_parameter_text: String,
    pub s_parameter_content: text_editor::Content,
    pub ordered_circuit_enabled: bool,
    pub ordered_circuit_tokens: String,
    pub ordered_circuit_error: Option<String>,
    pub url_query: String,
    pub url_error: Option<String>,
    pub file_status: Option<String>,
    pub(in crate::tool) csv_export_configuration: Option<CsvExportConfiguration>,
}

impl SmithChartState {
    pub fn new() -> Self {
        let s_parameter_text = DEFAULT_S_PARAMETER_TEXT.to_string();
        Self {
            circuit_components: vec![CircuitEditorComponent::new(CircuitComponentKind::BlackBox)],
            load_re: "50".to_string(),
            load_im: "0".to_string(),
            load_tolerance: "0".to_string(),
            load_re_slider: "0".to_string(),
            load_im_slider: "0".to_string(),
            frequency_mhz: "2440".to_string(),
            frequency_unit: ScalarUnit::MegaHertz,
            reference_ohm: "50".to_string(),
            span_mhz: "0.5".to_string(),
            span_unit: ScalarUnit::MegaHertz,
            resolution: "10".to_string(),
            show_ideal: false,
            diagram_mode: SmithChartDiagramMode::TwoDimensional,
            smith_sphere_yaw: -0.65,
            smith_sphere_pitch: 0.35,
            show_grid: true,
            show_admittance: false,
            stacked_layout: false,
            show_vswr: true,
            show_q: true,
            show_stability_circles: true,
            show_s11_trace: true,
            show_s21_trace: true,
            show_s12_trace: true,
            show_s22_trace: true,
            conjugate_s_parameter_traces: false,
            vswr_circle_input_db: false,
            resistance_label_list: "0;0.2;0.5;1;2;4;10".to_string(),
            reactance_label_list: "0.2;0.5;1;2;4;10;-0.2;-0.5;-1;-2;-4;-10".to_string(),
            marker_list: String::new(),
            q_circle_list: String::new(),
            vswr_circle_list: String::new(),
            noise_figure_circle_list: String::new(),
            gain_input_circle_list: String::new(),
            gain_output_circle_list: String::new(),
            series_resistance_enabled: false,
            series_resistance_ohm: "0".to_string(),
            series_resistance_esl_nh: "0".to_string(),
            series_resistance_tolerance: "0".to_string(),
            series_resistance_slider: "0".to_string(),
            shunt_resistance_enabled: false,
            shunt_resistance_ohm: "100".to_string(),
            shunt_resistance_esl_nh: "0".to_string(),
            shunt_resistance_tolerance: "0".to_string(),
            shunt_resistance_slider: "0".to_string(),
            series_inductance_enabled: false,
            series_inductance_nh: "10".to_string(),
            series_inductance_esr: "0".to_string(),
            series_inductance_tolerance: "0".to_string(),
            series_inductance_slider: "0".to_string(),
            shunt_inductance_enabled: false,
            shunt_inductance_nh: "10".to_string(),
            shunt_inductance_esr: "0".to_string(),
            shunt_inductance_tolerance: "0".to_string(),
            shunt_inductance_slider: "0".to_string(),
            series_capacitance_enabled: false,
            series_capacitance_pf: "1".to_string(),
            series_capacitance_esr: "0".to_string(),
            series_capacitance_esl_nh: "0".to_string(),
            series_capacitance_tolerance: "0".to_string(),
            series_capacitance_slider: "0".to_string(),
            shunt_capacitance_enabled: false,
            shunt_capacitance_pf: "1".to_string(),
            shunt_capacitance_esr: "0".to_string(),
            shunt_capacitance_esl_nh: "0".to_string(),
            shunt_capacitance_tolerance: "0".to_string(),
            shunt_capacitance_slider: "0".to_string(),
            rlc_enabled: false,
            rlc_resistance_ohm: "1000".to_string(),
            rlc_inductance_nh: "10".to_string(),
            rlc_capacitance_pf: "1".to_string(),
            rlc_resistance_slider: "0".to_string(),
            line_enabled: false,
            line_length_mm: "10".to_string(),
            line_impedance_ohm: "50".to_string(),
            line_eeff: "1".to_string(),
            line_tolerance: "0".to_string(),
            line_length_slider: "0".to_string(),
            open_stub_enabled: false,
            open_stub_length_mm: "10".to_string(),
            open_stub_impedance_ohm: "50".to_string(),
            open_stub_eeff: "1".to_string(),
            open_stub_tolerance: "0".to_string(),
            open_stub_length_slider: "0".to_string(),
            short_stub_enabled: false,
            short_stub_length_mm: "10".to_string(),
            short_stub_impedance_ohm: "50".to_string(),
            short_stub_eeff: "1".to_string(),
            short_stub_tolerance: "0".to_string(),
            short_stub_length_slider: "0".to_string(),
            short_stub_warning_seen: false,
            transformer_enabled: false,
            transformer_model: TransformerModel::CoupledInductor,
            transformer_ratio: "1".to_string(),
            transformer_l1_nh: "1".to_string(),
            transformer_l2_nh: "1".to_string(),
            transformer_coupling: "1".to_string(),
            custom_enabled: false,
            custom_interpolation: CustomInterpolation::Linear,
            custom_points: "900,50,0;1000,40,10;1100,55,-5".to_string(),
            s_parameter_enabled: false,
            s_parameter_content: text_editor::Content::with_text(&s_parameter_text),
            s_parameter_text,
            ordered_circuit_enabled: false,
            ordered_circuit_tokens: String::new(),
            ordered_circuit_error: None,
            url_query: String::new(),
            url_error: None,
            file_status: None,
            csv_export_configuration: None,
        }
    }
}

fn parse_csv_frequency(label: &str, value: &str) -> Result<f64, String> {
    let frequency = value
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("{label} must be a number."))?;
    if !frequency.is_finite() || frequency <= 0.0 {
        return Err(format!("{label} must be greater than zero."));
    }
    Ok(frequency)
}

fn s21_samples(circuit: &[SmithChartElement]) -> Vec<(f64, Complex)> {
    let mut samples = circuit
        .iter()
        .find_map(|element| match element {
            SmithChartElement::SParameter(block) => Some(
                block
                    .points
                    .iter()
                    .filter_map(|point| point.s21.map(|s21| (point.frequency_hz, s21)))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    samples.sort_by(|left, right| left.0.total_cmp(&right.0));
    samples
}

fn interpolate_complex_sample(samples: &[(f64, Complex)], frequency_hz: f64) -> Option<Complex> {
    let first = samples.first().copied()?;
    if frequency_hz <= first.0 {
        return Some(first.1);
    }
    let last = samples.last().copied()?;
    if frequency_hz >= last.0 {
        return Some(last.1);
    }
    samples.windows(2).find_map(|pair| {
        let [left, right] = pair else {
            return None;
        };
        if frequency_hz < left.0 || frequency_hz > right.0 {
            return None;
        }
        let span = right.0 - left.0;
        if span.abs() <= f64::EPSILON {
            return Some(right.1);
        }
        let ratio = (frequency_hz - left.0) / span;
        Some(left.1 + (right.1 - left.1) * ratio)
    })
}

fn magnitude_db(value: Complex) -> f64 {
    20.0 * value.magnitude().max(1.0e-12).log10()
}

fn estimated_s21_db_for_export(reflection_coefficient: Complex) -> f64 {
    let reflected_power = reflection_coefficient.magnitude().powi(2);
    let transmitted_power = (1.0 - reflected_power).clamp(1.0e-12, 1.0);
    10.0 * transmitted_power.log10()
}

impl Default for SmithChartState {
    fn default() -> Self {
        Self::new()
    }
}

impl SmithChartState {
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
            SmithChartMessage::ShowGridChanged(value) => self.show_grid = value,
            SmithChartMessage::ShowAdmittanceChanged(value) => self.show_admittance = value,
            SmithChartMessage::StackedLayoutChanged(value) => self.stacked_layout = value,
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
                    self.ordered_circuit_tokens =
                        serialize_online_smith_chart_circuit_tokens(&circuit);
                    self.ordered_circuit_enabled = true;
                    self.ordered_circuit_error = None;
                }
                Err(err) => self.ordered_circuit_error = Some(err),
            },
            SmithChartMessage::UrlQueryChanged(value) => {
                self.url_query = value;
                self.url_error = None;
            }
            SmithChartMessage::ExportUrlQuery => match self.url_state() {
                Ok((circuit, settings)) => match self.url_overlays() {
                    Ok(overlays) => {
                        self.url_query =
                            serialize_online_smith_chart_query(&circuit, &settings, &overlays);
                        self.url_error = None;
                    }
                    Err(err) => self.url_error = Some(err),
                },
                Err(err) => self.url_error = Some(err),
            },
            SmithChartMessage::CopyUrl => match self.generated_online_smith_chart_url() {
                Ok(url) => self.file_status = Some(format!("Copied URL to clipboard: {url}")),
                Err(err) => self.url_error = Some(err),
            },
            SmithChartMessage::OpenReferenceLink(link) => {
                self.file_status = Some(format!("Opening reference link: {}", link.url()));
            }
            SmithChartMessage::ReferenceLinkOpened(result) => match result {
                Ok(()) => self.file_status = Some("Opened reference link".to_string()),
                Err(err) => self.file_status = Some(err),
            },
            SmithChartMessage::ImportUrlQuery => {
                match parse_online_smith_chart_query(&self.url_query) {
                    Ok(state) => {
                        self.apply_url_state(state);
                        self.url_error = None;
                        self.file_status = Some("Imported OnlineSmithChart URL state".to_string());
                    }
                    Err(err) => self.url_error = Some(err.to_string()),
                }
            }
            SmithChartMessage::ClearImportedUrlState => {
                self.reset_url_state_preserving_preferences();
                self.file_status = Some("Cleared OnlineSmithChart URL state".to_string());
            }
            SmithChartMessage::OpenCsvExport(kind) => self.open_csv_export(kind),
            SmithChartMessage::CsvExportStartFrequencyChanged(value) => {
                if let Some(configuration) = &mut self.csv_export_configuration {
                    configuration.start_frequency_mhz = value;
                    configuration.error = None;
                }
            }
            SmithChartMessage::CsvExportStopFrequencyChanged(value) => {
                if let Some(configuration) = &mut self.csv_export_configuration {
                    configuration.stop_frequency_mhz = value;
                    configuration.error = None;
                }
            }
            SmithChartMessage::CsvExportSamplesChanged(value) => {
                if let Some(configuration) = &mut self.csv_export_configuration {
                    configuration.samples = value;
                    configuration.error = None;
                }
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
                self.reset_url_state_preserving_preferences();
            }
        }
    }

    fn reset_url_state_preserving_preferences(&mut self) {
        let stacked_layout = self.stacked_layout;
        *self = Self::new();
        self.stacked_layout = stacked_layout;
    }

    fn warn_shorted_stub_once(&mut self) {
        if !self.short_stub_warning_seen {
            self.short_stub_warning_seen = true;
            self.file_status = Some(SHORTED_STUB_WARNING.to_string());
        }
    }

    pub fn generated_online_smith_chart_url(&mut self) -> Result<String, String> {
        let (circuit, settings) = self.url_state()?;
        let overlays = self.url_overlays()?;
        self.url_query = serialize_online_smith_chart_query(&circuit, &settings, &overlays);
        self.url_error = None;
        Ok(online_smith_chart_url(&self.url_query))
    }

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

    pub fn generated_csv_export(&self) -> Result<(String, String), String> {
        let configuration = self
            .csv_export_configuration
            .as_ref()
            .ok_or_else(|| "Select a result diagram to export first.".to_string())?;
        let start_frequency_mhz =
            parse_csv_frequency("start frequency", &configuration.start_frequency_mhz)?;
        let stop_frequency_mhz =
            parse_csv_frequency("stop frequency", &configuration.stop_frequency_mhz)?;
        if stop_frequency_mhz <= start_frequency_mhz {
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

        let start_frequency_hz = start_frequency_mhz * 1.0e6;
        let stop_frequency_hz = stop_frequency_mhz * 1.0e6;
        let step_hz = (stop_frequency_hz - start_frequency_hz) / (samples - 1) as f64;
        let frequencies_hz = (0..samples)
            .map(|index| start_frequency_hz + index as f64 * step_hz)
            .collect::<Vec<_>>();
        let (circuit, settings) = self.url_state()?;
        let results = solve_frequency_points(
            &circuit,
            &frequencies_hz,
            settings.show_ideal,
            settings.reference_impedance_ohm,
        )
        .map_err(|err| err.to_string())?;
        let measured_s21 = s21_samples(&circuit);

        let mut csv = format!("Frequency [MHz],{}\r\n", configuration.kind.value_label());
        for result in results {
            let value = match configuration.kind {
                ResultDiagramKind::ImpedanceMagnitude => result.impedance.magnitude(),
                ResultDiagramKind::S11Db => magnitude_db(result.reflection_coefficient),
                ResultDiagramKind::S21Db => {
                    if measured_s21.len() >= 2 {
                        interpolate_complex_sample(&measured_s21, result.frequency_hz)
                            .map(magnitude_db)
                            .unwrap_or_else(|| {
                                estimated_s21_db_for_export(result.reflection_coefficient)
                            })
                    } else {
                        estimated_s21_db_for_export(result.reflection_coefficient)
                    }
                }
            };
            csv.push_str(&format!(
                "{:.12},{value:.12}\r\n",
                result.frequency_hz / 1.0e6
            ));
        }
        Ok((configuration.kind.file_name().to_string(), csv))
    }

    fn open_csv_export(&mut self, kind: ResultDiagramKind) {
        match self.solve() {
            Ok(result) => {
                let start_frequency_mhz = result
                    .frequency_results
                    .first()
                    .map(|point| point.frequency_hz / 1.0e6)
                    .unwrap_or(result.active_frequency_hz / 1.0e6);
                let stop_frequency_mhz = result
                    .frequency_results
                    .last()
                    .map(|point| point.frequency_hz / 1.0e6)
                    .unwrap_or(result.active_frequency_hz / 1.0e6);
                let samples = result.frequency_results.len().max(2);
                self.csv_export_configuration = Some(CsvExportConfiguration::new(
                    kind,
                    start_frequency_mhz,
                    stop_frequency_mhz,
                    samples,
                ));
            }
            Err(err) => self.file_status = Some(err),
        }
    }

    pub(in crate::tool) fn solve(&self) -> Result<SmithChartAnalysis, String> {
        let (circuit, settings) = self.url_state()?;
        analyze_smith_chart(&circuit, settings).map_err(|err| err.to_string())
    }

    pub(in crate::tool) fn overlay_circles(
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

    fn solve_state(&self) -> Result<(Vec<SmithChartElement>, SmithChartSettings), String> {
        self.url_state()
    }

    fn url_state(&self) -> Result<(Vec<SmithChartElement>, SmithChartSettings), String> {
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

    fn sync_s_parameter_sweep_from_text(&mut self) {
        let Ok(block) = parse_touchstone_input(&self.s_parameter_text) else {
            return;
        };
        self.sync_s_parameter_sweep(&block);
    }

    fn sync_s_parameter_sweep(&mut self, block: &crate::SParameterBlock) {
        let Some(first) = block.points.first() else {
            return;
        };
        let middle = &block.points[block.points.len() / 2];
        let last = block.points.last().unwrap_or(first);
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

    fn url_overlays(&self) -> Result<SmithChartOverlays, String> {
        Ok(SmithChartOverlays {
            z_markers: parse_marker_list(&self.marker_list)?,
            vswr_circles: parse_vswr_circle_list(
                &self.vswr_circle_list,
                self.vswr_circle_input_db,
            )?,
            q_circles: parse_scalar_list(&self.q_circle_list)?,
            noise_figure_circles: parse_scalar_list(&self.noise_figure_circle_list)?,
            gain_input_circles: parse_scalar_list(&self.gain_input_circle_list)?,
            gain_output_circles: parse_scalar_list(&self.gain_output_circle_list)?,
        })
    }

    pub(in crate::tool) fn active_circuit(&self) -> Result<Vec<SmithChartElement>, String> {
        self.circuit_components
            .iter()
            .map(CircuitEditorComponent::to_element)
            .collect()
    }

    fn set_black_box_field(&mut self, field: CircuitComponentField, value: String) {
        if let Some(component) = self.circuit_components.first_mut() {
            if component.kind == CircuitComponentKind::BlackBox {
                component.set_field(field, value);
            }
        }
    }

    fn ordered_circuit_rows(&self) -> Vec<String> {
        split_online_smith_chart_circuit_tokens(&self.ordered_circuit_tokens)
            .into_iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn set_ordered_circuit_rows(&mut self, rows: Vec<String>) {
        self.ordered_circuit_tokens = rows.join("__");
        self.ordered_circuit_enabled = true;
        self.ordered_circuit_error = None;
    }

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
            if block.kind == SParameterKind::S1P {
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

    pub(super) fn apply_url_state(&mut self, state: crate::SmithChartSnapshot) {
        let black_box = state
            .circuit
            .iter()
            .find(|element| matches!(element, SmithChartElement::BlackBox { .. }))
            .or_else(|| {
                state
                    .circuit
                    .iter()
                    .find(|element| matches!(element, SmithChartElement::LoadTermination { .. }))
            })
            .map(CircuitEditorComponent::from_element)
            .unwrap_or_else(|| CircuitEditorComponent::new(CircuitComponentKind::BlackBox));
        self.circuit_components = std::iter::once(black_box)
            .chain(
                state
                    .circuit
                    .iter()
                    .filter(|element| {
                        !matches!(
                            element,
                            SmithChartElement::BlackBox { .. }
                                | SmithChartElement::LoadTermination { .. }
                        )
                    })
                    .map(CircuitEditorComponent::from_element),
            )
            .collect();
        self.frequency_unit = state.settings.frequency_unit;
        self.frequency_mhz =
            format_number(state.settings.frequency_hz / state.settings.frequency_unit.multiplier());
        self.reference_ohm = format_number(state.settings.reference_impedance_ohm);
        self.span_unit = state.settings.span_unit;
        self.span_mhz =
            format_number(state.settings.span_hz / state.settings.span_unit.multiplier());
        self.resolution = state.settings.resolution.to_string();
        self.show_ideal = state.settings.show_ideal;
        self.vswr_circle_input_db = false;
        self.marker_list = format_marker_list(&state.overlays.z_markers);
        self.vswr_circle_list = format_scalar_list(&state.overlays.vswr_circles);
        self.q_circle_list = format_scalar_list(&state.overlays.q_circles);
        self.noise_figure_circle_list = format_scalar_list(&state.overlays.noise_figure_circles);
        self.gain_input_circle_list = format_scalar_list(&state.overlays.gain_input_circles);
        self.gain_output_circle_list = format_scalar_list(&state.overlays.gain_output_circles);
        self.ordered_circuit_tokens = serialize_online_smith_chart_circuit_tokens(&state.circuit);
        self.ordered_circuit_enabled = state.circuit.len() > 1;
        self.ordered_circuit_error = None;

        self.series_resistance_enabled = false;
        self.shunt_resistance_enabled = false;
        self.series_inductance_enabled = false;
        self.shunt_inductance_enabled = false;
        self.series_capacitance_enabled = false;
        self.shunt_capacitance_enabled = false;
        self.rlc_enabled = false;
        self.line_enabled = false;
        self.open_stub_enabled = false;
        self.short_stub_enabled = false;
        self.transformer_enabled = false;
        self.custom_enabled = false;
        self.s_parameter_enabled = false;
        self.reset_runtime_sliders();

        for element in state.circuit {
            match element {
                SmithChartElement::BlackBox {
                    impedance,
                    tolerance_percent,
                }
                | SmithChartElement::LoadTermination {
                    impedance,
                    tolerance_percent,
                } => {
                    self.load_re = format_number(impedance.re);
                    self.load_im = format_number(impedance.im);
                    self.load_tolerance = format_optional(tolerance_percent);
                }
                SmithChartElement::SeriesResistor {
                    resistance_ohm,
                    esl_h,
                    tolerance_percent,
                } => {
                    self.series_resistance_enabled = true;
                    self.series_resistance_ohm = format_number(resistance_ohm);
                    self.series_resistance_esl_nh = format_number(esl_h / 1.0e-9);
                    self.series_resistance_tolerance = format_optional(tolerance_percent);
                }
                SmithChartElement::ShuntResistor {
                    resistance_ohm,
                    esl_h,
                    tolerance_percent,
                } => {
                    self.shunt_resistance_enabled = true;
                    self.shunt_resistance_ohm = format_number(resistance_ohm);
                    self.shunt_resistance_esl_nh = format_number(esl_h / 1.0e-9);
                    self.shunt_resistance_tolerance = format_optional(tolerance_percent);
                }
                SmithChartElement::SeriesInductor {
                    inductance_h,
                    esr_ohm,
                    tolerance_percent,
                } => {
                    self.series_inductance_enabled = true;
                    self.series_inductance_nh = format_number(inductance_h / 1.0e-9);
                    self.series_inductance_esr = format_number(esr_ohm);
                    self.series_inductance_tolerance = format_optional(tolerance_percent);
                }
                SmithChartElement::ShuntInductor {
                    inductance_h,
                    esr_ohm,
                    tolerance_percent,
                } => {
                    self.shunt_inductance_enabled = true;
                    self.shunt_inductance_nh = format_number(inductance_h / 1.0e-9);
                    self.shunt_inductance_esr = format_number(esr_ohm);
                    self.shunt_inductance_tolerance = format_optional(tolerance_percent);
                }
                SmithChartElement::SeriesCapacitor {
                    capacitance_f,
                    esr_ohm,
                    esl_h,
                    tolerance_percent,
                } => {
                    self.series_capacitance_enabled = true;
                    self.series_capacitance_pf = format_number(capacitance_f / 1.0e-12);
                    self.series_capacitance_esr = format_number(esr_ohm);
                    self.series_capacitance_esl_nh = format_number(esl_h / 1.0e-9);
                    self.series_capacitance_tolerance = format_optional(tolerance_percent);
                }
                SmithChartElement::ShuntCapacitor {
                    capacitance_f,
                    esr_ohm,
                    esl_h,
                    tolerance_percent,
                } => {
                    self.shunt_capacitance_enabled = true;
                    self.shunt_capacitance_pf = format_number(capacitance_f / 1.0e-12);
                    self.shunt_capacitance_esr = format_number(esr_ohm);
                    self.shunt_capacitance_esl_nh = format_number(esl_h / 1.0e-9);
                    self.shunt_capacitance_tolerance = format_optional(tolerance_percent);
                }
                SmithChartElement::SeriesParallelRlc {
                    resistance_ohm,
                    inductance_h,
                    capacitance_f,
                } => {
                    self.rlc_enabled = true;
                    self.rlc_resistance_ohm = format_number(resistance_ohm);
                    self.rlc_inductance_nh = format_number(inductance_h / 1.0e-9);
                    self.rlc_capacitance_pf = format_number(capacitance_f / 1.0e-12);
                }
                SmithChartElement::TransmissionLine {
                    length_m,
                    characteristic_impedance_ohm,
                    effective_dielectric,
                    tolerance_percent,
                } => {
                    self.line_enabled = true;
                    self.line_length_mm = format_number(length_m / 1.0e-3);
                    self.line_impedance_ohm = format_number(characteristic_impedance_ohm);
                    self.line_eeff = format_number(effective_dielectric);
                    self.line_tolerance = format_optional(tolerance_percent);
                }
                SmithChartElement::OpenStub {
                    length_m,
                    characteristic_impedance_ohm,
                    effective_dielectric,
                    tolerance_percent,
                } => {
                    self.open_stub_enabled = true;
                    self.open_stub_length_mm = format_number(length_m / 1.0e-3);
                    self.open_stub_impedance_ohm = format_number(characteristic_impedance_ohm);
                    self.open_stub_eeff = format_number(effective_dielectric);
                    self.open_stub_tolerance = format_optional(tolerance_percent);
                }
                SmithChartElement::ShortedStub {
                    length_m,
                    characteristic_impedance_ohm,
                    effective_dielectric,
                    tolerance_percent,
                } => {
                    self.short_stub_enabled = true;
                    self.short_stub_length_mm = format_number(length_m / 1.0e-3);
                    self.short_stub_impedance_ohm = format_number(characteristic_impedance_ohm);
                    self.short_stub_eeff = format_number(effective_dielectric);
                    self.short_stub_tolerance = format_optional(tolerance_percent);
                }
                SmithChartElement::Transformer {
                    model,
                    l1_h,
                    l2_h,
                    coupling_or_turns_ratio,
                } => {
                    self.transformer_enabled = true;
                    self.transformer_model = model;
                    self.transformer_l1_nh = format_number(l1_h / 1.0e-9);
                    self.transformer_l2_nh = format_number(l2_h / 1.0e-9);
                    match model {
                        TransformerModel::Ideal => {
                            self.transformer_ratio = format_number(coupling_or_turns_ratio);
                            self.transformer_coupling = "0".to_string();
                        }
                        TransformerModel::CoupledInductor => {
                            self.transformer_coupling = format_number(coupling_or_turns_ratio);
                        }
                    }
                }
                SmithChartElement::Custom {
                    points,
                    interpolation,
                } => {
                    self.custom_enabled = true;
                    self.custom_interpolation = interpolation;
                    self.custom_points = points
                        .iter()
                        .map(|point| {
                            format!(
                                "{},{},{}",
                                format_number(point.frequency_hz / 1.0e6),
                                format_number(point.impedance.re),
                                format_number(point.impedance.im)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(";");
                }
                SmithChartElement::SParameter(block) => {
                    self.s_parameter_enabled = true;
                    self.s_parameter_text = block.raw.clone();
                    self.s_parameter_content =
                        text_editor::Content::with_text(&self.s_parameter_text);
                }
            }
        }
    }

    fn reset_runtime_sliders(&mut self) {
        self.load_re_slider = "0".to_string();
        self.load_im_slider = "0".to_string();
        self.series_resistance_slider = "0".to_string();
        self.shunt_resistance_slider = "0".to_string();
        self.series_inductance_slider = "0".to_string();
        self.shunt_inductance_slider = "0".to_string();
        self.series_capacitance_slider = "0".to_string();
        self.shunt_capacitance_slider = "0".to_string();
        self.rlc_resistance_slider = "0".to_string();
        self.line_length_slider = "0".to_string();
        self.open_stub_length_slider = "0".to_string();
        self.short_stub_length_slider = "0".to_string();
    }
}
