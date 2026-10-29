//! Rule: UnitsNotPlaced, project-wide (#1286). Units of one part are often
//! split across sheets, so a sheet's own run cannot tell a missing unit from
//! one placed elsewhere. The app replaces every per-sheet `UnitsNotPlaced`
//! finding with this pass over all the sheets it ran.

use signex_types::schematic::SchematicSheet;

use crate::Violation;
use crate::rules::{PlacedUnit, missing_units, units_finding};

/// Every units-not-placed finding across `sheets`, each filed under the sheet
/// that holds the part's first unit. `sheets` order decides which `U?`-free
/// units pair up and which sheet a finding lands on; pass a stable order.
pub fn project_units_not_placed<K: Clone>(sheets: &[(K, &SchematicSheet)]) -> Vec<(K, Violation)> {
    let mut owners: Vec<&K> = Vec::new();
    let mut units: Vec<PlacedUnit> = Vec::new();
    for (key, sheet) in sheets {
        for symbol in &sheet.symbols {
            if !PlacedUnit::is_eligible(&symbol.reference, symbol.is_power) {
                continue;
            }
            let unit_count = sheet
                .resolve_lib_symbol(symbol)
                .map_or(1, |lib| lib.unit_count());
            owners.push(key);
            units.push(PlacedUnit {
                reference: &symbol.reference,
                lib_id: &symbol.lib_id,
                unit: symbol.unit,
                unit_count,
                uuid: symbol.uuid,
                at: symbol.position,
            });
        }
    }
    missing_units(&units)
        .into_iter()
        .map(|(first, gap)| {
            let violation = Violation::from(units_finding(&units[first], &gap));
            (owners[first].clone(), violation)
        })
        .collect()
}
