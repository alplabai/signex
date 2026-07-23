use crate::transmission_line_calculator::{Complex, SolveError};
use ndarray::{Array1, ArrayView1};

pub(crate) type RfComplex = rust_rf::Complex64;
pub(crate) type RfFrequency = rust_rf::Frequency;
pub(crate) type RfNetwork = rust_rf::Network;

pub(crate) fn to_rf_complex(value: Complex) -> RfComplex {
    RfComplex::new(value.re, value.im)
}

pub(crate) fn from_rf_complex(value: RfComplex) -> Complex {
    Complex::new(value.re, value.im)
}

pub(crate) fn to_rf_complex_array(values: &[Complex]) -> Array1<RfComplex> {
    Array1::from_iter(values.iter().copied().map(to_rf_complex))
}

pub(crate) fn from_rf_complex_array(values: ArrayView1<'_, RfComplex>) -> Vec<Complex> {
    values.iter().copied().map(from_rf_complex).collect()
}

pub(crate) fn frequency_from_hz(values_hz: &[f64]) -> Result<RfFrequency, SolveError> {
    RfFrequency::from_hz(Array1::from_iter(values_hz.iter().copied())).map_err(map_rf_error)
}

pub(crate) fn map_rf_error(error: rust_rf::Error) -> SolveError {
    SolveError::RfCalculationFailed {
        reason: error.to_string(),
    }
}

pub(crate) fn map_touchstone_parse_error(error: rust_rf::Error) -> SolveError {
    SolveError::TouchstoneParseFailed {
        reason: error.to_string(),
    }
}
