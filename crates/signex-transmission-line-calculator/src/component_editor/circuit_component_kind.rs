use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitComponentKind {
    BlackBox,
    ShortedCapacitor,
    SeriesCapacitor,
    ShortedInductor,
    SeriesInductor,
    ShortedResistor,
    SeriesResistor,
    ParallelRlc,
    Custom,
    TransmissionLine,
    Stub,
    ShortedStub,
    Transformer,
    SParameters,
}

impl CircuitComponentKind {
    pub const PICKER_OPTIONS: [Self; 13] = [
        Self::ShortedCapacitor,
        Self::SeriesCapacitor,
        Self::ShortedInductor,
        Self::SeriesInductor,
        Self::ShortedResistor,
        Self::SeriesResistor,
        Self::ParallelRlc,
        Self::Custom,
        Self::TransmissionLine,
        Self::Stub,
        Self::ShortedStub,
        Self::Transformer,
        Self::SParameters,
    ];
}

impl fmt::Display for CircuitComponentKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::BlackBox => "Black Box",
            Self::ShortedCapacitor => "Shorted Capacitor",
            Self::SeriesCapacitor => "Series Capacitor",
            Self::ShortedInductor => "Shorted Inductor",
            Self::SeriesInductor => "Series Inductor",
            Self::ShortedResistor => "Shorted Resistor",
            Self::SeriesResistor => "Series Resistor",
            Self::ParallelRlc => "Parallel RLC",
            Self::Custom => "Custom Z(f)",
            Self::TransmissionLine => "Transmission Line (Z_0)",
            Self::Stub => "Stub",
            Self::ShortedStub => "Shorted Stub",
            Self::Transformer => "Transformer (L1 L2)",
            Self::SParameters => "S-Parameters",
        })
    }
}
