//! A pin end that lands on the middle of a wire joins it through a junction
//! dot (maintainer decision 2026-11-18). The netlist still connects only at
//! wire ends, dots, labels and no-connects (#107); the edit that makes the
//! contact (placing or moving a part, drawing or moving a wire) mints the
//! dot, in its own undo step, through `reconcile_wire_junctions`. A sheet that
//! already has such a bare contact is never changed on open: it is reported,
//! and [`pin_junction_repairs`] is the one-step repair.

use std::collections::{BTreeMap, HashSet};

use signex_net::{point_on_segment, pt_key, uf::Key};
use signex_types::object_id::IdSource;
use signex_types::schematic::{
    Junction, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol, SymbolTransform, Wire,
    instance_pins,
};

use crate::Command;

/// The pin tips of `symbol` on `sheet` (its own unit), empty when its library
/// body is missing.
pub(crate) fn symbol_pin_tips(sheet: &SchematicSheet, symbol: &Symbol) -> Vec<Point> {
    let Some(lib) = sheet.resolve_lib_symbol(symbol) else {
        return Vec::new();
    };
    let xform = SymbolTransform::from_symbol(symbol);
    instance_pins(&lib, symbol)
        .map(|lib_pin| xform.apply(lib_pin.pin.position))
        .collect()
}

/// Every pin tip of `sheet`.
fn all_pin_tips(sheet: &SchematicSheet) -> Vec<Point> {
    (sheet.symbols.iter())
        .flat_map(|symbol| symbol_pin_tips(sheet, symbol))
        .collect()
}

/// Where the dots of `sheet` are.
fn dot_keys(sheet: &SchematicSheet) -> HashSet<Key> {
    (sheet.junctions.iter())
        .map(|j| pt_key(&j.position))
        .collect()
}

/// Whether `k` lies on `wire` and is not one of its ends.
fn strictly_inside(k: Key, wire: &Wire) -> bool {
    let (a, b) = (pt_key(&wire.start), pt_key(&wire.end));
    k != a && k != b && point_on_segment(k, a, b)
}

/// The wires of a sheet by the row or column they run along, so "which wire
/// passes through this point" reads one line, not the sheet.
struct WireLines<'a> {
    rows: BTreeMap<i64, Vec<&'a Wire>>,
    columns: BTreeMap<i64, Vec<&'a Wire>>,
    slanted: Vec<&'a Wire>,
}

impl<'a> WireLines<'a> {
    fn of(wires: &'a [Wire]) -> Self {
        let mut lines = Self {
            rows: BTreeMap::new(),
            columns: BTreeMap::new(),
            slanted: Vec::new(),
        };
        for wire in wires {
            let (a, b) = (pt_key(&wire.start), pt_key(&wire.end));
            if a.1 == b.1 {
                lines.rows.entry(a.1).or_default().push(wire);
            } else if a.0 == b.0 {
                lines.columns.entry(a.0).or_default().push(wire);
            } else {
                lines.slanted.push(wire);
            }
        }
        lines
    }

    /// Whether some wire passes through `k` with neither end there.
    fn passes_through(&self, k: Key) -> bool {
        let row = self.rows.get(&k.1).into_iter().flatten();
        let column = self.columns.get(&k.0).into_iter().flatten();
        row.chain(column)
            .chain(self.slanted.iter())
            .any(|wire| strictly_inside(k, wire))
    }
}

/// Where a pin tip sits strictly inside a wire with no dot there: a contact
/// the netlist does not count, left by an older Signex or another tool.
#[must_use]
pub fn bare_pin_taps(sheet: &SchematicSheet) -> Vec<Point> {
    let dots = dot_keys(sheet);
    let lines = WireLines::of(&sheet.wires);
    let mut seen: HashSet<Key> = HashSet::new();
    all_pin_tips(sheet)
        .into_iter()
        .filter(|tip| {
            let k = pt_key(tip);
            !dots.contains(&k) && lines.passes_through(k) && seen.insert(k)
        })
        .collect()
}

