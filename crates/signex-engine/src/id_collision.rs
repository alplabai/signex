//! The caller mints the id of a new object (#2429, ADR-0006), so `execute`
//! refuses a command whose new id is already in the sheet, before anything
//! is touched.
//!
//! A single command scans the sheet. A batch (a paste of thousands of items)
//! builds one index of the sheet's ids on its first check and keeps it in
//! step as its commands apply, so each check is a hash lookup, not a scan.
//! An edit that is not a plain placement only marks the index stale, so a
//! batch of edits with no placement after them never builds it.

use std::collections::HashSet;

use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use crate::command::Command;
use crate::{Engine, EngineError};

/// Every object id of a sheet.
pub(crate) type IdIndex = HashSet<Uuid>;

/// The id index of the running batch.
#[derive(Debug, Default)]
pub(crate) enum BatchIds {
    /// No batch runs: each check scans the sheet.
    #[default]
    Off,
    /// Built on the next check.
    Stale,
    Ready(IdIndex),
}

impl Engine {
    /// The refusal for `cmd`, or `None` when its new ids are free.
    pub(crate) fn id_in_use_refusal(&mut self, cmd: &Command) -> Option<EngineError> {
        let ids = new_object_ids(cmd);
        if ids.is_empty() {
            return None;
        }
        if matches!(self.batch_ids, BatchIds::Stale) {
            self.batch_ids = BatchIds::Ready(id_index(&self.document));
        }
        let mut seen = HashSet::new();
        ids.into_iter()
            .find(|id| !seen.insert(*id) || self.holds_id(*id))
            .map(EngineError::IdInUse)
    }

    fn holds_id(&self, id: Uuid) -> bool {
        match &self.batch_ids {
            // The index may still hold an id a merge removed; the scan confirms a hit.
            BatchIds::Ready(index) => index.contains(&id) && holds_id(&self.document, id),
            BatchIds::Off | BatchIds::Stale => holds_id(&self.document, id),
        }
    }

    /// Run `batch` with one id index for the whole batch, built when first read.
    pub(crate) fn with_id_index<T>(&mut self, batch: impl FnOnce(&mut Self) -> T) -> T {
        let outer = std::mem::replace(&mut self.batch_ids, BatchIds::Stale);
        let result = batch(self);
        self.batch_ids = outer;
        result
    }

    /// Bring the batch index up to date after a command changed the sheet.
    /// `added` is [`index_update`] of that command.
    // shortcut: junction dots the engine mints mid-batch are not indexed; their
    // random v7 ids cannot meet a caller's fresh id. Index them once autoplace
    // takes its ids from the caller (#2429).
    pub(crate) fn note_applied(&mut self, added: Option<Vec<Uuid>>) {
        match (&mut self.batch_ids, added) {
            (BatchIds::Ready(index), Some(added)) => index.extend(added),
            (BatchIds::Ready(_), None) => self.batch_ids = BatchIds::Stale,
            (BatchIds::Off | BatchIds::Stale, _) => {}
        }
    }
}

/// The ids a command adds when it only adds objects, so the batch index can
/// be extended; `None` when it must be rebuilt.
pub(crate) fn index_update(cmd: &Command) -> Option<Vec<Uuid>> {
    let ids = new_object_ids(cmd);
    (!ids.is_empty() || matches!(cmd, Command::RegisterLibSymbol { .. })).then_some(ids)
}

/// The ids `cmd` adds to the sheet, for every command that places an object
/// under a caller-minted id.
fn new_object_ids(cmd: &Command) -> Vec<Uuid> {
    match cmd {
        Command::BreakWire { new_wire, .. } => vec![*new_wire],
        Command::PlaceSymbol { symbol } => vec![symbol.uuid],
        Command::PlaceWireSegment { wire } | Command::PlaceWireSegmentAsIs { wire } => {
            vec![wire.uuid]
        }
        Command::PlaceBus { bus } => vec![bus.uuid],
        Command::PlaceBusEntry { bus_entry } => vec![bus_entry.uuid],
        Command::PlaceJunction { junction } => vec![junction.uuid],
        Command::PlaceNoConnect { no_connect } => vec![no_connect.uuid],
        Command::PlaceNoErcDirective { directive } => vec![directive.uuid],
        Command::PlaceLabel { label } => vec![label.uuid],
        Command::PlaceTextNote { text_note } => vec![text_note.uuid],
        Command::PlaceSchDrawing { drawing } => vec![drawing.uuid()],
        Command::PlaceChildSheet { child_sheet } => std::iter::once(child_sheet.uuid)
            .chain(child_sheet.pins.iter().map(|p| p.uuid))
            .collect(),
        Command::PlaceSheetPin { pin, .. } => vec![pin.uuid],
        Command::PlaceHarnessConnector { connector } => std::iter::once(connector.uuid)
            .chain(connector.entries.iter().map(|e| e.uuid))
            .collect(),
        Command::PlaceHarnessEntry { entry, .. } => vec![entry.uuid],
        Command::PlaceHarnessWire { wire } => vec![wire.uuid],
        _ => Vec::new(),
    }
}

fn id_index(sheet: &SchematicSheet) -> IdIndex {
    let mut index = IdIndex::new();
    index.extend(sheet.symbols.iter().map(|o| o.uuid));
    index.extend(sheet.wires.iter().map(|o| o.uuid));
    index.extend(sheet.buses.iter().map(|o| o.uuid));
    index.extend(sheet.bus_entries.iter().map(|o| o.uuid));
    index.extend(sheet.junctions.iter().map(|o| o.uuid));
    index.extend(sheet.no_connects.iter().map(|o| o.uuid));
    index.extend(sheet.no_erc_directives.iter().map(|o| o.uuid));
    index.extend(sheet.labels.iter().map(|o| o.uuid));
    index.extend(sheet.text_notes.iter().map(|o| o.uuid));
    index.extend(sheet.drawings.iter().map(|o| o.uuid()));
    for symbol in &sheet.child_sheets {
        index.insert(symbol.uuid);
        index.extend(symbol.pins.iter().map(|p| p.uuid));
    }
    index.extend(sheet.harness.ids());
    index
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
        || sheet.harness.holds_id(id)
}
