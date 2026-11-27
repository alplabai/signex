//! `query_net_ties document=<doc>` (#1277; Caner, 2026-11-25): the net ties
//! of one sheet, each with the nets its pins are on, or of one board, each
//! net-tie footprint with the nets its pads bridge. Read through
//! `signex_types::net_tie`, the functions the netlist export reads; the
//! sheet's nets are its own connectivity (`signex_net::build_netlist`).
//! Shapes: `docs/CLI.md`, "Net ties".

use serde_json::{Value, json};
use signex_types::net_tie::{BoardNetTie, NetTie, NetTiePin, board_net_ties, net_ties};
use signex_types::schematic::InstancePath;

use super::Answer;
use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::app::document::DocumentKey;
use crate::app::document_ids::DocumentKind;

fn pins_json(pins: &[NetTiePin], key: &str) -> Vec<Value> {
    pins.iter()
        .map(|p| json!({ key: p.pin, "net": p.net }))
        .collect()
}

fn sheet_tie_json(tie: &NetTie) -> Value {
    json!({
        "id": tie.symbol.to_string(),
        "designator": tie.designator,
        "nets": tie.nets(),
        "pins": pins_json(&tie.pins, "pin"),
        "status": tie.status().as_str(),
    })
}

fn board_tie_json(tie: &BoardNetTie) -> Value {
    json!({
        "id": tie.footprint.to_string(),
        "designator": tie.designator,
        "component_type": tie.component_type,
        "nets": tie.nets(),
        "pads": pins_json(&tie.pads, "pad"),
        "status": tie.status().as_str(),
    })
}

impl Signex {
    /// `query_net_ties document=<doc>`: a loaded sheet's or board's ties.
    pub(super) fn query_net_ties(&self, args: &CommandArgs) -> Answer {
        let entry = self.document_operand(args)?;
        let (kind, ties): (&str, Vec<Value>) = match &entry.key {
            Some(DocumentKey::Schematic(path)) => {
                let sheet = self
                    .document_state
                    .engines
                    .get(path)
                    .ok_or("the sheet is not loaded")?
                    .document();
                let netlist = signex_net::build_netlist(sheet);
                let ties = net_ties(&netlist, &[(InstancePath::root(), sheet)]);
                ("schematic", ties.iter().map(sheet_tie_json).collect())
            }
            Some(DocumentKey::Pcb(path)) => {
                let board = self
                    .document_state
                    .pcb_board(path)
                    .ok_or_else(|| format!("{} is not loaded", entry.name))?;
                (
                    "pcb",
                    board_net_ties(board).iter().map(board_tie_json).collect(),
                )
            }
            None if matches!(entry.kind, DocumentKind::Schematic | DocumentKind::Pcb) => {
                return Err(format!("{} is not loaded", entry.name));
            }
            _ => {
                return Err(
                    "`query_net_ties` reads a schematic sheet or a board; this document is neither"
                        .into(),
                );
            }
        };
        Ok(json!({
            "document": entry.name,
            "kind": kind,
            "count": ties.len(),
            "ties": ties,
        }))
    }
}
