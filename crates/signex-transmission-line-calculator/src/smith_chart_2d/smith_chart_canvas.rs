use crate::{
    Complex, FrequencyPointResult, GainCircle, NoiseFigureCircle, ScalarUnit, StabilityCircle,
};

use super::{ImpedanceArcTrace, SParameterTrace};

#[derive(Debug, Clone)]
pub(crate) struct SmithChartCanvas {
    pub(crate) point: Option<Complex>,
    pub(crate) frequency_results: Vec<FrequencyPointResult>,
    pub(crate) active_frequency_hz: f64,
    pub(crate) frequency_unit: ScalarUnit,
    pub(crate) show_grid: bool,
    pub(crate) show_admittance: bool,
    pub(crate) admittance_chart: bool,
    pub(crate) show_vswr: bool,
    pub(crate) show_q: bool,
    pub(crate) resistance_labels: Vec<f64>,
    pub(crate) reactance_labels: Vec<f64>,
    pub(crate) markers: Vec<Complex>,
    pub(crate) q_circles: Vec<f64>,
    pub(crate) vswr_circles: Vec<f64>,
    pub(crate) reference_impedance_ohm: f64,
    pub(crate) stability_circles: Vec<StabilityCircle>,
    pub(crate) gain_circles: Vec<GainCircle>,
    pub(crate) noise_figure_circles: Vec<NoiseFigureCircle>,
    pub(crate) impedance_arc_traces: Vec<ImpedanceArcTrace>,
    pub(crate) s_parameter_traces: Vec<SParameterTrace>,
}
