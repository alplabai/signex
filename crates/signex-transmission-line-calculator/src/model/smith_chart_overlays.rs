use crate::Complex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SmithChartOverlays {
    pub z_markers: Vec<Complex>,
    pub vswr_circles: Vec<f64>,
    pub q_circles: Vec<f64>,
    pub noise_figure_circles: Vec<f64>,
    pub gain_input_circles: Vec<f64>,
    pub gain_output_circles: Vec<f64>,
}
