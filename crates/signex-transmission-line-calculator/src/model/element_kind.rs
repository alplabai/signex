use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElementKind {
    Load,
    SeriesResistor,
    ShuntResistor,
    SeriesCapacitor,
    ShuntCapacitor,
    SeriesInductor,
    ShuntInductor,
    TransmissionLine,
    OpenStub,
    ShortedStub,
    SeriesRlc,
    Custom,
    IdealTransformer,
    CoupledTransformer,
    SParameter,
    LoadTerm,
}
