//! Every placed part of the stitched project with all of its pins, wired or
//! not: the facts a per-part view lists beside the nets of [`NetIndex`].
//! Parts and pins are named the way the stitch names terminals, so a pin
//! here finds its net with [`NetIndex::net_of_pin`].
//!
//! [`NetIndex`]: super::NetIndex
//! [`NetIndex::net_of_pin`]: super::NetIndex::net_of_pin

use signex_types::schematic::{InstancePath, instance_pins};
use uuid::Uuid;

use super::{Occ, SheetKey};
use crate::build::terminal_pin_id;

/// One pin of a placed part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartPin {
    /// What a terminal on this pin carries as its pin: the number, else the
    /// name.
    pub id: String,
    /// The pin's name; empty when the library gives none.
    pub name: String,
    /// The placed unit symbol the pin belongs to.
    pub symbol: Uuid,
}

/// One physical part on one sheet occurrence, with every pin of its units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartPins {
    /// Trimmed designator of the occurrence.
    pub designator: String,
    pub value: String,
    pub footprint: String,
    pub sheet: SheetKey,
    pub occurrence: InstancePath,
    /// Unit by unit, in library order.
    pub pins: Vec<PartPin>,
}

/// The non-power parts of every occurrence, in walk order then placement
/// order, units grouped by the schematic's own rule (`design_parts`).
pub(super) fn parts_of(occs: &[Occ]) -> Vec<PartPins> {
    let mut parts = Vec::new();
    for occ in occs {
        let placed = [(occ.path.clone(), occ.sheet)];
        for part in crate::design_parts(&placed) {
            let units: Vec<_> = part
                .units
                .iter()
                .filter_map(|id| occ.sheet.symbols.iter().find(|s| s.uuid == *id))
                .collect();
            let Some(first) = units.first() else {
                continue;
            };
            let mut pins = Vec::new();
            for sym in &units {
                let Some(lib) = occ.sheet.resolve_lib_symbol(sym) else {
                    continue;
                };
                pins.extend(instance_pins(&lib, sym).map(|lp| PartPin {
                    id: terminal_pin_id(lp),
                    name: lp.pin.name.clone(),
                    symbol: sym.uuid,
                }));
            }
            parts.push(PartPins {
                designator: part.designator,
                value: first.value.clone(),
                footprint: first.footprint.clone(),
                sheet: occ.key.clone(),
                occurrence: occ.path.clone(),
                pins,
            });
        }
    }
    parts
}
