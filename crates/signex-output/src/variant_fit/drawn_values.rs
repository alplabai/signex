//! The value a part is drawn with in a variant (#2948): the variant's own
//! value for the part (`variant_settings`, mockup row 32) in place of the
//! sheet's, on the canvas, in SVG, PDF and print, as the not-fitted drawing
//! follows the variant (#1294). The part is found as the BOM finds it (its
//! units grouped across the placements given, its part link), so the value
//! drawn is the value the BOM lists. Base draws the sheets' values.

use std::borrow::Cow;
use std::collections::HashMap;

use signex_types::project::{PlacedPart, VALUE_PARAMETER, VariantSettings, is_base_variant};
use signex_types::schematic::{
    InstancePath, PartKey, SchematicSheet, Symbol, group_into_parts, normalized_lib_id,
};
use uuid::Uuid;

use crate::ExportContext;

/// The values a variant gives part symbols in place of their own, by symbol
/// uuid. A symbol not in the map draws its own value.
pub type DrawnValueMap = HashMap<Uuid, String>;

/// The variant values of every part symbol of a set of sheet placements,
/// decided once per part across all of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DrawnValues {
    by_placement: HashMap<InstancePath, DrawnValueMap>,
}

impl DrawnValues {
    /// Decide the parts of `placements` (each sheet at one placement, its
    /// designators resolved there) in `variant`; `None` or Base's name is
    /// Base, which gives no values. Power ports are never parts.
    pub fn of(
        placements: &[(&SchematicSheet, InstancePath)],
        variant: Option<&str>,
        settings: &VariantSettings,
    ) -> Self {
        let mut out = Self::default();
        let Some(variant) = variant.filter(|name| !is_base_variant(name)) else {
            return out;
        };
        let gives_values = settings.parts.iter().any(|part| {
            part.variant.eq_ignore_ascii_case(variant.trim())
                && part.parameters.contains_key(VALUE_PARAMETER)
        });
        if !gives_values {
            return out;
        }
        let mut placed: Vec<(&Symbol, &str)> = Vec::new();
        let mut paths: Vec<&InstancePath> = Vec::new();
        for (sheet, path) in placements {
            for sym in sheet
                .symbols
                .iter()
                .filter(|sym| crate::bom::is_part_symbol(sym))
            {
                placed.push((sym, sym.designator_in(path).trim()));
                paths.push(path);
            }
        }
        let parts = group_into_parts(placed.iter().map(|(sym, designator)| PartKey {
            reference: designator,
            lib_id: normalized_lib_id(&sym.lib_id),
            unit: sym.unit,
        }));
        for part in &parts {
            let link = super::part_link(
                part.iter()
                    .map(|&member| (paths[member], placed[member].0.uuid)),
            );
            let own = PlacedPart::new(placed[part[0]].1, link.as_ref());
            let Some(value) = variant_value(settings, Some(variant), own) else {
                continue;
            };
            for &member in part {
                out.by_placement
                    .entry(paths[member].clone())
                    .or_default()
                    .insert(placed[member].0.uuid, value.to_string());
            }
        }
        out
    }

    /// [`Self::of`] over the placements `ctx`'s BOM is built from, with the
    /// project's variant settings.
    pub fn of_context(ctx: &ExportContext, variant: Option<&str>) -> Self {
        let sheets = crate::bom::bom_sheets(ctx);
        Self::of(&sheets, variant, &ctx.metadata.variant_settings)
    }

    /// The values of the symbols shown at `placement`; `None` when the
    /// variant gives none there.
    pub fn at(&self, placement: &InstancePath) -> Option<&DrawnValueMap> {
        self.by_placement.get(placement)
    }

    /// Whether the variant gives no part a value of its own.
    pub fn is_empty(&self) -> bool {
        self.by_placement.values().all(HashMap::is_empty)
    }
}

/// The value `settings` give the part `placed` names in `variant`; `None`
/// leaves the sheets' value. Base (`None`) gives none.
pub fn variant_value<'s, 'p>(
    settings: &'s VariantSettings,
    variant: Option<&str>,
    placed: impl Into<PlacedPart<'p>>,
) -> Option<&'s str> {
    settings
        .part(variant, placed)?
        .parameters
        .get(VALUE_PARAMETER)
        .map(String::as_str)
}

/// `symbol` with the value `values` gives it, for the value to be shown by
/// the one rule (`crate::shown_value`): an `=` variant value is evaluated
/// as a sheet value is. `symbol` itself when `values` gives it none.
pub fn drawn_symbol<'s>(symbol: &'s Symbol, values: Option<&DrawnValueMap>) -> Cow<'s, Symbol> {
    match values.and_then(|values| values.get(&symbol.uuid)) {
        Some(value) if *value != symbol.value => {
            let mut drawn = symbol.clone();
            drawn.value.clone_from(value);
            Cow::Owned(drawn)
        }
        _ => Cow::Borrowed(symbol),
    }
}
