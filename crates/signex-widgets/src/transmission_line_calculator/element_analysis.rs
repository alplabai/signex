use super::analysis::TAU;
use super::rust_rf_adapter::lossless_line_input_impedance;
use super::*;

/// Computes the custom complex impedance.
pub(super) fn custom_impedance(
    points: &[CustomPoint],
    frequency_hz: f64,
    interpolation: CustomInterpolation,
) -> Complex {
    if points.is_empty() {
        return Complex::ZERO;
    }
    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a.frequency_hz.total_cmp(&b.frequency_hz));
    if frequency_hz <= sorted[0].frequency_hz {
        return sorted[0].impedance;
    }
    if frequency_hz >= sorted[sorted.len() - 1].frequency_hz {
        return sorted[sorted.len() - 1].impedance;
    }
    for pair in sorted.windows(2) {
        let [left, right] = pair else {
            continue;
        };
        if frequency_hz >= left.frequency_hz && frequency_hz < right.frequency_hz {
            if interpolation == CustomInterpolation::SampleAndHold {
                return left.impedance;
            }
            let ratio =
                (frequency_hz - left.frequency_hz) / (right.frequency_hz - left.frequency_hz);
            return left.impedance + (right.impedance - left.impedance) * ratio;
        }
    }
    Complex::ZERO
}

/// Creates a solve-step summary for a circuit element.
pub(super) fn summary_element(element: &SmithChartElement) -> CircuitElement {
    let (name, kind, value) = match element {
        SmithChartElement::BlackBox { impedance, .. } => {
            ("Black Box", ElementKind::Load, impedance.re)
        }
        SmithChartElement::LoadTermination { impedance, .. } => {
            ("Load Termination", ElementKind::LoadTerm, impedance.re)
        }
        SmithChartElement::SeriesCapacitor { capacitance_f, .. } => (
            "Series Capacitor",
            ElementKind::SeriesCapacitor,
            *capacitance_f,
        ),
        SmithChartElement::ShuntCapacitor { capacitance_f, .. } => (
            "Shorted Capacitor",
            ElementKind::ShuntCapacitor,
            *capacitance_f,
        ),
        SmithChartElement::SeriesInductor { inductance_h, .. } => (
            "Series Inductor",
            ElementKind::SeriesInductor,
            *inductance_h,
        ),
        SmithChartElement::ShuntInductor { inductance_h, .. } => (
            "Shorted Inductor",
            ElementKind::ShuntInductor,
            *inductance_h,
        ),
        SmithChartElement::SeriesResistor { resistance_ohm, .. } => (
            "Series Resistor",
            ElementKind::SeriesResistor,
            *resistance_ohm,
        ),
        SmithChartElement::ShuntResistor { resistance_ohm, .. } => (
            "Shorted Resistor",
            ElementKind::ShuntResistor,
            *resistance_ohm,
        ),
        SmithChartElement::SeriesParallelRlc { resistance_ohm, .. } => {
            ("Parallel RLC", ElementKind::SeriesRlc, *resistance_ohm)
        }
        SmithChartElement::Custom { .. } => ("Custom Z(f)", ElementKind::Custom, 0.0),
        SmithChartElement::TransmissionLine { length_m, .. } => (
            "Transmission Line",
            ElementKind::TransmissionLine,
            *length_m,
        ),
        SmithChartElement::OpenStub { length_m, .. } => ("Stub", ElementKind::OpenStub, *length_m),
        SmithChartElement::ShortedStub { length_m, .. } => {
            ("Shorted Stub", ElementKind::ShortedStub, *length_m)
        }
        SmithChartElement::Transformer { model, l1_h, .. } => match model {
            TransformerModel::Ideal => ("Ideal Transformer", ElementKind::IdealTransformer, 0.0),
            TransformerModel::CoupledInductor => {
                ("Transformer", ElementKind::CoupledTransformer, *l1_h)
            }
        },
        SmithChartElement::SParameter(_) => ("S-Parameter", ElementKind::SParameter, 0.0),
    };
    CircuitElement::new(name, kind, value)
}

/// Computes electrical length in radians at the supplied frequency.
fn electrical_length_rad_at(length_m: f64, frequency_hz: f64, effective_dielectric: f64) -> f64 {
    TAU * frequency_hz * length_m * effective_dielectric.sqrt() / SPEED_OF_LIGHT_M_PER_S
}

/// Transforms a load through a transmission line with an effective dielectric.
pub(super) fn transmission_line_with_dielectric(
    load: Complex,
    length_m: f64,
    characteristic_impedance_ohm: f64,
    frequency_hz: f64,
    effective_dielectric: f64,
) -> Result<Complex, SolveError> {
    let beta_l = electrical_length_rad_at(length_m, frequency_hz, effective_dielectric);
    if lossless_line_is_singular(load, characteristic_impedance_ohm, beta_l) {
        return Err(SolveError::SingularNetwork {
            kind: ElementKind::TransmissionLine,
        });
    }
    Ok(lossless_line_input_impedance(
        load,
        characteristic_impedance_ohm,
        beta_l,
    ))
}

