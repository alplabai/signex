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

pub(crate) fn reflection_coefficient(impedance: Complex, reference_impedance_ohm: f64) -> Complex {
    from_rf_complex(rust_rf::transmission_line::reflection_coefficient(
        RfComplex::new(reference_impedance_ohm, 0.0),
        to_rf_complex(impedance),
    ))
}

pub(crate) fn impedance_from_reflection(
    reflection: Complex,
    reference_impedance_ohm: f64,
) -> Complex {
    from_rf_complex(rust_rf::transmission_line::impedance_from_reflection(
        RfComplex::new(reference_impedance_ohm, 0.0),
        to_rf_complex(reflection),
    ))
}

pub(crate) fn lossless_line_input_impedance(
    load_impedance: Complex,
    characteristic_impedance_ohm: f64,
    electrical_length_rad: f64,
) -> Complex {
    from_rf_complex(
        rust_rf::transmission_line::input_impedance_at_electrical_length(
            RfComplex::new(characteristic_impedance_ohm, 0.0),
            to_rf_complex(load_impedance),
            RfComplex::new(0.0, electrical_length_rad),
        ),
    )
}

pub(crate) fn standing_wave_ratio(reflection: Complex) -> f64 {
    if reflection.magnitude() >= 1.0 {
        f64::INFINITY
    } else {
        rust_rf::transmission_line::standing_wave_ratio(to_rf_complex(reflection))
    }
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
