use thiserror::Error;

/// Describes invalid references or singular matrices encountered by two-port calculations.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum TwoPortError {
    #[error("port reference impedances must be finite and have a positive real part")]
    InvalidReferenceImpedance,
    #[error("S-to-ABCD conversion is singular because S21 is zero")]
    SingularSParameterMatrix,
    #[error("ABCD-to-S conversion has a zero denominator")]
    SingularAbcdMatrix,
    #[error("the terminated two-port network has a zero input-impedance denominator")]
    SingularTermination,
}
