//! The report as the command line reads it (#2462): every difference with a
//! stable id, its kind, both sides, the fixes it offers and why a side is
//! blocked, as JSON. The words are the plain-text report's (`text.rs`), so
//! no locale reaches the answer. Shape: `docs/CLI.md`, "Board against
//! schematic".

use serde_json::{Map, Value, json};

use super::text::{back_block_text, back_note_text, object_text};
use super::{
    CompareAction, CompareKind, CompareObject, CompareReport, CompareRow, CompareRowId,
    CompareValue, Direction, Unavailable,
};
use crate::pcb::eco::back::BackKey;
use crate::pcb::eco::change::EcoKey;
use crate::pcb::eco::report::{blocked_text, note_text};

/// The directions that change something, in the order the answer lists them.
const SIDES: [Direction; 2] = [Direction::UpdatePcb, Direction::UpdateSchematic];

impl CompareKind {
    /// The kind's name in an answer.
    pub const fn token(self) -> &'static str {
        match self {
            Self::MissingOnBoard => "missing_on_board",
            Self::ExtraOnBoard => "extra_on_board",
            Self::NetDifference => "net_difference",
            Self::FootprintMismatch => "footprint_mismatch",
            Self::ValueChange => "value_change",
        }
    }
}

impl Direction {
    /// The direction's name in an answer and in an operand.
    pub const fn token(self) -> &'static str {
        match self {
            Self::UpdatePcb => "update_pcb",
            Self::UpdateSchematic => "update_schematic",
            Self::Skip => "skip",
        }
    }
}

impl CompareAction {
    /// The action's name in an answer.
    pub const fn token(self) -> &'static str {
        match self {
            Self::AddToBoard => "add_to_board",
            Self::LinkFootprint => "link_footprint",
            Self::RemoveFromBoard => "remove_from_board",
            Self::AddNet => "add_net",
            Self::RemoveNet => "remove_net",
            Self::RenameNet => "rename_net",
            Self::MovePin => "move_pin",
            Self::ConnectPin => "connect_pin",
            Self::DisconnectPin => "disconnect_pin",
            Self::SwapFootprint => "swap_footprint",
            Self::TakeBoardFootprint => "take_board_footprint",
            Self::SetValue => "set_value",
            Self::TakeBoardValue => "take_board_value",
            Self::SetDesignator => "set_designator",
            Self::TakeBoardDesignator => "take_board_designator",
        }
    }
}

impl CompareRowId {
    /// The row's id in an answer: 16 hex digits hashed from the change list
    /// keys it pairs (footprint, pad, part and net ids, never a position in
    /// the list), so the same difference has the same id in every read.
    pub fn token(&self) -> String {
        let mut key = String::new();
        if let Some(pcb) = &self.pcb {
            key.push_str("pcb ");
            key.push_str(&eco_key_text(pcb));
        }
        if let Some(back) = self.back {
            key.push_str("|back ");
            key.push_str(&back_key_text(back));
        }
        format!("{:016x}", fnv1a(key.as_bytes()))
    }
}

impl CompareRow {
    /// [`CompareRowId::token`] of this row.
    pub fn token(&self) -> String {
        self.id().token()
    }
}

impl CompareReport {
    /// The rows that have the id `token`; at most one.
    pub fn row_by_token(&self, token: &str) -> Option<&CompareRow> {
        self.rows.iter().find(|row| row.token() == token.trim())
    }

    /// "Board matches schematic", or how many differences there are.
    pub fn summary(&self) -> String {
        match self.rows.len() {
            0 => "Board matches schematic".to_string(),
            1 => "1 difference".to_string(),
            n => format!("{n} differences"),
        }
    }

    /// The whole report as the query answers it: the board revision both
    /// lists were read at, whether board and schematic match, the counts by
    /// kind, every difference in report order and the notes.
    pub fn answer(&self) -> Value {
        let mut kinds = Map::new();
        for kind in CompareKind::ALL {
            kinds.insert(kind.token().into(), json!(self.kind_count(Some(kind))));
        }
        let notes: Vec<String> = self
            .eco_notes()
            .iter()
            .map(note_text)
            .chain(self.back_notes().iter().map(back_note_text))
            .collect();
        let differences: Vec<Value> = self.rows.iter().map(|row| self.row_answer(row)).collect();
        json!({
            "revision": self.revision(),
            "matches": self.in_sync(),
            "summary": self.summary(),
            "compared": {
                "parts": self.compared.parts,
                "nets": self.compared.nets,
                "footprints": self.compared.footprints,
            },
            "count": self.rows.len(),
            "kinds": kinds,
            "differences": differences,
            "notes": notes,
        })
    }

