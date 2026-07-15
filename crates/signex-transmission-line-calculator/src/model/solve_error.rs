use crate::ElementKind;
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum SolveError {
    #[error("frequency must be positive")]
    NonPositiveFrequency,
    #[error("reference impedance must be positive")]
    NonPositiveReferenceImpedance,
    #[error("velocity factor must be positive")]
    NonPositiveVelocityFactor,
    #[error("{kind:?} value must be positive")]
    NonPositiveElementValue { kind: ElementKind },
    #[error("network contains an open-circuit or zero denominator at {kind:?}")]
    SingularNetwork { kind: ElementKind },
    #[error("circuit must start with a black box or s-parameter element")]
    MissingSourceElement,
    #[error("touchstone parse failed: {reason}")]
    TouchstoneParseFailed { reason: String },
}
