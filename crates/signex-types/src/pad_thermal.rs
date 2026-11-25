//! Thermal relief (#2580, #2593): how a polygon pour joins a pad of its own
//! net. A pour with `Zone::thermal_relief` on keeps an air gap round each
//! such pad and bridges it with spokes; a pad may set its own air gap, spoke
//! width and spoke count (Manual), each else following the pour (Rule).
//! Rules: `docs/PCB_TOOLS.md`, "Thermal relief".

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::pcb::{PCB_THERMAL_BRIDGE_MM, PCB_THERMAL_GAP_MM, Zone};

/// The spokes a pour gives a pad that sets none.
pub const DEFAULT_SPOKES: u8 = 4;

/// The most spokes a pad may set.
pub const MAX_SPOKES: u8 = 4;

/// A pad's own thermal relief values; each `None` follows the pour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PadThermal {
    /// The air gap between the pad and the pour, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub air_gap: Option<f64>,
    /// The width of each spoke, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spoke_width: Option<f64>,
    /// How many spokes, 1 to [`MAX_SPOKES`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spokes: Option<u8>,
}

/// A per-pad value no pour can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadThermalFault {
    AirGap,
    SpokeWidth,
    Spokes,
}

impl PadThermalFault {
    /// The name of the value, as the file and the commands spell it.
    pub fn field(self) -> &'static str {
        match self {
            PadThermalFault::AirGap => "air_gap",
            PadThermalFault::SpokeWidth => "spoke_width",
            PadThermalFault::Spokes => "spokes",
        }
    }
}

impl fmt::Display for PadThermalFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PadThermalFault::AirGap => f.write_str("an air gap must be a length above 0 mm"),
            PadThermalFault::SpokeWidth => f.write_str("a spoke width must be a length above 0 mm"),
            PadThermalFault::Spokes => {
                write!(f, "spokes must be a whole number from 1 to {MAX_SPOKES}")
            }
        }
    }
}

/// Whether `mm` can be an air gap or a spoke width: a finite length above 0.
pub fn is_thermal_length(mm: f64) -> bool {
    mm.is_finite() && mm > 0.0
}

/// Whether `n` can be a pad's spoke count.
pub fn is_spoke_count(n: u8) -> bool {
    (1..=MAX_SPOKES).contains(&n)
}

impl PadThermal {
    /// Every value follows the pour.
    pub const RULE: PadThermal = PadThermal {
        air_gap: None,
        spoke_width: None,
        spokes: None,
    };

    /// Whether every value follows the pour; such a pad writes nothing.
    pub fn is_rule(&self) -> bool {
        *self == Self::RULE
    }

    /// The first value no pour can use; the reader, the writer and every
    /// edit refuse such a pad.
    pub fn fault(&self) -> Option<PadThermalFault> {
        if self.air_gap.is_some_and(|v| !is_thermal_length(v)) {
            Some(PadThermalFault::AirGap)
        } else if self.spoke_width.is_some_and(|v| !is_thermal_length(v)) {
            Some(PadThermalFault::SpokeWidth)
        } else if self.spokes.is_some_and(|n| !is_spoke_count(n)) {
            Some(PadThermalFault::Spokes)
        } else {
            None
        }
    }
}

/// The relief a pour gives one pad, every value resolved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relief {
    pub air_gap: f64,
    pub spoke_width: f64,
    pub spokes: u8,
}

impl Zone {
    /// The relief this pour gives a pad of its net whose own values are
    /// `pad`: the pad's value where it sets one, else the pour's; `None`
    /// when the pour joins its pads solidly (Thermal Relief off, or no
    /// pour). A stored gap or width that is no length above 0 reads as the
    /// default ([`PCB_THERMAL_GAP_MM`], [`PCB_THERMAL_BRIDGE_MM`]).
    pub fn pour_relief(&self, pad: &PadThermal) -> Option<Relief> {
        if !self.is_pour() || !self.thermal_relief {
            return None;
        }
        let resolve = |own: Option<f64>, stored: f64, default: f64| {
            own.filter(|v| is_thermal_length(*v))
                .unwrap_or(if is_thermal_length(stored) {
                    stored
                } else {
                    default
                })
        };
        Some(Relief {
            air_gap: resolve(pad.air_gap, self.thermal_gap, PCB_THERMAL_GAP_MM),
            spoke_width: resolve(pad.spoke_width, self.thermal_width, PCB_THERMAL_BRIDGE_MM),
            spokes: pad
                .spokes
                .filter(|n| is_spoke_count(*n))
                .unwrap_or(DEFAULT_SPOKES),
        })
    }
}

#[cfg(test)]
#[path = "pad_thermal_tests.rs"]
mod tests;
