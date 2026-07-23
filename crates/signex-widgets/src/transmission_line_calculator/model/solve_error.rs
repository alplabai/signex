use crate::transmission_line_calculator::ElementKind;
use thiserror::Error;

/// Describes invalid inputs or singular networks that prevent circuit solving.
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
    #[error("touchstone read failed: {reason}")]
    TouchstoneReadFailed { reason: String },
    #[error("touchstone write failed: {reason}")]
    TouchstoneWriteFailed { reason: String },
}
