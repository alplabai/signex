use ndarray::{Array1, Array2, Array3};
use rust_rf::{
    Network, SParameterDefinition,
    circuit::Circuit,
    media::{DefinedGammaZ0, LengthUnit, Media},
    network::{abcd_to_s, cascade_list},
};

use crate::transmission_line_calculator::{
    Complex, ElementKind, SParameterMatrix, SmithChartElement, SolveError, TransformerModel,
    TwoPortSParameterPoint,
    chart_geometry::SPEED_OF_LIGHT_M_PER_S,
    element_analysis::custom_impedance,
    rust_rf_adapter::{
        RfComplex, RfFrequency, frequency_from_hz, from_rf_complex, lossless_line_input_impedance,
        to_rf_complex,
    },
};

/// Solves true two-port S-parameters for a supported passive circuit chain.
///
/// The first circuit element is the terminating load used by the Smith-chart
/// impedance walk. Every following element is built over the complete
/// frequency axis and cascaded in source-to-load order.
pub fn solve_two_port_s_parameters(
    circuit: &[SmithChartElement],
    frequencies_hz: &[f64],
    show_ideal: bool,
    reference_impedance_ohm: f64,
) -> Result<Vec<TwoPortSParameterPoint>, SolveError> {
    if !reference_impedance_ohm.is_finite() || reference_impedance_ohm <= 0.0 {
        return Err(SolveError::NonPositiveReferenceImpedance);
    }
    if frequencies_hz
        .iter()
        .any(|frequency| !frequency.is_finite() || *frequency <= 0.0)
    {
        return Err(SolveError::NonPositiveFrequency);
    }
    if circuit
        .iter()
        .any(|element| matches!(element, SmithChartElement::SParameter(_)))
    {
        return Ok(Vec::new());
    }

    let frequency = frequency_from_hz(frequencies_hz)?;
    let reference_impedance = RfComplex::new(reference_impedance_ohm, 0.0);
    let mut networks = Vec::new();
    for element in circuit.iter().skip(1).rev() {
        let Some(network) = element_network(element, &frequency, show_ideal, reference_impedance)?
        else {
            return Ok(Vec::new());
        };
        networks.push(network);
    }
    if networks.is_empty() {
        networks.push(through_network(&frequency, reference_impedance)?);
    }
    let network = cascade_list(&networks).map_err(singular_s_parameter)?;

    Ok(frequencies_hz
        .iter()
        .enumerate()
        .map(|(point, frequency_hz)| TwoPortSParameterPoint {
            frequency_hz: *frequency_hz,
            s_parameters: SParameterMatrix::new(
                from_rf_complex(network.s[(point, 0, 0)]),
                from_rf_complex(network.s[(point, 0, 1)]),
                from_rf_complex(network.s[(point, 1, 0)]),
                from_rf_complex(network.s[(point, 1, 1)]),
            ),
        })
        .collect())
}

