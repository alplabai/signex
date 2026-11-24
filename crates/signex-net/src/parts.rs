//! The physical parts of a design: placed units grouped into parts by the
//! schematic's own rule, one entry per part per sheet occurrence (#1312).

use signex_types::schematic::{InstancePath, PartKey, SchematicSheet, group_into_parts};
use uuid::Uuid;

use crate::project::ChannelDesignators;

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

/// Whether a trimmed `designator` is not annotated yet: empty, or still
/// holding `?`. Such a designator names no one part.
pub(crate) fn needs_annotation(designator: &str) -> bool {
    designator.is_empty() || designator.contains('?')
}

/// Every non-power part of `sheets` (placed sheets with their occurrence
/// paths), in sheet order then placement order. Units are grouped by
/// `group_into_parts`: a designator plus library symbol, one instance per
/// unit. Unannotated parts are kept and flagged.
pub fn design_parts(sheets: &[(InstancePath, &SchematicSheet)]) -> Vec<DesignPart> {
    design_parts_named(sheets, &ChannelDesignators::default())
}

/// [`design_parts`] with a part in a channel of a repeated sheet under its
/// channel designator, the one the netlist's terminals carry.
pub fn design_parts_named(
    sheets: &[(InstancePath, &SchematicSheet)],
    channels: &ChannelDesignators,
) -> Vec<DesignPart> {
    let mut parts = Vec::new();
    for (path, sheet) in sheets {
        let placed: Vec<_> = sheet.symbols.iter().filter(|s| !s.is_power).collect();
        let names: Vec<_> = placed
            .iter()
            .copied()
            .map(|s| channels.designator_of(s, path))
            .collect();
        let keys: Vec<PartKey<'_>> = placed
            .iter()
            .zip(&names)
            .map(|(s, name)| PartKey {
                reference: name.trim(),
                ..s.part_key()
            })
            .collect();
        for group in group_into_parts(keys.iter().copied()) {
            let designator = keys[group[0]].reference;
            parts.push(DesignPart {
                sheet: path.clone(),
                designator: designator.to_string(),
                units: group.iter().map(|&i| placed[i].uuid).collect(),
                needs_annotation: needs_annotation(designator),
            });
        }
    }
    parts
}

#[cfg(test)]
#[path = "parts_tests.rs"]
mod tests;
