use crate::{
    FrequencyPointResult, GainPoint, ImpedanceArc, NoiseFigurePoint, S1pReflectionPoint,
    SolveResult, StabilityCircle,
};
use serde::{Deserialize, Serialize};

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
    pub noise_figure: Vec<NoiseFigurePoint>,
    pub stability_circles: Vec<StabilityCircle>,
    pub active_frequency_hz: f64,
}
