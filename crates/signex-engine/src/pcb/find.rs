//! Find and Replace on a board (#2403): the footprints whose designator,
//! value or footprint name holds the text, and the nets whose name does.
//! Pure: a scan reads the board it is given, and a replace re-reads it, so a
//! hit names where it is, never what it said.

use std::collections::BTreeMap;

use pcb_geom::{ItemRef, net_items};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::command::BoardCommand;

/// What text of the board a hit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoardFindKind {
    Designator,
    Value,
    /// The library footprint a placed footprint was made from.
    Footprint,
    Net,
}

impl BoardFindKind {
    /// The name the results list shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Designator => "Designator",
            Self::Value => "Value",
            Self::Footprint => "Footprint",
            Self::Net => "Net",
        }
    }

    /// Whether Replace writes this text. A net is renamed from the
    /// schematic and a footprint swapped with Change Footprint, so neither
    /// is written here.
    pub fn is_replaceable(self) -> bool {
        matches!(self, Self::Designator | Self::Value)
    }
}

/// One occurrence of the text on a board.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardFindHit {
    pub kind: BoardFindKind,
    /// What the hit selects: the footprint, or every track, via and pad of
    /// the net.
    pub items: Vec<ItemRef>,
    /// The footprint a designator, value or footprint hit is on.
    pub footprint: Option<Uuid>,
    /// The text at scan time, for display only.
    pub text: String,
}

fn holds(text: &str, needle: &str) -> bool {
    text.to_lowercase().contains(needle)
}

/// Every footprint text and net name of `board` holding `query`, compared
/// without case: per footprint its designator, value and footprint name in
/// that order, footprints in board order, then the nets by name. An empty
/// query finds nothing.
pub fn scan_board(board: &PcbBoard, query: &str) -> Vec<BoardFindHit> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for fp in &board.footprints {
        for (kind, text) in [
            (BoardFindKind::Designator, &fp.reference),
            (BoardFindKind::Value, &fp.value),
            (BoardFindKind::Footprint, &fp.footprint_id),
        ] {
            if holds(text, &needle) {
                hits.push(BoardFindHit {
                    kind,
                    items: vec![ItemRef::Footprint(fp.uuid)],
                    footprint: Some(fp.uuid),
                    text: text.clone(),
                });
            }
        }
    }
    let mut nets: Vec<_> = board
        .nets
        .iter()
        .filter(|def| def.number != 0 && holds(&def.name, &needle))
        .collect();
    nets.sort_by(|a, b| a.name.cmp(&b.name));
    hits.extend(nets.into_iter().map(|def| BoardFindHit {
        kind: BoardFindKind::Net,
        items: net_items(board, def.number),
        footprint: None,
        text: def.name.clone(),
    }));
    hits
}

/// The commands that replace `query` with `replacement` in the designator
/// and value hits of `hits`, read from `board` as it is now: one
/// [`BoardCommand::SetFootprintText`] per footprint whose text changes. Net
/// and footprint hits are left alone ([`BoardFindKind::is_replaceable`]),
/// and so is a footprint no longer on the board.
pub fn replace_commands(
    board: &PcbBoard,
    hits: &[BoardFindHit],
    query: &str,
    replacement: &str,
) -> Vec<BoardCommand> {
    let mut wanted: BTreeMap<Uuid, (bool, bool)> = BTreeMap::new();
    for hit in hits.iter().filter(|h| h.kind.is_replaceable()) {
        let Some(id) = hit.footprint else {
            continue;
        };
        let entry = wanted.entry(id).or_default();
        match hit.kind {
            BoardFindKind::Designator => entry.0 = true,
            _ => entry.1 = true,
        }
    }
    let query = query.trim();
    wanted
        .into_iter()
        .filter_map(|(id, (designator, value))| {
            let fp = board.footprints.iter().find(|fp| fp.uuid == id)?;
            let reference = if designator {
                crate::replace_all_ci(&fp.reference, query, replacement)
            } else {
                fp.reference.clone()
            };
            let value = if value {
                crate::replace_all_ci(&fp.value, query, replacement)
            } else {
                fp.value.clone()
            };
            (reference != fp.reference || value != fp.value).then_some(
                BoardCommand::SetFootprintText {
                    footprint: id,
                    reference,
                    value,
                },
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
