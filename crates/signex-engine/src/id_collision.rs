//! The caller mints the id of a new object (#2429, ADR-0006), so `execute`
//! refuses a command whose new id is already in the sheet, before anything
//! is touched.

use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use crate::command::Command;
use crate::{Engine, EngineError};

impl Engine {
    /// The refusal for `cmd`, or `None` when its new id is free.
    pub(crate) fn id_in_use_refusal(&self, cmd: &Command) -> Option<EngineError> {
        let id = new_object_id(cmd)?;
        holds_id(&self.document, id).then_some(EngineError::IdInUse(id))
    }
}

/// The id `cmd` adds to the sheet, for the commands whose id comes from the
/// caller: Break Wire's second half and every paste placement.
fn new_object_id(cmd: &Command) -> Option<Uuid> {
    Some(match cmd {
        Command::BreakWire { new_wire, .. } => *new_wire,
        Command::PlaceSymbol { symbol } => symbol.uuid,
        Command::PlaceWireSegmentAsIs { wire } => wire.uuid,
        Command::PlaceBus { bus } => bus.uuid,
        Command::PlaceBusEntry { bus_entry } => bus_entry.uuid,
        Command::PlaceJunction { junction } => junction.uuid,
        Command::PlaceNoConnect { no_connect } => no_connect.uuid,
        Command::PlaceLabel { label } => label.uuid,
        Command::PlaceTextNote { text_note } => text_note.uuid,
        _ => return None,
    })
}

/// Whether any object of `sheet` carries `id`.
fn holds_id(sheet: &SchematicSheet, id: Uuid) -> bool {
    sheet.symbols.iter().any(|o| o.uuid == id)
        || sheet.wires.iter().any(|o| o.uuid == id)
        || sheet.buses.iter().any(|o| o.uuid == id)
        || sheet.bus_entries.iter().any(|o| o.uuid == id)
        || sheet.junctions.iter().any(|o| o.uuid == id)
        || sheet.no_connects.iter().any(|o| o.uuid == id)
        || sheet.no_erc_directives.iter().any(|o| o.uuid == id)
        || sheet.labels.iter().any(|o| o.uuid == id)
        || sheet.text_notes.iter().any(|o| o.uuid == id)
        || sheet.drawings.iter().any(|o| o.uuid() == id)
        || sheet
            .child_sheets
            .iter()
            .any(|s| s.uuid == id || s.pins.iter().any(|p| p.uuid == id))
}
