use crate::transmission_line_calculator::{
    Complex, DEFAULT_REFERENCE_IMPEDANCE_OHM, NoisePoint, SParameterKind, SParameterPoint,
    ScalarUnit, SolveError, reflection_to_impedance,
    rust_rf_adapter::{
        RfComplex, RfNetwork, frequency_from_hz, from_rf_complex, map_rf_error, to_rf_complex,
    },
};
use ndarray::{Array1, Array2, Array3};
use serde::{Deserialize, Serialize};

/// Stores an RF network with the source metadata needed by the widget editor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SParameterBlock {
    network: Box<RfNetwork>,
    pub source_frequency_unit: ScalarUnit,
    pub raw: String,
}

impl SParameterBlock {
    /// Creates a block from widget S-parameter and noise samples.
    pub fn from_samples(
        kind: SParameterKind,
        port_reference_impedances_ohm: Vec<f64>,
        source_frequency_unit: ScalarUnit,
        mut points: Vec<SParameterPoint>,
        mut noise: Vec<NoisePoint>,
        raw: String,
    ) -> Result<Self, SolveError> {
        points.sort_by(|left, right| left.frequency_hz.total_cmp(&right.frequency_hz));
        noise.sort_by(|left, right| left.frequency_hz.total_cmp(&right.frequency_hz));
        let ports = port_count(kind);
        if port_reference_impedances_ohm.len() != ports {
            return Err(SolveError::RfCalculationFailed {
                reason: format!(
                    "expected {ports} port reference impedances, got {}",
                    port_reference_impedances_ohm.len()
                ),
            });
        }
        let frequency = frequency_from_hz(
            &points
                .iter()
                .map(|point| point.frequency_hz)
                .collect::<Vec<_>>(),
        )?;
        let scattering =
            Array3::from_shape_fn((points.len(), ports, ports), |(index, row, column)| {
                let point = &points[index];
                let value = match (row, column) {
                    (0, 0) => Some(point.s11),
                    (1, 0) => point.s21,
                    (0, 1) => point.s12,
                    (1, 1) => point.s22,
                    _ => None,
                };
                value.map_or_else(|| RfComplex::new(f64::NAN, f64::NAN), to_rf_complex)
            });
        if scattering
            .iter()
            .any(|value| !value.re.is_finite() || !value.im.is_finite())
        {
            return Err(SolveError::RfCalculationFailed {
                reason: "S-parameter samples are incomplete or non-finite".to_owned(),
            });
        }
        let reference = Array2::from_shape_fn((points.len(), ports), |(_, port)| {
            RfComplex::new(port_reference_impedances_ohm[port], 0.0)
        });
        let mut network = RfNetwork::new(frequency, scattering, reference).map_err(map_rf_error)?;
        if !noise.is_empty() {
            let noise_frequency = frequency_from_hz(
                &noise
                    .iter()
                    .map(|point| point.frequency_hz)
                    .collect::<Vec<_>>(),
            )?;
            network
                .set_noise_parameters(
                    noise_frequency,
                    Array1::from_iter(noise.iter().map(|point| point.fmin_db)),
                    Array1::from_iter(noise.iter().map(|point| to_rf_complex(point.optimum_gamma))),
                    Array1::from_iter(noise.iter().map(|point| point.rn_ohm)),
                )
                .map_err(map_rf_error)?;
        }
        Ok(Self {
            network: Box::new(network),
            source_frequency_unit,
            raw,
        })
    }

    /// Identifies whether this is one-port or two-port data.
    pub fn kind(&self) -> SParameterKind {
        if self.network.ports() == 1 {
            SParameterKind::S1P
        } else {
            SParameterKind::S2P
        }
    }

    /// Returns the port-one reference impedance.
    pub fn reference_impedance_ohm(&self) -> f64 {
        self.network
            .z0
            .get((0, 0))
            .map_or(DEFAULT_REFERENCE_IMPEDANCE_OHM, |value| value.re)
    }

    /// Returns the per-port reference impedances at the first frequency.
    pub fn port_reference_impedances_ohm(&self) -> Vec<f64> {
        (0..self.network.ports())
            .map(|port| {
                self.network
                    .z0
                    .get((0, port))
                    .map_or(DEFAULT_REFERENCE_IMPEDANCE_OHM, |value| value.re)
            })
            .collect()
    }

    /// Returns widget samples projected from the canonical RF network.
    pub fn points(&self) -> Vec<SParameterPoint> {
        (0..self.network.frequency_points())
            .map(|index| point_from_network(&self.network, index))
            .collect()
    }

