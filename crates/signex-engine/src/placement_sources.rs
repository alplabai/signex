//! Content for the cursor that is not a copy of the selection (#1667): a
//! design block (another sheet's content), drawings imported from a file, and
//! another of a placed part (Copy attributes).

use signex_types::schematic::{Point, SchDrawing, SchematicSheet};
use uuid::Uuid;

use crate::{CopiedSelection, Engine, EngineError, PlacementPayload};

/// A sheet's content as a copy, to be placed as a design block.
#[derive(Debug, Clone)]
pub struct DesignBlock {
    pub copied: CopiedSelection,
    /// Sheet symbols the block's sheet holds, which are not placed: each names
    /// a child file relative to the sheet it sits on.
    pub skipped_sheet_symbols: usize,
}

impl DesignBlock {
    /// The parts, wires, buses, labels, junctions, no-connects, text and
    /// drawings of `sheet`, with the bodies its parts use. `None` when the
    /// sheet holds none of those; `grid_mm` snaps the anchor as a copy does.
    pub fn from_sheet(sheet: &SchematicSheet, grid_mm: f64) -> Result<Option<Self>, EngineError> {
        let mut fragment = SchematicSheet {
            uuid: Uuid::now_v7(),
            symbols: sheet.symbols.clone(),
            wires: sheet.wires.clone(),
            buses: sheet.buses.clone(),
            bus_entries: sheet.bus_entries.clone(),
            labels: sheet.labels.clone(),
            junctions: sheet.junctions.clone(),
            no_connects: sheet.no_connects.clone(),
            text_notes: sheet.text_notes.clone(),
            drawings: sheet.drawings.clone(),
            ..Default::default()
        };
        for symbol in &fragment.symbols {
            if let Some(body) = sheet.body_of(symbol) {
                fragment
                    .lib_symbols
                    .insert(symbol.body_key().to_owned(), body.clone());
            }
        }
        let copied = CopiedSelection::from_fragment(fragment, grid_mm)?;
        Ok(copied.map(|copied| Self {
            copied,
            skipped_sheet_symbols: sheet.child_sheets.len(),
        }))
    }
}

impl CopiedSelection {
    /// `drawings` (sheet coordinates) as a copy anchored at their centre.
    pub fn from_drawings(
        drawings: Vec<SchDrawing>,
        grid_mm: f64,
    ) -> Result<Option<Self>, EngineError> {
        let fragment = SchematicSheet {
            uuid: Uuid::now_v7(),
            drawings,
            ..Default::default()
        };
        Self::from_fragment(fragment, grid_mm)
    }
}

impl Engine {
    /// Another of the placed part `id`, for the cursor: its body, value,
    /// footprint, fields and look, its origin on the cursor and `reference`
    /// as its designator. Unlocked, with no per-placement designators. `None`
    /// when `id` names no part on the sheet.
    pub fn part_copy_payload(&self, id: Uuid, reference: String) -> Option<PlacementPayload> {
        let doc = self.document();
        let mut symbol = doc.symbols.iter().find(|s| s.uuid == id)?.clone();
        let origin = symbol.position;
        symbol.position = Point::new(0.0, 0.0);
        for text in [&mut symbol.ref_text, &mut symbol.val_text]
            .into_iter()
            .flatten()
        {
            text.position = Point::new(text.position.x - origin.x, text.position.y - origin.y);
        }
        symbol.uuid = Uuid::now_v7();
        symbol.instances.clear();
        symbol.locked = false;
        symbol.reference = reference;
        let mut fragment = SchematicSheet {
            uuid: Uuid::now_v7(),
            ..Default::default()
        };
        if let Some(body) = doc.body_of(&symbol) {
            fragment
                .lib_symbols
                .insert(symbol.body_key().to_owned(), body.clone());
        }
        fragment.symbols.push(symbol);
        Some(PlacementPayload::new(fragment))
    }
}

#[cfg(test)]
#[path = "placement_sources_tests.rs"]
mod tests;
