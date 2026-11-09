//! `query_pins`: the pins of a part on one sheet, each with its tip in mm.
//!
//! The tips come from `signex_types::schematic::pin_tips`, which symbol
//! lowering, ERC and the full netlist build read too. The designator is the
//! one the netlist names a terminal with, `Symbol::designator_in`, at every
//! instance path of the sheet in its project (the root path for a sheet no
//! project walk reaches), compared exactly. A multi-unit part is named by its
//! bare designator and lists the pins of every unit placed on the sheet; a
//! unit that is not placed is not listed.

use serde_json::{Value, json};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol, pin_tips};

use super::{Answer, text_operand};
use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::app::document::DocumentKey;
use crate::app::document_ids::DocumentKind;

/// The placed units of the one part `designator` names on `sheet` at any of
/// `paths`, by unit.
fn part_units<'a>(
    sheet: &'a SchematicSheet,
    designator: &str,
    paths: &[InstancePath],
) -> Result<Vec<&'a Symbol>, String> {
    if designator.is_empty() || designator.ends_with('?') {
        return Err(format!(
            "`{designator}` is not annotated, so it names no single part"
        ));
    }
    let mut units: Vec<&Symbol> = sheet
        .symbols
        .iter()
        .filter(|s| !s.is_power && paths.iter().any(|p| s.designator_in(p) == designator))
        .collect();
    let Some(first_lib) = units.first().map(|s| s.lib_id.as_str()) else {
        return Err(format!("no part `{designator}` on this sheet"));
    };
    let one_lib = units.iter().all(|s| s.lib_id == first_lib);
    units.sort_by_key(|s| s.unit);
    let one_part = one_lib && units.windows(2).all(|pair| pair[0].unit != pair[1].unit);
    if one_part {
        Ok(units)
    } else {
        Err(format!(
            "`{designator}` is ambiguous: two different parts, or one unit twice, share it"
        ))
    }
}

/// The `pins` array for the placed units of `designator` on `sheet`: one
/// entry per placed unit for each pin it shows, as the netlist emits a
/// terminal at every placed unit's tip. A pin shared by all units (unit 0)
/// is marked `shared`.
pub(super) fn pins_of(
    sheet: &SchematicSheet,
    designator: &str,
    paths: &[InstancePath],
) -> Result<Vec<Value>, String> {
    let mut pins = Vec::new();
    for sym in part_units(sheet, designator, paths)? {
        let lib = sheet
            .resolve_lib_symbol(sym)
            .ok_or_else(|| format!("the library symbol `{}` is not in this sheet", sym.lib_id))?;
        for (lp, tip) in pin_tips(&lib, sym) {
            // The pin id ERC and the netlist use: the number, else the name.
            let number = if lp.pin.number.is_empty() {
                &lp.pin.name
            } else {
                &lp.pin.number
            };
            pins.push(json!({
                "number": number,
                "name": lp.pin.name,
                "electrical_type": lp.pin.direction,
                "hidden": !lp.pin.visible,
                "unit": sym.unit,
                "shared": lp.unit == 0,
                "position": [tip.x, tip.y],
            }));
        }
    }
    Ok(pins)
}

impl Signex {
    /// `query_pins document=<doc> designator=<ref>`.
    pub(super) fn query_pins(&self, args: &CommandArgs) -> Answer {
        let entry = self.document_operand(args)?;
        let designator = text_operand(args, "designator").unwrap_or_default().trim();
        let sheet = match &entry.key {
            Some(DocumentKey::Schematic(path)) => self
                .document_state
                .engines
                .get(path)
                .ok_or("the sheet is not loaded")?
                .document(),
            None if entry.kind == DocumentKind::Schematic => {
                return Err("the sheet is not loaded".into());
            }
            _ => {
                return Err(
                    "`query_pins` reads a schematic sheet; this document is not one".into(),
                );
            }
        };
        let mut paths = self.instance_paths_of(&entry);
        if paths.is_empty() {
            paths.push(InstancePath::root());
        }
        let pins = pins_of(sheet, designator, &paths)?;
        Ok(json!({ "designator": designator, "count": pins.len(), "pins": pins }))
    }
}
