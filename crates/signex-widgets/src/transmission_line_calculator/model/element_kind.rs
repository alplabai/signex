use serde::{Deserialize, Serialize};

/// Identifies the legacy circuit-element model used by the network solver.
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
