//! Which placed symbols of a sheet draw as not fitted (#1294): the canvas,
//! the SVG and the PDF ask here, so a part crossed out on screen is crossed
//! out on the page. The fit is the BOM's: the project's variant settings over
//! the sheets' fit fields over the DNP flag.

use std::collections::HashSet;

use signex_types::project::{PlacedPart, VariantSettings};
use signex_types::schematic::{
    InstancePath, PartKey, SchematicSheet, Symbol, group_into_parts, normalized_lib_id,
};
use uuid::Uuid;

/// The uuids of the symbols on `sheet`, shown at `placement`, whose part is
/// not fitted in `variant` (`None` or Base's name is Base). A part's units
/// are the units on this sheet; power ports are never parts.
pub fn not_fitted_symbols(
    sheet: &SchematicSheet,
    placement: &InstancePath,
    variant: Option<&str>,
    settings: &VariantSettings,
) -> HashSet<Uuid> {
    let placed: Vec<(&Symbol, &str)> = sheet
        .symbols
        .iter()
        .filter(|sym| crate::bom::is_part_symbol(sym))
        .map(|sym| (sym, sym.designator_in(placement).trim()))
        .collect();
    let parts = group_into_parts(placed.iter().map(|(sym, designator)| PartKey {
        reference: designator,
        lib_id: normalized_lib_id(&sym.lib_id),
        unit: sym.unit,
    }));
    let mut out = HashSet::new();
    for part in parts {
        let units: Vec<&Symbol> = part.iter().map(|&member| placed[member].0).collect();
        let link = super::part_link(units.iter().map(|unit| (placement, unit.uuid)));
        let own = PlacedPart::new(placed[part[0]].1, link.as_ref());
        let project = super::project_fit(settings, variant, own);
        if !super::part_fit_over(&units, variant, project) {
            out.extend(units.iter().map(|unit| unit.uuid));
        }
    }
    out
}