/// The repair of [`bare_pin_taps`]: one dot per bare contact, as commands to
/// run as one batch (one undo step). Empty when there is nothing to repair.
/// Each dot is an automatic one, drawn and saved like any junction and
/// checked again on every later edit; its id comes from `ids` (#2429).
#[must_use]
pub fn pin_junction_repairs(sheet: &SchematicSheet, ids: &mut dyn IdSource) -> Vec<Command> {
    (bare_pin_taps(sheet).into_iter())
        .map(|at| Command::PlaceJunction {
            junction: Junction {
                uuid: ids.next_id(),
                position: at,
                minted: true,
                ..Junction::default()
            },
        })
        .collect()
}

/// The dots an edit needs because a pin end now lies strictly inside a wire:
/// the pins of the `items` symbols on any wire, and any pin on the `items`
/// wires. Only places with no dot yet; called with the edit applied.
pub(crate) fn pin_dots_for(sheet: &SchematicSheet, items: &[SelectedItem]) -> Vec<Junction> {
    let kinds = |kind: SelectedKind| -> HashSet<uuid::Uuid> {
        (items.iter())
            .filter(|item| item.kind == kind)
            .map(|item| item.uuid)
            .collect()
    };
    let (symbols, wires) = (kinds(SelectedKind::Symbol), kinds(SelectedKind::Wire));
    if symbols.is_empty() && wires.is_empty() {
        return Vec::new();
    }
    let lines = WireLines::of(&sheet.wires);
    let mut candidates: Vec<Point> = (sheet.symbols.iter())
        .filter(|symbol| symbols.contains(&symbol.uuid))
        .flat_map(|symbol| symbol_pin_tips(sheet, symbol))
        .filter(|tip| lines.passes_through(pt_key(tip)))
        .collect();
    if !wires.is_empty() {
        let touched: Vec<&Wire> = (sheet.wires.iter())
            .filter(|wire| wires.contains(&wire.uuid))
            .collect();
        candidates.extend(all_pin_tips(sheet).into_iter().filter(|tip| {
            touched
                .iter()
                .any(|wire| strictly_inside(pt_key(tip), wire))
        }));
    }
    let mut taken = dot_keys(sheet);
    candidates
        .into_iter()
        .filter(|tip| taken.insert(pt_key(tip)))
        // An automatic dot, made where the T dots of a reconcile are made.
        .map(super::autoplace::junction_at)
        .collect()
}

/// The pin tips of `sheet` that lie strictly inside a wire, for judging the
/// minted dots again: such a dot stays while its pin does.
pub(crate) fn pin_taps(sheet: &SchematicSheet) -> HashSet<Key> {
    let lines = WireLines::of(&sheet.wires);
    (all_pin_tips(sheet).into_iter())
        .map(|tip| pt_key(&tip))
        .filter(|k| lines.passes_through(*k))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rubber_band_tests::{add_wire, lib_pin, lib_symbol, place};
    use crate::test_support::test_sheet;
    use signex_types::object_id::SequentialIds;

    fn sheet_with_tap() -> SchematicSheet {
        let mut sheet = test_sheet();
        sheet
            .lib_symbols
            .insert("TP".to_string(), lib_symbol("TP", vec![lib_pin("1", 0.0)]));
        place(&mut sheet, "TP1", "TP", Point::new(20.0, 10.0));
        place(&mut sheet, "TP2", "TP", Point::new(0.0, 10.0));
        add_wire(&mut sheet, (0.0, 10.0), (40.0, 10.0));
        sheet
    }

    #[test]
    fn a_pin_mid_wire_is_a_bare_tap_and_one_on_the_end_is_not() {
        let sheet = sheet_with_tap();
        assert_eq!(bare_pin_taps(&sheet), vec![Point::new(20.0, 10.0)]);
        assert_eq!(pin_taps(&sheet).len(), 1);
    }

    #[test]
    fn a_dot_on_the_tap_ends_the_report_and_the_repair_adds_one_dot() {
        let mut sheet = sheet_with_tap();
        let mut ids = SequentialIds::default();
        let repairs = pin_junction_repairs(&sheet, &mut ids);
        assert_eq!(repairs.len(), 1);
        let Command::PlaceJunction { junction } = &repairs[0] else {
            panic!("a repair places a junction");
        };
        assert!(junction.minted);
        assert_eq!(junction.uuid, uuid::Uuid::from_u128(1), "the caller's id");
        sheet.junctions.push(junction.clone());
        assert!(bare_pin_taps(&sheet).is_empty());
        assert!(pin_junction_repairs(&sheet, &mut ids).is_empty());
    }
}
