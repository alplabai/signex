//! `query_pins`: the pins of a part on one sheet, each with its tip in mm.
//!
//! The tips come from `signex_types::schematic::pin_tips`, the one transform
//! the canvas hit-test, the netlist and ERC share. A multi-unit part is named
//! by its bare designator and lists the pins of every unit placed on the
//! sheet; a unit that is not placed is not listed.

use serde_json::{Value, json};
use signex_types::schematic::{SchematicSheet, Symbol, pin_tips};

use super::{Answer, text_operand};
use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::app::document::DocumentKey;
use crate::app::document_ids::DocumentKind;

/// The placed units of the one part `designator` names on `sheet`, by unit.
fn part_units<'a>(sheet: &'a SchematicSheet, designator: &str) -> Result<Vec<&'a Symbol>, String> {
    if designator.is_empty() || designator.ends_with('?') {
        return Err(format!(
            "`{designator}` is not annotated, so it names no single part"
        ));
    }
    let wanted = designator.to_lowercase();
    let mut units: Vec<&Symbol> = sheet
        .symbols
        .iter()
        .filter(|s| !s.is_power && s.reference.to_lowercase() == wanted)
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

/// The `pins` array for the placed units of `designator` on `sheet`.
fn pins_of(sheet: &SchematicSheet, designator: &str) -> Result<Vec<Value>, String> {
    let mut pins = Vec::new();
    for (index, sym) in part_units(sheet, designator)?.into_iter().enumerate() {
        let lib = sheet
            .resolve_lib_symbol(sym)
            .ok_or_else(|| format!("the library symbol `{}` is not in this sheet", sym.lib_id))?;
        for (lp, tip) in pin_tips(&lib, sym) {
            // A pin shared by every unit (unit 0) is one pin: listed once.
            if lp.unit == 0 && index > 0 {
                continue;
            }
            pins.push(json!({
                "number": lp.pin.number,
                "name": lp.pin.name,
                "electrical_type": lp.pin.direction,
                "hidden": !lp.pin.visible,
                "unit": sym.unit,
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
        let pins = pins_of(sheet, designator)?;
        Ok(json!({ "designator": designator, "count": pins.len(), "pins": pins }))
    }
}
