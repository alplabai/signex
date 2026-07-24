use crate::transmission_line_calculator::{
    FrequencyPointResult, GainPoint, ImpedanceArc, NoiseFigurePoint, S1pReflectionPoint,
    SolveResult, StabilityCircle, TwoPortSParameterPoint,
};
use serde::{Deserialize, Serialize};

/// Collects nominal, tolerance, sweep, S-parameter, noise, and stability results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmithChartAnalysis {
    pub nominal: SolveResult,
    pub tolerance_results: Vec<SolveResult>,
    pub impedance_arcs: Vec<ImpedanceArc>,
    pub frequency_results: Vec<FrequencyPointResult>,
    pub frequency_result_variants: Vec<Vec<FrequencyPointResult>>,
    pub s1p_reflection_variants: Vec<Vec<S1pReflectionPoint>>,
    pub s_parameter_gain: Vec<GainPoint>,
    pub s_parameter_gain_variants: Vec<Vec<GainPoint>>,
    #[serde(default)]
    pub two_port_s_parameters: Vec<TwoPortSParameterPoint>,
    pub noise_figure: Vec<NoiseFigurePoint>,
    pub stability_circles: Vec<StabilityCircle>,
    pub active_frequency_hz: f64,
}
