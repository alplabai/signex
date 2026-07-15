use crate::Complex;

use super::{ImpedanceArcTrace, SParameterTrace};

#[derive(Debug, Clone)]
pub(crate) struct SmithChart3dCanvas {
    pub(crate) point: Option<Complex>,
    pub(crate) show_grid: bool,
    pub(crate) resistance_labels: Vec<f64>,
    pub(crate) reactance_labels: Vec<f64>,
    pub(crate) markers: Vec<Complex>,
    pub(crate) reference_impedance_ohm: f64,
    pub(crate) impedance_arc_traces: Vec<ImpedanceArcTrace>,
    pub(crate) s_parameter_traces: Vec<SParameterTrace>,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}