/// Opens stub impedance at for the requested workflow.
pub(super) fn open_stub_impedance_at(
    length_m: f64,
    characteristic_impedance_ohm: f64,
    frequency_hz: f64,
    effective_dielectric: f64,
) -> Result<Complex, SolveError> {
    let tan = electrical_length_rad_at(length_m, frequency_hz, effective_dielectric).tan();
    if tan.abs() <= f64::EPSILON {
        return Err(SolveError::SingularNetwork {
            kind: ElementKind::OpenStub,
        });
    }
    Ok(lossless_line_input_impedance(
        Complex::new(f64::INFINITY, 0.0),
        characteristic_impedance_ohm,
        electrical_length_rad_at(length_m, frequency_hz, effective_dielectric),
    ))
}

/// Computes shorted-stub impedance at the supplied frequency.
pub(super) fn shorted_stub_impedance_at(
    length_m: f64,
    characteristic_impedance_ohm: f64,
    frequency_hz: f64,
    effective_dielectric: f64,
) -> Complex {
    lossless_line_input_impedance(
        Complex::ZERO,
        characteristic_impedance_ohm,
        electrical_length_rad_at(length_m, frequency_hz, effective_dielectric),
    )
}

/// Applies element and returns the resulting value.
pub(super) fn apply_element(
    impedance: Complex,
    element: &CircuitElement,
    settings: SolveSettings,
) -> Result<Complex, SolveError> {
    let omega = TAU * settings.frequency_hz;
    let kind = element.kind;
    let value = element.value;
    let result = match kind {
        ElementKind::Load => impedance,
        ElementKind::SeriesResistor => impedance + Complex::new(value, 0.0),
        ElementKind::ShuntResistor => shunt(impedance, Complex::new(value, 0.0))?,
        ElementKind::SeriesCapacitor => impedance + Complex::new(0.0, -1.0 / (omega * value)),
        ElementKind::ShuntCapacitor => shunt(impedance, Complex::new(0.0, -1.0 / (omega * value)))?,
        ElementKind::SeriesInductor => impedance + Complex::new(0.0, omega * value),
        ElementKind::ShuntInductor => shunt(impedance, Complex::new(0.0, omega * value))?,
        ElementKind::TransmissionLine => {
            transmission_line(impedance, value, settings.reference_impedance_ohm, settings)?
        }
        ElementKind::OpenStub => {
            let stub = open_stub_impedance(value, settings.reference_impedance_ohm, settings)?;
            shunt(impedance, stub)?
        }
        ElementKind::ShortedStub => {
            let stub = shorted_stub_impedance(value, settings.reference_impedance_ohm, settings)?;
            shunt(impedance, stub)?
        }
        ElementKind::SeriesRlc
        | ElementKind::Custom
        | ElementKind::IdealTransformer
        | ElementKind::CoupledTransformer
        | ElementKind::SParameter
        | ElementKind::LoadTerm => impedance,
    };
    Ok(result)
}

/// Combines an impedance with a shunt branch in parallel.
pub(super) fn shunt(a: Complex, b: Complex) -> Result<Complex, SolveError> {
    let sum = a + b;
    if sum.magnitude() <= f64::EPSILON {
        return Err(SolveError::SingularNetwork {
            kind: ElementKind::ShuntResistor,
        });
    }
    Ok((a * b) / sum)
}

/// Converts physical line length to electrical length in radians.
fn electrical_length_rad(length_m: f64, settings: SolveSettings) -> f64 {
    TAU * settings.frequency_hz * length_m / (SPEED_OF_LIGHT_M_PER_S * settings.velocity_factor)
}

/// Transforms a load impedance through a lossless transmission line.
pub(super) fn transmission_line(
    load: Complex,
    length_m: f64,
    characteristic_impedance_ohm: f64,
    settings: SolveSettings,
) -> Result<Complex, SolveError> {
    let beta_l = electrical_length_rad(length_m, settings);
    if lossless_line_is_singular(load, characteristic_impedance_ohm, beta_l) {
        return Err(SolveError::SingularNetwork {
            kind: ElementKind::TransmissionLine,
        });
    }
    Ok(lossless_line_input_impedance(
        load,
        characteristic_impedance_ohm,
        beta_l,
    ))
}

/// Opens stub impedance for the requested workflow.
pub(super) fn open_stub_impedance(
    length_m: f64,
    characteristic_impedance_ohm: f64,
    settings: SolveSettings,
) -> Result<Complex, SolveError> {
    let tan = electrical_length_rad(length_m, settings).tan();
    if tan.abs() <= f64::EPSILON {
        return Err(SolveError::SingularNetwork {
            kind: ElementKind::OpenStub,
        });
    }
    Ok(lossless_line_input_impedance(
        Complex::new(f64::INFINITY, 0.0),
        characteristic_impedance_ohm,
        electrical_length_rad(length_m, settings),
    ))
}

/// Computes the shorted stub complex impedance.
pub(super) fn shorted_stub_impedance(
    length_m: f64,
    characteristic_impedance_ohm: f64,
    settings: SolveSettings,
) -> Result<Complex, SolveError> {
    Ok(lossless_line_input_impedance(
        Complex::ZERO,
        characteristic_impedance_ohm,
        electrical_length_rad(length_m, settings),
    ))
}

fn lossless_line_is_singular(
    load: Complex,
    characteristic_impedance_ohm: f64,
    electrical_length_rad: f64,
) -> bool {
    let z0 = Complex::new(characteristic_impedance_ohm, 0.0);
    let j_tan = Complex::new(0.0, electrical_length_rad.tan());
    (z0 + load * j_tan).magnitude() <= f64::EPSILON
}
