//! The signal harness commands (#1280): place a connector, an entry or a
//! harness wire, update a connector, and type a sheet entry as a harness.
//! Each is one undo step; consecutive updates of one connector share one,
//! like typing a directive's field. Model and rules:
//! `docs/SIGNAL_HARNESSES.md`.

use crate::*;
use signex_types::schematic::{HarnessConnector, HarnessWire, Point, SelectedKind};

/// The typing-run key of a connector update. The run is also keyed by the
/// connector's uuid, which no symbol shares.
const CONNECTOR_RUN_KEY: &str = "harness-connector";

fn finite(p: &Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// Whether `connector` can be drawn: a type, a finite place, a width and
/// entry pitch above zero, a quarter turn and named entries.
fn connector_error(connector: &HarnessConnector) -> Option<EngineError> {
    let quarter = [0.0, 90.0, 180.0, 270.0].contains(&connector.rotation);
    let sized = connector.width.is_finite()
        && connector.width > 0.0
        && connector.entry_pitch.is_finite()
        && connector.entry_pitch > 0.0;
    if connector.harness_type.trim().is_empty()
        || !finite(&connector.position)
        || !sized
        || !quarter
    {
        return Some(EngineError::InvalidHarnessConnector);
    }
    connector
        .entries
        .iter()
        .any(|e| e.name.trim().is_empty())
        .then_some(EngineError::EmptyHarnessEntryName)
}

fn wire_is_valid(wire: &HarnessWire) -> bool {
    finite(&wire.start) && finite(&wire.end) && wire.start != wire.end
}

impl Engine {
    pub(crate) fn exec_harness(
        &mut self,
        before: SchematicSheet,
        cmd: Command,
    ) -> Result<CommandResult, EngineError> {
        let mut run = None;
        let semantic = match cmd {
            Command::PlaceHarnessConnector { connector } => {
                if let Some(error) = connector_error(&connector) {
                    return Err(error);
                }
                self.document.harness.connectors.push(connector);
                SemanticPatch::ObjectPlaced
            }
            Command::UpdateHarnessConnector { uuid, connector } => {
                if let Some(error) = connector_error(&connector) {
                    return Err(error);
                }
                let Some(slot) = self.document.harness.connector_mut(uuid) else {
                    return Err(EngineError::HarnessNotFound {
                        kind: "connector",
                        id: uuid,
                    });
                };
                // The connector keeps its id whatever the caller passed.
                let connector = HarnessConnector { uuid, ..connector };
                if *slot == connector {
                    return Ok(CommandResult::unchanged());
                }
                *slot = connector;
                run = Some(uuid);
                SemanticPatch::StyleUpdated
            }
            Command::PlaceHarnessEntry {
                connector_id,
                entry,
                index,
            } => {
                if entry.name.trim().is_empty() {
                    return Err(EngineError::EmptyHarnessEntryName);
                }
                let Some(connector) = self.document.harness.connector_mut(connector_id) else {
                    return Err(EngineError::HarnessNotFound {
                        kind: "connector",
                        id: connector_id,
                    });
                };
                let index = index.min(connector.entries.len());
                connector.entries.insert(index, entry);
                SemanticPatch::ObjectPlaced
            }
            Command::PlaceHarnessWire { wire } => {
                if !wire_is_valid(&wire) {
                    return Err(EngineError::InvalidHarnessWire);
                }
                self.document.harness.wires.push(wire);
                SemanticPatch::ObjectPlaced
            }
            Command::SetHarnessEntryType {
                sheet_pin_id,
                harness_type,
            } => {
                let exists = self
                    .document
                    .child_sheets
                    .iter()
                    .any(|cs| cs.pins.iter().any(|p| p.uuid == sheet_pin_id));
                if !exists {
                    return Err(EngineError::HarnessNotFound {
                        kind: "sheet entry",
                        id: sheet_pin_id,
                    });
                }
                let harness_type = harness_type
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty());
                let types = &mut self.document.harness.entry_types;
                if types.get(&sheet_pin_id) == harness_type.as_ref() {
                    return Ok(CommandResult::unchanged());
                }
                match harness_type {
                    Some(t) => {
                        types.insert(sheet_pin_id, t);
                    }
                    None => {
                        types.remove(&sheet_pin_id);
                    }
                }
                SemanticPatch::StyleUpdated
            }
            _ => unreachable!("exec_harness routes only the harness commands"),
        };
        let patch_pair = PatchPair {
            semantic,
            document: DocumentPatch::HARNESS,
        };
        match run {
            Some(id) => self.record_field_edit(before, patch_pair, id, CONNECTOR_RUN_KEY),
            None => self.record_history(before, patch_pair),
        }
        Ok(CommandResult::changed(patch_pair))
    }

    /// The Properties rows of the harness object `item`; `None` when the
    /// sheet has no such object.
    pub(crate) fn harness_info(&self, item: &SelectedItem) -> Option<Vec<(String, String)>> {
        let harness = &self.document.harness;
        let at = |p: &Point| format!("{:.2}, {:.2}", p.x, self.document.page_y(p.y));
        let mut info = Vec::new();
        match item.kind {
            SelectedKind::HarnessConnector => {
                let c = harness.connector(item.uuid)?;
                info.push(("Type".into(), "Harness Connector".into()));
                info.push(("Harness Type".into(), c.harness_type.clone()));
                info.push(("Position".into(), at(&c.position)));
                info.push(("Width".into(), format!("{:.2}", c.width)));
                info.push(("Entry Pitch".into(), format!("{:.2}", c.entry_pitch)));
                info.push(("Entries".into(), c.entries.len().to_string()));
            }
            SelectedKind::HarnessEntry => {
                let (c, index) = harness.entry(item.uuid)?;
                info.push(("Type".into(), "Harness Entry".into()));
                info.push(("Name".into(), c.entries[index].name.clone()));
                info.push(("Harness Type".into(), c.harness_type.clone()));
                info.push(("Position".into(), at(&c.entry_point(index))));
            }
            SelectedKind::HarnessWire => {
                let w = harness.wire(item.uuid)?;
                info.push(("Type".into(), "Harness Wire".into()));
                info.push(("Start".into(), at(&w.start)));
                info.push(("End".into(), at(&w.end)));
            }
            _ => return None,
        }
        Some(info)
    }
}