    fn row_answer(&self, row: &CompareRow) -> Value {
        let fixes: Vec<Value> = SIDES
            .into_iter()
            .filter(|d| row.allows(*d))
            .map(|d| json!({ "direction": d.token(), "action": row.action(d).map(CompareAction::token) }))
            .collect();
        let blocked: Vec<Value> = SIDES
            .into_iter()
            .filter_map(|d| {
                let why = row.unavailable(d)?;
                Some(json!({ "direction": d.token(), "reason": unavailable_text(&why) }))
            })
            .collect();
        let part = row.jump.part.as_ref().map(
            |part| json!({ "sheet": part.sheet.to_string(), "symbol": part.symbol.to_string() }),
        );
        json!({
            "id": row.token(),
            "kind": row.kind.token(),
            "object": object_answer(&row.object),
            "label": object_text(&row.object),
            "schematic": value_answer(&row.schematic),
            "board": value_answer(&row.board),
            "fixes": fixes,
            "blocked": blocked,
            "default": row.direction.token(),
            "waits": self.waits(row),
            "removes_from_board": row.removes_from_board,
            "board_item": row.jump.footprint.map(|id| id.to_string()),
            "schematic_part": part,
            "schematic_net": row.jump.schematic_net,
            "board_net": row.jump.board_net,
        })
    }
}

/// Why a direction cannot be chosen, in the plain-text report's words.
pub fn unavailable_text(why: &Unavailable) -> String {
    match why {
        Unavailable::NoBoardChange => "Update PCB has no change for this difference".into(),
        Unavailable::Board(block) => blocked_text(block),
        Unavailable::SchematicLater => {
            "taking this difference into the schematic is not built yet".into()
        }
        Unavailable::Schematic(block) => back_block_text(block).into(),
    }
}

fn object_answer(object: &CompareObject) -> Value {
    match object {
        CompareObject::Designator(designator) => json!({ "designator": designator }),
        CompareObject::Net(net) => json!({ "net": net }),
        CompareObject::Pin { reference, pad } => json!({ "designator": reference, "pad": pad }),
    }
}

fn value_answer(side: &Option<CompareValue>) -> Value {
    match side {
        None => Value::Null,
        Some(CompareValue::Text(text)) => json!({ "text": text }),
        Some(CompareValue::Part { value, footprint }) => {
            json!({ "value": value, "footprint": footprint })
        }
        Some(CompareValue::NoNet) => json!({ "no_net": true }),
        Some(CompareValue::Unlinked) => json!({ "unlinked": true }),
    }
}

fn eco_key_text(key: &EcoKey) -> String {
    match key {
        EcoKey::AddFootprint { sheet, part } => format!("add_footprint {sheet} {part}"),
        EcoKey::RemoveFootprint(id) => format!("remove_footprint {id}"),
        EcoKey::ChangeFootprint(id) => format!("change_footprint {id}"),
        EcoKey::SetDesignator(id) => format!("set_designator {id}"),
        EcoKey::SetValue(id) => format!("set_value {id}"),
        EcoKey::Link(id) => format!("link {id}"),
        EcoKey::AddNet(name) => format!("add_net {name}"),
        EcoKey::RemoveNet(number) => format!("remove_net {number}"),
        EcoKey::RenameNet(number) => format!("rename_net {number}"),
        EcoKey::PadNet { footprint, pad } => format!("pad_net {footprint} {pad}"),
    }
}

fn back_key_text(key: BackKey) -> String {
    match key {
        BackKey::Designator(id) => format!("designator {id}"),
        BackKey::Value(id) => format!("value {id}"),
        BackKey::Footprint(id) => format!("footprint {id}"),
    }
}

/// 64-bit FNV-1a: fixed, so an id never changes between builds.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}
