use crate::{Complex, CustomInterpolation, CustomPoint, SParameterBlock, TransformerModel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SmithChartElement {
    BlackBox {
        impedance: Complex,
        tolerance_percent: Option<f64>,
    },
    LoadTermination {
        impedance: Complex,
        tolerance_percent: Option<f64>,
    },
    SeriesCapacitor {
        capacitance_f: f64,
        esr_ohm: f64,
        esl_h: f64,
        tolerance_percent: Option<f64>,
    },
    ShuntCapacitor {
        capacitance_f: f64,
        esr_ohm: f64,
        esl_h: f64,
        tolerance_percent: Option<f64>,
    },
    SeriesInductor {
        inductance_h: f64,
        esr_ohm: f64,
        tolerance_percent: Option<f64>,
    },
    ShuntInductor {
        inductance_h: f64,
        esr_ohm: f64,
        tolerance_percent: Option<f64>,
    },
    SeriesResistor {
        resistance_ohm: f64,
        esl_h: f64,
        tolerance_percent: Option<f64>,
    },
    ShuntResistor {
        resistance_ohm: f64,
        esl_h: f64,
        tolerance_percent: Option<f64>,
    },
    SeriesParallelRlc {
        resistance_ohm: f64,
        inductance_h: f64,
        capacitance_f: f64,
    },
    Custom {
        points: Vec<CustomPoint>,
        interpolation: CustomInterpolation,
    },
    TransmissionLine {
        length_m: f64,
        characteristic_impedance_ohm: f64,
        effective_dielectric: f64,
        tolerance_percent: Option<f64>,
    },
    OpenStub {
        length_m: f64,
        characteristic_impedance_ohm: f64,
        effective_dielectric: f64,
        tolerance_percent: Option<f64>,
    },
    ShortedStub {
        length_m: f64,
        characteristic_impedance_ohm: f64,
        effective_dielectric: f64,
        tolerance_percent: Option<f64>,
    },
    Transformer {
        model: TransformerModel,
        l1_h: f64,
        l2_h: f64,
        coupling_or_turns_ratio: f64,
    },
    SParameter(SParameterBlock),
}
