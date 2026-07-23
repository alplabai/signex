use crate::transmission_line_calculator::{
    Complex, CustomInterpolation, FrequencyScale, ScalarUnit, SmithViewTransform, TransformerModel,
};
use iced::widget::text_editor;

use crate::transmission_line_calculator::tool::component_editor::{
    CircuitComponentKind, CircuitEditorComponent, DEFAULT_S_PARAMETER_TEXT,
};
use crate::transmission_line_calculator::tool::results::CsvExportConfiguration;

use super::*;

pub(super) const SHORTED_STUB_WARNING: &str = "A shorted shunt stub starts at zero impedance when its length is zero. Confirm that this topology is intended before continuing.";

/// Stores calculator inputs, derived analysis, display settings, and editor state.
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
    pub result_frequency_scale: FrequencyScale,
    pub diagram_mode: SmithChartDiagramMode,
    pub smith_sphere_yaw: f32,
    pub smith_sphere_pitch: f32,
    pub smith_view_transform: SmithViewTransform,
    pub show_grid: bool,
    pub show_admittance: bool,
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
    pub file_status: Option<String>,
    pub(in crate::transmission_line_calculator::tool) csv_export_configuration:
        Option<CsvExportConfiguration>,
}

impl SmithChartState {
    /// Creates the initial calculator state and evaluates its default circuit.
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
            result_frequency_scale: FrequencyScale::Linear,
            diagram_mode: SmithChartDiagramMode::TwoDimensional,
            smith_sphere_yaw: -0.65,
            smith_sphere_pitch: 0.35,
            smith_view_transform: SmithViewTransform::identity(),
            show_grid: true,
            show_admittance: false,
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
            file_status: None,
            csv_export_configuration: None,
        }
    }
}

/// Parses CSV frequency from its textual representation.
pub(super) fn parse_csv_frequency(label: &str, value: &str) -> Result<f64, String> {
    let frequency = value
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("{label} must be a number."))?;
    if !frequency.is_finite() || frequency <= 0.0 {
        return Err(format!("{label} must be greater than zero."));
    }
    Ok(frequency)
}

/// Converts a complex magnitude to decibels with a finite floor.
pub(super) fn magnitude_db(value: Complex) -> f64 {
    20.0 * value.magnitude().max(1.0e-12).log10()
}

/// Estimates exported S21 in decibels from a reflection coefficient.
pub(super) fn estimated_s21_db_for_export(reflection_coefficient: Complex) -> f64 {
    let reflected_power = reflection_coefficient.magnitude().powi(2);
    let transmitted_power = (1.0 - reflected_power).clamp(1.0e-12, 1.0);
    10.0 * transmitted_power.log10()
}

impl Default for SmithChartState {
    /// Creates the default value for this type.
    fn default() -> Self {
        Self::new()
    }
}
