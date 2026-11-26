//! Which placed symbols draw as not fitted (#1294): the canvas, the SVG and
//! the PDF ask here, so a part crossed out on screen is crossed out on the
//! page. The fit is the BOM's: its grouping (`bom::placed_parts`, a part's
//! units across every sheet), its part link, then the project's variant
//! settings over the sheets' fit fields over the DNP flag.

use std::collections::{HashMap, HashSet};

use signex_types::project::{PlacedPart, VariantSettings};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

use crate::ExportContext;

/// The fit of every part symbol of a set of sheet placements, decided once
/// per part across all of them, as the BOM decides it.
#[derive(Debug, Clone, Default)]
pub struct PlacedFits {
    /// Per placement, each part symbol's fit there.
    by_placement: HashMap<InstancePath, HashMap<Uuid, bool>>,
}

impl PlacedFits {
    /// Decide every part of `placements` (each sheet at one placement, its
    /// designators resolved there) in `variant` (`None` or Base's name is
    /// Base). Power ports are never parts.
    pub fn of(
        placements: &[(&SchematicSheet, InstancePath)],
        variant: Option<&str>,
        settings: &VariantSettings,
    ) -> Self {
        let mut placed: Vec<(&Symbol, &str)> = Vec::new();
        let mut paths: Vec<&InstancePath> = Vec::new();
        for (sheet, path) in placements {
            for sym in sheet
                .symbols
                .iter()
                .filter(|s| crate::bom::is_part_symbol(s))
            {
                placed.push((sym, sym.designator_in(path).trim()));
                paths.push(path);
            }
        }
        let mut by_placement: HashMap<InstancePath, HashMap<Uuid, bool>> = HashMap::new();
        for part in crate::bom::placed_parts(&placed, &paths) {
            let units: Vec<&Symbol> = part.members.iter().map(|&m| placed[m].0).collect();
            let own = PlacedPart::new(placed[part.members[0]].1, part.link.as_ref());
            let project = super::project_fit(settings, variant, own);
            let fitted = super::part_fit_over(&units, variant, project);
            for &member in &part.members {
                let at = by_placement.entry(paths[member].clone()).or_default();
                at.insert(placed[member].0.uuid, fitted);
            }
        }
        Self { by_placement }
    }

    /// [`Self::of`] over the placements `ctx`'s BOM is built from, with the
    /// project's variant settings.
    pub fn of_context(ctx: &ExportContext, variant: Option<&str>) -> Self {
        let sheets = crate::bom::bom_sheets(ctx);
        Self::of(&sheets, variant, &ctx.metadata.variant_settings)
    }

    /// The fit of the symbol `uuid` at `placement`; `None` when it is not a
    /// part symbol of these placements.
    pub fn fit_at(&self, placement: &InstancePath, uuid: Uuid) -> Option<bool> {
        self.by_placement.get(placement)?.get(&uuid).copied()
    }

    /// The symbols of `sheet` not fitted at every one of `placements`: the
    /// copies of a repeated sheet's channels draw on one sheet, and a part
    /// is drawn not fitted only when no channel fits it. A symbol these fits
    /// do not cover counts as fitted at that placement.
    pub fn not_fitted_in_every(
        &self,
        sheet: &SchematicSheet,
        placements: &[InstancePath],
    ) -> HashSet<Uuid> {
        if placements.is_empty() {
            return HashSet::new();
        }
        sheet
            .symbols
            .iter()
            .map(|sym| sym.uuid)
            .filter(|&uuid| {
                placements
                    .iter()
                    .all(|placement| self.fit_at(placement, uuid) == Some(false))
            })
            .collect()
    }

    /// The symbols of `sheet`, shown at `placement`, that are not fitted.
    /// A sheet these fits do not cover (a page the hierarchy walk never
    /// reached) is decided on its own.
    pub fn not_fitted_on(
        &self,
        sheet: &SchematicSheet,
        placement: &InstancePath,
        variant: Option<&str>,
        settings: &VariantSettings,
    ) -> HashSet<Uuid> {
        let covered = sheet
            .symbols
            .iter()
            .filter(|s| crate::bom::is_part_symbol(s))
            .all(|s| self.fit_at(placement, s.uuid).is_some());
        if covered {
            self.not_fitted_in_every(sheet, std::slice::from_ref(placement))
        } else {
            not_fitted_symbols(sheet, placement, variant, settings)
        }
    }
}

/// The uuids of the symbols on `sheet`, shown at `placement`, whose part is
/// not fitted in `variant` (`None` or Base's name is Base), the sheet taken
/// alone: a part's units are the units on this sheet. A caller that knows
/// the other sheets of the project asks [`PlacedFits`].
pub fn not_fitted_symbols(
    sheet: &SchematicSheet,
    placement: &InstancePath,
    variant: Option<&str>,
    settings: &VariantSettings,
) -> HashSet<Uuid> {
    PlacedFits::of(&[(sheet, placement.clone())], variant, settings)
        .not_fitted_in_every(sheet, std::slice::from_ref(placement))
}
