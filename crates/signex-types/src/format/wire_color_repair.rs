//! Wires whose `color` cell this build cannot read (cosmetic, so not fatal).
//!
//! The cell stays a foreign cell and is written back unchanged; each one is
//! reported through `SheetLoadRepair`. Format:
//! `docs/architecture/snxsch-wire-color.md`.

use super::load_repair::{RepairedObject, SheetLoadRepair, kind};
use super::sch_rows::WIRE_COLOR_COLUMN;
use crate::schematic::SchematicSheet;

/// One repair per wire still holding a `color` foreign cell.
pub(super) fn unreadable_wire_colors(sheet: &SchematicSheet) -> Vec<SheetLoadRepair> {
    sheet
        .wires
        .iter()
        .enumerate()
        .filter_map(|(i, wire)| {
            let value = wire.unknown_columns.get(WIRE_COLOR_COLUMN)?;
            Some(SheetLoadRepair::WireColorUnreadable {
                object: RepairedObject {
                    kind: kind::WIRE,
                    name: String::new(),
                    position: i + 1,
                },
                value: value.to_string(),
            })
        })
        .collect()
}
