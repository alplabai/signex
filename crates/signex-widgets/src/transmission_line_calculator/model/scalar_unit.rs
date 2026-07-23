use serde::{Deserialize, Serialize};

/// Identifies the engineering unit used to enter or display a scalar value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalarUnit {
    Henry,
    MilliHenry,
    MicroHenry,
    NanoHenry,
    PicoHenry,
    FemtoHenry,
    Farad,
    MilliFarad,
    MicroFarad,
    NanoFarad,
    PicoFarad,
    FemtoFarad,
    MegaOhm,
    KiloOhm,
    Ohm,
    MilliOhm,
    Meter,
    MilliMeter,
    MicroMeter,
    Wavelength,
    Degree,
    Hertz,
    KiloHertz,
    MegaHertz,
    GigaHertz,
    TeraHertz,
}

pub(crate) const FREQUENCY_UNITS: [ScalarUnit; 5] = [
    ScalarUnit::Hertz,
    ScalarUnit::KiloHertz,
    ScalarUnit::MegaHertz,
    ScalarUnit::GigaHertz,
    ScalarUnit::TeraHertz,
];

impl ScalarUnit {
    /// Returns the scalar multiplier represented by this unit.
    pub fn multiplier(self) -> f64 {
        match self {
            Self::Henry | Self::Farad | Self::Ohm | Self::Meter | Self::Hertz => 1.0,
            Self::MilliHenry | Self::MilliFarad | Self::MilliOhm | Self::MilliMeter => 1.0e-3,
            Self::MicroHenry | Self::MicroFarad | Self::MicroMeter => 1.0e-6,
            Self::NanoHenry | Self::NanoFarad => 1.0e-9,
            Self::PicoHenry | Self::PicoFarad => 1.0e-12,
            Self::FemtoHenry | Self::FemtoFarad => 1.0e-15,
            Self::MegaOhm => 1.0e6,
            Self::KiloOhm | Self::KiloHertz => 1.0e3,
            Self::MegaHertz => 1.0e6,
            Self::GigaHertz => 1.0e9,
            Self::TeraHertz => 1.0e12,
            Self::Wavelength | Self::Degree => 0.0,
        }
    }

    /// Returns the display symbol for this frequency unit.
    pub fn frequency_symbol(self) -> Option<&'static str> {
        match self {
            Self::Hertz => Some("Hz"),
            Self::KiloHertz => Some("kHz"),
            Self::MegaHertz => Some("MHz"),
            Self::GigaHertz => Some("GHz"),
            Self::TeraHertz => Some("THz"),
            _ => None,
        }
    }
}

impl std::fmt::Display for ScalarUnit {
    /// Formats the value for user-facing display.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Henry => "H",
            Self::MilliHenry => "mH",
            Self::MicroHenry => "uH",
            Self::NanoHenry => "nH",
            Self::PicoHenry => "pH",
            Self::FemtoHenry => "fH",
            Self::Farad => "F",
            Self::MilliFarad => "mF",
            Self::MicroFarad => "uF",
            Self::NanoFarad => "nF",
            Self::PicoFarad => "pF",
            Self::FemtoFarad => "fF",
            Self::MegaOhm => "MΩ",
            Self::KiloOhm => "KΩ",
            Self::Ohm => "Ω",
            Self::MilliOhm => "mΩ",
            Self::Meter => "m",
            Self::MilliMeter => "mm",
            Self::MicroMeter => "um",
            Self::Wavelength => "λ",
            Self::Degree => "deg",
            Self::Hertz => "Hz",
            Self::KiloHertz => "kHz",
            Self::MegaHertz => "MHz",
            Self::GigaHertz => "GHz",
            Self::TeraHertz => "THz",
        };
        formatter.write_str(label)
    }
}
