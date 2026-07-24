use super::rust_rf_adapter::{
    RfNetwork, frequency_from_hz, from_rf_complex_array, to_rf_complex_array,
};
use super::{Complex, SolveError};
use ndarray::{Array2, Array3};

#[test]
fn complex_array_conversion_round_trips() {
    let values = [Complex::new(1.0, -2.0), Complex::new(3.5, 4.25)];

    let converted = to_rf_complex_array(&values);
    let restored = from_rf_complex_array(converted.view());

    assert_eq!(restored, values);
}

#[test]
fn network_boundary_accepts_widget_values() {
    let frequency = frequency_from_hz(&[1.0e9]).unwrap();
    let scattering = Array3::from_elem(
        (1, 1, 1),
        to_rf_complex_array(&[Complex::new(0.25, -0.5)])[0],
    );
    let reference = Array2::from_elem((1, 1), rust_rf::Complex64::new(50.0, 0.0));

    let network = RfNetwork::new(frequency, scattering, reference).unwrap();

    assert_eq!(network.frequency_points(), 1);
    assert_eq!(network.ports(), 1);
}

#[test]
fn rust_rf_errors_map_to_solver_errors() {
    let error = frequency_from_hz(&[f64::NAN]).unwrap_err();

    assert!(matches!(
        error,
        SolveError::RfCalculationFailed { reason }
            if reason.contains("invalid frequency axis")
    ));
}
