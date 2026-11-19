//! Via protection after IPC-4761 (#96): how a via's hole is filled or
//! plugged, whether copper caps it, and which sides the mask tents. The
//! settings combine; [`ViaProtection::class`] names the IPC-4761 type they
//! make together. A via stores only its tenting today
//! (`ViaAttributes::{top, bottom}.tented`); fill and cap are not stored, so
//! [`ViaProtection::of_via`] reads them as none.

use crate::pcb::Via;

/// What fills a via's hole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ViaFill {
    /// An open hole.
    #[default]
    None,
    /// Plugged from one side or both (part of the hole's depth).
    Plugged,
    /// Filled through with non-conductive resin.
    Resin,
    /// Filled through with conductive paste.
    Conductive,
}

impl ViaFill {
    /// Whether the hole is filled through, which a copper cap needs.
    pub fn is_filled(self) -> bool {
        matches!(self, Self::Resin | Self::Conductive)
    }
}

/// The side a plug goes in from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PlugSide {
    #[default]
    Top,
    Bottom,
    Both,
}

/// The protection of one via: fill, plug side, cap and tenting per side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ViaProtection {
    pub fill: ViaFill,
    /// Where a plug goes in; read only while `fill` is
    /// [`ViaFill::Plugged`].
    pub plug_from: PlugSide,
    /// Copper plated over the filled hole.
    pub capped: bool,
    pub tented_top: bool,
    pub tented_bottom: bool,
}

/// The IPC-4761 via protection types, plus open and custom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ipc4761 {
    /// No protection.
    Open,
    /// Tented one side.
    IA,
    /// Tented both sides.
    IB,
    /// Tented and covered: a second cover coat the board does not store.
    II,
    /// Plugged one side.
    IIIA,
    /// Plugged both sides.
    IIIB,
    /// Plugged and covered one side.
    IVA,
    /// Plugged and covered both sides.
    IVB,
    /// Filled.
    V,
    /// Filled and covered one side.
    VIA,
    /// Filled and covered both sides.
    VIB,
    /// Filled and capped.
    VII,
    /// Settings that make no type: a cap without a filled hole, or a plug
    /// on one side and the cover on the other.
    Custom,
}

impl Ipc4761 {
    /// Every type, in the order a type list shows them.
    pub const ALL: [Self; 13] = [
        Self::Open,
        Self::IA,
        Self::IB,
        Self::II,
        Self::IIIA,
        Self::IIIB,
        Self::IVA,
        Self::IVB,
        Self::V,
        Self::VIA,
        Self::VIB,
        Self::VII,
        Self::Custom,
    ];

    /// The standard's designation, "I-b"; `None` for open and custom,
    /// which the standard does not number.
    pub fn code(self) -> Option<&'static str> {
        Some(match self {
            Self::IA => "I-a",
            Self::IB => "I-b",
            Self::II => "II",
            Self::IIIA => "III-a",
            Self::IIIB => "III-b",
            Self::IVA => "IV-a",
            Self::IVB => "IV-b",
            Self::V => "V",
            Self::VIA => "VI-a",
            Self::VIB => "VI-b",
            Self::VII => "VII",
            Self::Open | Self::Custom => return None,
        })
    }

    /// The settings picking this type sets. `None` for [`Self::II`] (its
    /// second cover coat is not stored) and [`Self::Custom`] (no one set of
    /// settings). A one-sided type covers the top.
    pub fn preset(self) -> Option<ViaProtection> {
        let open = ViaProtection::default();
        let plugged = |plug_from| ViaProtection {
            fill: ViaFill::Plugged,
            plug_from,
            ..open
        };
        let filled = ViaProtection {
            fill: ViaFill::Resin,
            ..open
        };
        let top = |p: ViaProtection| ViaProtection {
            tented_top: true,
            ..p
        };
        let both = |p: ViaProtection| ViaProtection {
            tented_top: true,
            tented_bottom: true,
            ..p
        };
        Some(match self {
            Self::Open => open,
            Self::IA => top(open),
            Self::IB => both(open),
            Self::IIIA => plugged(PlugSide::Top),
            Self::IIIB => plugged(PlugSide::Both),
            Self::IVA => top(plugged(PlugSide::Top)),
            Self::IVB => both(plugged(PlugSide::Both)),
            Self::V => filled,
            Self::VIA => top(filled),
            Self::VIB => both(filled),
            Self::VII => ViaProtection {
                fill: ViaFill::Conductive,
                capped: true,
                ..open
            },
            Self::II | Self::Custom => return None,
        })
    }
}

impl ViaProtection {
    /// The protection `via` stores: its tenting per side. Fill and cap are
    /// not stored yet, so they read as none.
    pub fn of_via(via: &Via) -> Self {
        Self {
            tented_top: via.attributes.top.tented,
            tented_bottom: via.attributes.bottom.tented,
            ..Self::default()
        }
    }

    /// How many sides the mask tents: 0, 1 or 2.
    pub fn tented_sides(&self) -> u8 {
        u8::from(self.tented_top) + u8::from(self.tented_bottom)
    }

    /// The IPC-4761 type these settings make.
    pub fn class(&self) -> Ipc4761 {
        let tents = self.tented_sides();
        if self.capped && !self.fill.is_filled() {
            return Ipc4761::Custom;
        }
        match self.fill {
            ViaFill::None => match tents {
                0 => Ipc4761::Open,
                1 => Ipc4761::IA,
                _ => Ipc4761::IB,
            },
            ViaFill::Plugged => {
                if tents == 0 {
                    return if self.plug_from == PlugSide::Both {
                        Ipc4761::IIIB
                    } else {
                        Ipc4761::IIIA
                    };
                }
                let plug_covered = match self.plug_from {
                    PlugSide::Both => true,
                    PlugSide::Top => self.tented_top,
                    PlugSide::Bottom => self.tented_bottom,
                };
                match (plug_covered, tents) {
                    (false, _) => Ipc4761::Custom,
                    (true, 1) => Ipc4761::IVA,
                    (true, _) => Ipc4761::IVB,
                }
            }
            ViaFill::Resin | ViaFill::Conductive if self.capped => Ipc4761::VII,
            ViaFill::Resin | ViaFill::Conductive => match tents {
                0 => Ipc4761::V,
                1 => Ipc4761::VIA,
                _ => Ipc4761::VIB,
            },
        }
    }
}

#[cfg(test)]
#[path = "pcb_via_protection_tests.rs"]
mod tests;
