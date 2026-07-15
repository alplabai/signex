use crate::{
    Complex, DEFAULT_REFERENCE_IMPEDANCE_OHM, GainCircle, NoiseFigureCircle, SmithChartSvgTrace,
    StabilityCircle,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmithChartSvgOptions {
    pub width: f64,
    pub height: f64,
    pub reference_impedance_ohm: f64,
    pub show_grid: bool,
    pub show_admittance: bool,
    pub show_vswr: bool,
    pub show_q: bool,
    pub resistance_labels: Vec<f64>,
    pub reactance_labels: Vec<f64>,
    pub z_markers: Vec<Complex>,
    pub vswr_circles: Vec<f64>,
    pub q_circles: Vec<f64>,
    pub stability_circles: Vec<StabilityCircle>,
    pub gain_circles: Vec<GainCircle>,
    pub noise_figure_circles: Vec<NoiseFigureCircle>,
    pub impedance_arc_traces: Vec<SmithChartSvgTrace>,
    pub s_parameter_traces: Vec<SmithChartSvgTrace>,
}

impl Default for SmithChartSvgOptions {
    fn default() -> Self {
        Self {
            width: 900.0,
            height: 900.0,
            reference_impedance_ohm: DEFAULT_REFERENCE_IMPEDANCE_OHM,
            show_grid: true,
            show_admittance: false,
            show_vswr: true,
            show_q: true,
            resistance_labels: vec![0.2, 0.5, 1.0, 2.0, 5.0],
            reactance_labels: vec![-2.0, -1.0, -0.5, 0.5, 1.0, 2.0],
            z_markers: Vec::new(),
            vswr_circles: vec![1.5, 3.0, 7.0],
            q_circles: vec![0.5, 1.0, 2.0],
            stability_circles: Vec::new(),
            gain_circles: Vec::new(),
            noise_figure_circles: Vec::new(),
            impedance_arc_traces: Vec::new(),
            s_parameter_traces: Vec::new(),
        }
    }
}
