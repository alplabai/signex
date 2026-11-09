//! The physical parts of a design: placed units grouped into parts by the
//! schematic's own rule, one entry per part per sheet occurrence (#1312).

use signex_types::schematic::{InstancePath, PartKey, SchematicSheet, group_into_parts};
use uuid::Uuid;

/// A part as the board sees it: the sheet occurrence and the smallest unit
/// UUID, the same anchor a footprint's `SymbolLink` names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PartAnchor {
    pub sheet: InstancePath,
    pub part: Uuid,
}

/// One physical part on one sheet occurrence.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignPart {
    pub sheet: InstancePath,
    /// Trimmed designator of the occurrence.
    pub designator: String,
    /// Unit symbol UUIDs in placement order.
    pub units: Vec<Uuid>,
    /// The designator is empty or still holds `?`: the part is not annotated
    /// and nothing may be matched or placed by it.
    pub needs_annotation: bool,
}

impl DesignPart {
    /// `None` only for a part with no units, which grouping never makes.
    pub fn anchor(&self) -> Option<PartAnchor> {
        Some(PartAnchor {
            sheet: self.sheet.clone(),
            part: self.units.iter().min().copied()?,
        })
    }
}

/// Every non-power part of `sheets` (placed sheets with their occurrence
/// paths), in sheet order then placement order. Units are grouped by
/// `group_into_parts`: a designator plus library symbol, one instance per
/// unit. Unannotated parts are kept and flagged.
pub fn design_parts(sheets: &[(InstancePath, &SchematicSheet)]) -> Vec<DesignPart> {
    let mut parts = Vec::new();
    for (path, sheet) in sheets {
        let placed: Vec<_> = sheet.symbols.iter().filter(|s| !s.is_power).collect();
        let keys: Vec<PartKey<'_>> = placed
            .iter()
            .map(|s| PartKey {
                reference: s.designator_in(path).trim(),
                ..s.part_key()
            })
            .collect();
        for group in group_into_parts(keys.iter().copied()) {
            let designator = keys[group[0]].reference;
            parts.push(DesignPart {
                sheet: path.clone(),
                designator: designator.to_string(),
                units: group.iter().map(|&i| placed[i].uuid).collect(),
                needs_annotation: designator.is_empty() || designator.contains('?'),
            });
        }
    }
    parts
}

#[cfg(test)]
#[path = "parts_tests.rs"]
mod tests;
