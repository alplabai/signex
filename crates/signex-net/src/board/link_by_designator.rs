//! The explicit migration: link footprints to schematic parts by designator.
//!
//! Net sync never reads a designator. A board made before footprints stored a
//! link is moved over once, by this edit, which the user applies knowingly.

use std::collections::HashMap;

use signex_types::pcb::{PcbBoard, SymbolLink};
use signex_types::schematic::{InstancePath, PartKey, SchematicSheet, group_into_parts};
use uuid::Uuid;

/// One footprint's link before or after the edit.
#[derive(Debug, Clone, PartialEq)]
pub struct FootprintLinkChange {
    pub footprint: Uuid,
    pub link: Option<SymbolLink>,
}

/// The links an edit sets. Pure data: applying it returns its exact inverse.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardLinkEdit {
    pub links: Vec<FootprintLinkChange>,
}

impl BoardLinkEdit {
    /// Write the links into `board` and return the edit that undoes them. A
    /// footprint no longer on the board is skipped and left out of the inverse.
    pub fn apply(self, board: &mut PcbBoard) -> BoardLinkEdit {
        let mut undo = Vec::with_capacity(self.links.len());
        for change in self.links {
            let Some(fp) = board
                .footprints
                .iter_mut()
                .find(|f| f.uuid == change.footprint)
            else {
                continue;
            };
            let old = std::mem::replace(&mut fp.schematic_link, change.link);
            undo.push(FootprintLinkChange {
                footprint: change.footprint,
                link: old,
            });
        }
        BoardLinkEdit { links: undo }
    }
}

/// What the migration decided, by footprint uuid in board order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LinkByDesignatorReport {
    /// Footprints the edit links.
    pub linked: Vec<Uuid>,
    /// Footprints whose designator names several parts (or a part another
    /// footprint holds or claims); left unlinked.
    pub ambiguous: Vec<Uuid>,
    /// Footprints with no usable designator or no part carrying it.
    pub unmatched: Vec<Uuid>,
}

/// A physical part on one sheet occurrence: its designator and unit uuids.
struct Part {
    sheet: InstancePath,
    designator: String,
    units: Vec<Uuid>,
}

/// Link each unlinked footprint to the one part its reference names.
///
/// `sheets` are the placed sheets with their occurrence paths. Units are
/// grouped into parts by the schematic's own rule (`group_into_parts`): a
/// designator plus library symbol, one instance per unit. A footprint is
/// linked only when exactly one part carries its reference and no other
/// footprint holds or claims that part. Linked footprints are never touched.
pub fn link_footprints_by_designator(
    board: &PcbBoard,
    sheets: &[(InstancePath, &SchematicSheet)],
) -> (BoardLinkEdit, LinkByDesignatorReport) {
    let parts = parts_of(sheets);
    let mut by_designator: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, part) in parts.iter().enumerate() {
        by_designator
            .entry(part.designator.as_str())
            .or_default()
            .push(i);
    }

    // A part a linked footprint already holds.
    let held = |part: &Part| {
        board.footprints.iter().any(|f| {
            f.schematic_link.as_ref().is_some_and(|l| {
                l.sheet == part.sheet && l.units.iter().any(|u| part.units.contains(u))
            })
        })
    };

    let mut report = LinkByDesignatorReport::default();
    let mut sole: Vec<(Uuid, usize)> = Vec::new();
    let mut claims: HashMap<usize, usize> = HashMap::new();
    for fp in board
        .footprints
        .iter()
        .filter(|f| f.schematic_link.is_none())
    {
        let reference = fp.reference.trim();
        let candidates = by_designator.get(reference).map_or(&[][..], Vec::as_slice);
        match candidates {
            _ if reference.is_empty() || reference.contains('?') => report.unmatched.push(fp.uuid),
            [] => report.unmatched.push(fp.uuid),
            [one] if !held(&parts[*one]) => {
                *claims.entry(*one).or_default() += 1;
                sole.push((fp.uuid, *one));
            }
            _ => report.ambiguous.push(fp.uuid),
        }
    }

    let mut edit = BoardLinkEdit::default();
    for (footprint, part) in sole {
        if claims[&part] > 1 {
            report.ambiguous.push(footprint);
            continue;
        }
        let p = &parts[part];
        edit.links.push(FootprintLinkChange {
            footprint,
            link: Some(SymbolLink::new(p.sheet.clone(), p.units.iter().copied())),
        });
        report.linked.push(footprint);
    }
    // Reports read in board order whatever branch decided them.
    let rank: HashMap<Uuid, usize> = board
        .footprints
        .iter()
        .enumerate()
        .map(|(i, f)| (f.uuid, i))
        .collect();
    edit.links.sort_by_key(|c| rank[&c.footprint]);
    for list in [
        &mut report.linked,
        &mut report.ambiguous,
        &mut report.unmatched,
    ] {
        list.sort_by_key(|u| rank[u]);
    }
    (edit, report)
}

fn parts_of(sheets: &[(InstancePath, &SchematicSheet)]) -> Vec<Part> {
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
            if designator.is_empty() || designator.contains('?') {
                continue;
            }
            parts.push(Part {
                sheet: path.clone(),
                designator: designator.to_string(),
                units: group.iter().map(|&i| placed[i].uuid).collect(),
            });
        }
    }
    parts
}