fn element_network(
    element: &SmithChartElement,
    frequency: &RfFrequency,
    show_ideal: bool,
    reference_impedance: RfComplex,
) -> Result<Option<Network>, SolveError> {
    let angular = frequency.angular();
    let network = match element {
        SmithChartElement::BlackBox { .. } | SmithChartElement::LoadTermination { .. } => {
            return Ok(Some(through_network(frequency, reference_impedance)?));
        }
        SmithChartElement::SeriesCapacitor {
            capacitance_f,
            esr_ohm,
            esl_h,
            ..
        } => series_impedance(
            frequency,
            Array1::from_shape_fn(frequency.points(), |point| {
                RfComplex::new(
                    if show_ideal { 0.0 } else { *esr_ohm },
                    angular[point] * if show_ideal { 0.0 } else { *esl_h }
                        - 1.0 / (angular[point] * capacitance_f),
                )
            }),
            "Series capacitor",
            reference_impedance,
        )?,
        SmithChartElement::ShuntCapacitor {
            capacitance_f,
            esr_ohm,
            esl_h,
            ..
        } => {
            let impedance = Array1::from_shape_fn(frequency.points(), |point| {
                Complex::new(
                    if show_ideal { 0.0 } else { *esr_ohm },
                    angular[point] * if show_ideal { 0.0 } else { *esl_h }
                        - 1.0 / (angular[point] * capacitance_f),
                )
            });
            let Some(admittance) = reciprocal_array(&impedance) else {
                return Ok(None);
            };
            shunt_admittance(
                frequency,
                admittance,
                "Shunt capacitor",
                reference_impedance,
            )?
        }
        SmithChartElement::SeriesInductor {
            inductance_h,
            esr_ohm,
            ..
        } => series_impedance(
            frequency,
            Array1::from_shape_fn(frequency.points(), |point| {
                RfComplex::new(
                    if show_ideal { 0.0 } else { *esr_ohm },
                    angular[point] * inductance_h,
                )
            }),
            "Series inductor",
            reference_impedance,
        )?,
        SmithChartElement::ShuntInductor {
            inductance_h,
            esr_ohm,
            ..
        } => {
            let impedance = Array1::from_shape_fn(frequency.points(), |point| {
                Complex::new(
                    if show_ideal { 0.0 } else { *esr_ohm },
                    angular[point] * inductance_h,
                )
            });
            let Some(admittance) = reciprocal_array(&impedance) else {
                return Ok(None);
            };
            shunt_admittance(frequency, admittance, "Shunt inductor", reference_impedance)?
        }
        SmithChartElement::SeriesResistor {
            resistance_ohm,
            esl_h,
            ..
        } => series_impedance(
            frequency,
            Array1::from_shape_fn(frequency.points(), |point| {
                RfComplex::new(
                    *resistance_ohm,
                    angular[point] * if show_ideal { 0.0 } else { *esl_h },
                )
            }),
            "Series resistor",
            reference_impedance,
        )?,
        SmithChartElement::ShuntResistor {
            resistance_ohm,
            esl_h,
            ..
        } => {
            let impedance = Array1::from_shape_fn(frequency.points(), |point| {
                Complex::new(
                    *resistance_ohm,
                    angular[point] * if show_ideal { 0.0 } else { *esl_h },
                )
            });
            let Some(admittance) = reciprocal_array(&impedance) else {
                return Ok(None);
            };
            shunt_admittance(frequency, admittance, "Shunt resistor", reference_impedance)?
        }
        SmithChartElement::SeriesParallelRlc {
            resistance_ohm,
            inductance_h,
            capacitance_f,
        } => series_impedance(
            frequency,
            Array1::from_shape_fn(frequency.points(), |point| {
                let omega = angular[point];
                let reactance =
                    (omega * inductance_h) / (1.0 - omega * omega * inductance_h * capacitance_f);
                let admittance = Complex::new(1.0 / resistance_ohm, -1.0 / reactance);
                to_rf_complex(admittance.reciprocal().unwrap_or(Complex::ZERO))
            }),
            "Parallel RLC",
            reference_impedance,
        )?,
        SmithChartElement::Custom {
            points,
            interpolation,
        } => series_impedance(
            frequency,
            Array1::from_shape_fn(frequency.points(), |point| {
                to_rf_complex(custom_impedance(
                    points,
                    frequency.values_hz()[point],
                    *interpolation,
                ))
            }),
            "Custom impedance",
            reference_impedance,
        )?,
        SmithChartElement::TransmissionLine {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            ..
        } => lossless_media(
            frequency,
            *characteristic_impedance_ohm,
            *effective_dielectric,
            Some(reference_impedance),
        )?
        .line(*length_m, LengthUnit::Meter)
        .map_err(singular_s_parameter)?,
        SmithChartElement::OpenStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            ..
        } => {
            let electrical_length = electrical_length(*length_m, frequency, *effective_dielectric);
            if electrical_length
                .iter()
                .any(|value| value.tan().abs() <= f64::EPSILON)
            {
                return Err(SolveError::SingularNetwork {
                    kind: ElementKind::OpenStub,
                });
            }
            let impedance = electrical_length.mapv(|value| {
                lossless_line_input_impedance(
                    Complex::new(f64::INFINITY, 0.0),
                    *characteristic_impedance_ohm,
                    value,
                )
            });
            if reciprocal_array(&impedance).is_none() {
                return Ok(None);
            }
            shunt_stub(
                frequency,
                *length_m,
                *characteristic_impedance_ohm,
                *effective_dielectric,
                reference_impedance,
                true,
            )?
        }
        SmithChartElement::ShortedStub {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            ..
        } => {
            let impedance =
                electrical_length(*length_m, frequency, *effective_dielectric).mapv(|value| {
                    lossless_line_input_impedance(
                        Complex::ZERO,
                        *characteristic_impedance_ohm,
                        value,
                    )
                });
            if reciprocal_array(&impedance).is_none() {
                return Ok(None);
            }
            shunt_stub(
                frequency,
                *length_m,
                *characteristic_impedance_ohm,
                *effective_dielectric,
                reference_impedance,
                false,
            )?
        }
        SmithChartElement::Transformer {
            model: TransformerModel::Ideal,
            coupling_or_turns_ratio,
            ..
        } => {
            let ratio = if *coupling_or_turns_ratio <= 0.0 {
                1.0
            } else {
                *coupling_or_turns_ratio
            };
            ideal_transformer_network(frequency, ratio, reference_impedance)?
        }
        SmithChartElement::Transformer {
            model: TransformerModel::CoupledInductor,
            ..
        }
        | SmithChartElement::SParameter(_) => return Ok(None),
    };
    Ok(Some(network))
}