    /// Returns widget noise samples projected from the canonical RF network.
    pub fn noise(&self) -> Vec<NoisePoint> {
        let Some(noise) = &self.network.noise else {
            return Vec::new();
        };
        let reference_impedance_ohm = self.reference_impedance_ohm();
        (0..noise.frequency.points())
            .map(|index| {
                let optimum_gamma = from_rf_complex(noise.optimal_reflection[index]);
                let optimum_admittance =
                    reflection_to_impedance(optimum_gamma, reference_impedance_ohm)
                        .reciprocal()
                        .unwrap_or(Complex::ZERO);
                NoisePoint {
                    frequency_hz: noise.frequency.values_hz()[index],
                    fmin_db: noise.minimum_noise_figure_db[index],
                    optimum_gamma,
                    rn_ohm: noise.equivalent_noise_resistance[index],
                    optimum_admittance,
                }
            })
            .collect()
    }

    /// Returns a Cartesian-linear sample at the requested frequency.
    pub fn interpolate(&self, frequency_hz: f64) -> Option<SParameterPoint> {
        if !frequency_hz.is_finite() || self.network.frequency_points() == 0 {
            return None;
        }
        if self.network.frequency_points() == 1 {
            return Some(point_from_network(&self.network, 0));
        }
        let start = self.network.frequency.start()?;
        let stop = self.network.frequency.stop()?;
        let clamped = frequency_hz.clamp(start, stop);
        let target = frequency_from_hz(&[clamped]).ok()?;
        let interpolated = self.network.interpolate(&target).ok()?;
        Some(point_from_network(&interpolated, 0))
    }
}

fn point_from_network(network: &RfNetwork, index: usize) -> SParameterPoint {
    let s11 = from_rf_complex(network.s[(index, 0, 0)]);
    let reference_impedance_ohm = network.z0[(index, 0)].re;
    SParameterPoint {
        frequency_hz: network.frequency.values_hz()[index],
        s11,
        s21: (network.ports() == 2).then(|| from_rf_complex(network.s[(index, 1, 0)])),
        s12: (network.ports() == 2).then(|| from_rf_complex(network.s[(index, 0, 1)])),
        s22: (network.ports() == 2).then(|| from_rf_complex(network.s[(index, 1, 1)])),
        z_s11: reflection_to_impedance(s11, reference_impedance_ohm),
    }
}

const fn port_count(kind: SParameterKind) -> usize {
    match kind {
        SParameterKind::S1P => 1,
        SParameterKind::S2P => 2,
    }
}

/// Creates the built-in one-port S-parameter sample.
pub fn default_s1p_block() -> SParameterBlock {
    let reference_impedance_ohm = DEFAULT_REFERENCE_IMPEDANCE_OHM;
    let s11 = Complex::from_polar(0.99, 6.2);
    SParameterBlock::from_samples(
        SParameterKind::S1P,
        vec![reference_impedance_ohm],
        ScalarUnit::MegaHertz,
        vec![SParameterPoint {
            frequency_hz: 1.5e6,
            s11,
            s21: None,
            s12: None,
            s22: None,
            z_s11: reflection_to_impedance(s11, reference_impedance_ohm),
        }],
        Vec::new(),
        String::new(),
    )
    .expect("the built-in one-port network is valid")
}

/// Creates the built-in two-port S-parameter sample.
pub fn default_s2p_block() -> SParameterBlock {
    let reference_impedance_ohm = DEFAULT_REFERENCE_IMPEDANCE_OHM;
    let samples = [
        (8.0e8, 0.44, -157.6, 4.725, 84.3, 0.0, 0.0, 0.339, -51.8),
        (14.0e8, 0.533, 176.6, 2.8, 64.5, 0.0, 0.0, 0.604, -58.3),
        (2.0e9, 0.439, 159.6, 2.057, 49.2, 0.0, 0.0, 0.294, -68.1),
    ];
    let points = samples
        .into_iter()
        .map(
            |(
                frequency_hz,
                s11_mag,
                s11_angle,
                s21_mag,
                s21_angle,
                s12_mag,
                s12_angle,
                s22_mag,
                s22_angle,
            )| {
                let s11 = Complex::from_polar(s11_mag, s11_angle);
                SParameterPoint {
                    frequency_hz,
                    s11,
                    s21: Some(Complex::from_polar(s21_mag, s21_angle)),
                    s12: Some(Complex::from_polar(s12_mag, s12_angle)),
                    s22: Some(Complex::from_polar(s22_mag, s22_angle)),
                    z_s11: reflection_to_impedance(s11, reference_impedance_ohm),
                }
            },
        )
        .collect();
    SParameterBlock::from_samples(
        SParameterKind::S2P,
        vec![reference_impedance_ohm; 2],
        ScalarUnit::MegaHertz,
        points,
        Vec::new(),
        String::new(),
    )
    .expect("the built-in two-port network is valid")
}
