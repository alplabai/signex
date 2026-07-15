use crate::{
    Complex, DEFAULT_REFERENCE_IMPEDANCE_OHM, NoisePoint, SParameterKind, SParameterPoint,
    ScalarUnit, reflection_to_impedance,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SParameterBlock {
    pub kind: SParameterKind,
    pub reference_impedance_ohm: f64,
    pub source_frequency_unit: ScalarUnit,
    pub points: Vec<SParameterPoint>,
    pub noise: Vec<NoisePoint>,
    pub raw: String,
}

pub fn default_s1p_block() -> SParameterBlock {
    let reference_impedance_ohm = DEFAULT_REFERENCE_IMPEDANCE_OHM;
    let s11 = Complex::from_polar(0.99, 6.2);
    SParameterBlock {
        kind: SParameterKind::S1P,
        reference_impedance_ohm,
        source_frequency_unit: ScalarUnit::MegaHertz,
        points: vec![SParameterPoint {
            frequency_hz: 1.5e6,
            s11,
            s21: None,
            s12: None,
            s22: None,
            z_s11: reflection_to_impedance(s11, reference_impedance_ohm),
        }],
        noise: Vec::new(),
        raw: String::new(),
    }
}

pub fn default_s2p_block() -> SParameterBlock {
    let reference_impedance_ohm = DEFAULT_REFERENCE_IMPEDANCE_OHM;
    let samples = [
        (8.0e8, 0.44, -157.6, 4.725, 84.3, 0.0, 0.0, 0.339, -51.8),
        (14.0e8, 0.533, 176.6, 2.8, 64.5, 0.0, 0.0, 0.604, -58.3),
        (2.0e9, 0.439, 159.6, 2.057, 49.2, 0.0, 0.0, 0.294, -68.1),
    ];
    SParameterBlock {
        kind: SParameterKind::S2P,
        reference_impedance_ohm,
        source_frequency_unit: ScalarUnit::MegaHertz,
        points: samples
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
            .collect(),
        noise: Vec::new(),
        raw: String::new(),
    }
}
