//! The [`DesignIndex`] of a project: its placed sheets and its netlist read
//! into the parts, pins and nets a notebook reference can name. The app's
//! notebook read and the command line build it the same way.

use std::collections::HashMap;

use signex_types::net::Netlist;
use signex_types::schematic::{InstancePath, SchematicSheet};
use uuid::Uuid;

use crate::design::{DesignIndex, DesignNet, DesignPart, DesignPin, PinName};

/// One placed sheet occurrence of the design.
#[derive(Debug, Clone, Copy)]
pub struct PlacedSheet<'a> {
    /// The sheet's file name (`power.snxsch`).
    pub file: &'a str,
    /// The occurrence path the sheet is placed by; the root for a page.
    pub placement: &'a InstancePath,
    pub sheet: &'a SchematicSheet,
}

/// The value of every placed part of `sheets`, under the designator its
/// placement gives it, as the notebook names parts (`R1_A`, `R1_B` for a
/// sheet placed twice), so a stored `taken-with` value is compared with the
/// right instance (#3019). Power ports and parts with no designator yet
/// (`R?`) are left out.
pub fn placed_values(sheets: &[PlacedSheet<'_>]) -> crate::attachment::DesignValues {
    let mut values = crate::attachment::DesignValues::new();
    for placed in sheets {
        for symbol in placed.sheet.symbols.iter().filter(|s| !s.is_power) {
            let designator = symbol.designator_in(placed.placement).trim();
            if designator.is_empty() || designator.ends_with('?') {
                continue;
            }
            if values.get(designator).is_none() {
                values.insert(designator, symbol.value.clone());
            }
        }
    }
    values
}

/// The index of `sheets` (in walk order) and `netlist`. A part is listed
/// under the designator its placement gives it; the units of one part share
/// the entry of its first unit, whose id it carries. Power ports and parts
/// with no designator are left out. A pin's net is the net whose terminal
/// names its part and pin.
pub fn design_index(sheets: &[PlacedSheet<'_>], netlist: &Netlist) -> DesignIndex {
    let mut nets_of_pins: HashMap<(&str, &str), &str> = HashMap::new();
    for net in netlist.nets.iter().filter(|net| !net.name.is_empty()) {
        for terminal in &net.terminals {
            nets_of_pins.insert((&terminal.reference, &terminal.pin), &net.name);
        }
    }
    let mut parts: Vec<DesignPart> = Vec::new();
    let mut sheet_of: HashMap<Uuid, &str> = HashMap::new();
    for placed in sheets {
        for symbol in placed.sheet.symbols.iter().filter(|s| !s.is_power) {
            sheet_of.entry(symbol.uuid).or_insert(placed.file);
            let designator = symbol.designator_in(placed.placement).trim();
            if designator.is_empty() || parts.iter().any(|p| p.designator == designator) {
                continue;
            }
            let body = placed.sheet.body_of(symbol);
            let mut pins: Vec<DesignPin> = Vec::new();
            for lib_pin in body.map(|b| b.pins.as_slice()).unwrap_or_default() {
                let number = &lib_pin.pin.number;
                if number.is_empty() || pins.iter().any(|p| &p.number == number) {
                    continue;
                }
                pins.push(DesignPin {
                    number: number.clone(),
                    name: lib_pin.pin.name.clone(),
                    net: nets_of_pins
                        .get(&(designator, number.as_str()))
                        .map(|net| net.to_string()),
                });
            }
            parts.push(DesignPart {
                designator: designator.to_string(),
                id: symbol.uuid,
                value: symbol.value.clone(),
                description: body.map(|b| b.description.clone()).unwrap_or_default(),
                footprint: symbol.footprint.clone(),
                sheet: placed.file.to_string(),
                fitted_in: Vec::new(),
                variant_count: 0,
                pins,
            });
        }
    }
    let nets = netlist
        .nets
        .iter()
        .filter(|net| !net.name.is_empty())
        .map(|net| DesignNet {
            name: net.name.clone(),
            pins: net
                .terminals
                .iter()
                .map(|t| PinName {
                    component: t.reference.clone(),
                    pin: t.pin.clone(),
                })
                .collect(),
            sheets: net
                .terminals
                .iter()
                .filter_map(|t| sheet_of.get(&t.symbol).map(|s| s.to_string()))
                .collect(),
        })
        .collect();
    DesignIndex::new(parts, nets)
}