fn series_impedance(
    frequency: &RfFrequency,
    impedance: Array1<RfComplex>,
    name: &str,
    reference_impedance: RfComplex,
) -> Result<Network, SolveError> {
    Circuit::series_impedance(frequency.clone(), &impedance, name, reference_impedance)
        .map_err(singular_s_parameter)
}

fn shunt_admittance(
    frequency: &RfFrequency,
    admittance: Array1<RfComplex>,
    name: &str,
    reference_impedance: RfComplex,
) -> Result<Network, SolveError> {
    Circuit::shunt_admittance(frequency.clone(), &admittance, name, reference_impedance)
        .map_err(singular_s_parameter)
}

fn through_network(
    frequency: &RfFrequency,
    reference_impedance: RfComplex,
) -> Result<Network, SolveError> {
    series_impedance(
        frequency,
        Array1::from_elem(frequency.points(), RfComplex::new(0.0, 0.0)),
        "Through",
        reference_impedance,
    )
}

fn lossless_media(
    frequency: &RfFrequency,
    characteristic_impedance_ohm: f64,
    effective_dielectric: f64,
    port_impedance: Option<RfComplex>,
) -> Result<DefinedGammaZ0, SolveError> {
    if !characteristic_impedance_ohm.is_finite()
        || characteristic_impedance_ohm <= 0.0
        || !effective_dielectric.is_finite()
        || effective_dielectric <= 0.0
    {
        return Err(SolveError::SingularNetwork {
            kind: ElementKind::TransmissionLine,
        });
    }
    let gamma = frequency.values_hz().mapv(|frequency_hz| {
        RfComplex::new(
            0.0,
            std::f64::consts::TAU * frequency_hz * effective_dielectric.sqrt()
                / SPEED_OF_LIGHT_M_PER_S,
        )
    });
    let characteristic_impedance = Array1::from_elem(
        frequency.points(),
        RfComplex::new(characteristic_impedance_ohm, 0.0),
    );
    let port_impedance = port_impedance.map(|value| Array1::from_elem(frequency.points(), value));
    DefinedGammaZ0::new(
        frequency.clone(),
        gamma,
        characteristic_impedance,
        port_impedance,
    )
    .map_err(singular_s_parameter)
}

fn shunt_stub(
    frequency: &RfFrequency,
    length_m: f64,
    characteristic_impedance_ohm: f64,
    effective_dielectric: f64,
    reference_impedance: RfComplex,
    open: bool,
) -> Result<Network, SolveError> {
    let media = lossless_media(
        frequency,
        characteristic_impedance_ohm,
        effective_dielectric,
        None,
    )?;
    let mut network = if open {
        media.shunt_delay_open(length_m, LengthUnit::Meter)
    } else {
        media.shunt_delay_short(length_m, LengthUnit::Meter)
    }
    .map_err(singular_s_parameter)?;
    network
        .renormalize(
            Array2::from_elem((frequency.points(), 2), reference_impedance),
            SParameterDefinition::Power,
        )
        .map_err(singular_s_parameter)?;
    Ok(network)
}

fn ideal_transformer_network(
    frequency: &RfFrequency,
    ratio: f64,
    reference_impedance: RfComplex,
) -> Result<Network, SolveError> {
    if !ratio.is_finite() || ratio <= 0.0 {
        return Err(SolveError::SingularNetwork {
            kind: ElementKind::IdealTransformer,
        });
    }
    let abcd = Array3::from_shape_fn((frequency.points(), 2, 2), |(_, row, column)| {
        match (row, column) {
            (0, 0) => RfComplex::new(ratio, 0.0),
            (1, 1) => RfComplex::new(1.0 / ratio, 0.0),
            _ => RfComplex::new(0.0, 0.0),
        }
    });
    let reference = Array2::from_elem((frequency.points(), 2), reference_impedance);
    let scattering = abcd_to_s(&abcd, &reference).map_err(singular_s_parameter)?;
    Network::new(frequency.clone(), scattering, reference).map_err(singular_s_parameter)
}

fn electrical_length(
    length_m: f64,
    frequency: &RfFrequency,
    effective_dielectric: f64,
) -> Array1<f64> {
    frequency.values_hz().mapv(|frequency_hz| {
        std::f64::consts::TAU * frequency_hz * length_m * effective_dielectric.sqrt()
            / SPEED_OF_LIGHT_M_PER_S
    })
}

fn reciprocal_array(values: &Array1<Complex>) -> Option<Array1<RfComplex>> {
    values
        .iter()
        .map(|value| value.reciprocal().map(to_rf_complex))
        .collect()
}

fn singular_s_parameter(_error: rust_rf::Error) -> SolveError {
    SolveError::SingularNetwork {
        kind: ElementKind::SParameter,
    }
}
