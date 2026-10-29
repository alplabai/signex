//! Rule: UnitsNotPlaced, project-wide (#1286). Units of one part are often
//! split across sheets, so a sheet's own run cannot tell a missing unit from
//! one placed elsewhere. The app replaces the per-sheet `UnitsNotPlaced`
//! findings of a project's sheets with this pass over those sheets.

use signex_types::schematic::SchematicSheet;

use crate::Violation;
use crate::context::{no_erc_directive_at, placement_designators};
use crate::rules::{PlacedUnit, missing_units, units_finding};

/// Every units-not-placed finding across `sheets`, each filed under the sheet
/// that holds the part's first unit. `sheets` order decides which `U?`-free
/// units pair up and which sheet a finding lands on; pass a stable order. A
/// finding located on a No-ERC directive of its sheet is dropped, as the
/// per-sheet run would.
pub fn project_units_not_placed<K: Clone>(sheets: &[(K, &SchematicSheet)]) -> Vec<(K, Violation)> {
    let mut owners: Vec<(&K, &SchematicSheet)> = Vec::new();
    let mut units: Vec<PlacedUnit> = Vec::new();
    for (key, sheet) in sheets {
        for symbol in &sheet.symbols {
            let unit_count = sheet
                .resolve_lib_symbol(symbol)
                .map_or(1, |lib| lib.unit_count());
            for designator in placement_designators(symbol) {
                if !PlacedUnit::is_eligible(designator, symbol.is_power) {
                    continue;
                }
                owners.push((key, sheet));
                units.push(PlacedUnit {
                    reference: designator,
                    lib_id: &symbol.lib_id,
                    unit: symbol.unit,
                    unit_count,
                    uuid: symbol.uuid,
                    at: symbol.position,
                });
            }
        }
    }
    missing_units(&units)
        .into_iter()
        .filter(|(first, _)| !has_no_erc_directive_at(owners[*first].1, &units[*first].at))
        .map(|(first, gap)| {
            let violation = Violation::from(units_finding(&units[first], &gap));
            (owners[first].0.clone(), violation)
        })
        .collect()
}

fn has_no_erc_directive_at(sheet: &SchematicSheet, at: &signex_types::schematic::Point) -> bool {
    no_erc_directive_at(sheet.no_erc_directives.iter().map(|d| &d.position), at)
}
