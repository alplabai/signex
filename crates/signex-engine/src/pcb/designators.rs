//! Designator and value edits of board footprints (#1312): one footprint's
//! text from the Properties panel, and Positional Re-Annotate, which numbers
//! the board's parts by where they sit.
//!
//! Pure: each function reads a board and returns the commands, which the
//! caller runs as one undo step. Back Annotate carries these edits to the
//! schematic; nothing here touches a sheet.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, PcbBoard};
use signex_types::pcb_layer::BoardSide;
use uuid::Uuid;

use super::command::BoardCommand;
use super::error::BoardCommandError;

/// The footprint text the Properties panel edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FootprintTextField {
    Designator,
    Value,
}

/// Why a footprint text edit did nothing.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TextEditRefusal {
    #[error(transparent)]
    Command(#[from] BoardCommandError),
    #[error("{0} is locked")]
    Locked(String),
    #[error("a designator cannot be empty")]
    EmptyDesignator,
}

/// The command that sets `field` of `footprint` to `text`, or `None` when it
/// already reads so. A designator is trimmed and may not be empty; a value is
/// kept as typed. A locked footprint is refused, as for placement edits.
pub fn text_edit_command(
    board: &PcbBoard,
    footprint: Uuid,
    field: FootprintTextField,
    text: &str,
) -> Result<Option<BoardCommand>, TextEditRefusal> {
    let fp = find(board, footprint)?;
    if fp.locked {
        return Err(TextEditRefusal::Locked(super::placement::name(fp)));
    }
    let (reference, value) = match field {
        FootprintTextField::Designator => {
            let designator = text.trim();
            if designator.is_empty() {
                return Err(TextEditRefusal::EmptyDesignator);
            }
            (designator.to_string(), fp.value.clone())
        }
        FootprintTextField::Value => (fp.reference.clone(), text.to_string()),
    };
    if reference == fp.reference && value == fp.value {
        return Ok(None);
    }
    Ok(Some(BoardCommand::SetFootprintText {
        footprint,
        reference,
        value,
    }))
}

/// The other footprints on `board` whose designator is `designator`.
pub fn designator_holders<'a>(
    board: &'a PcbBoard,
    designator: &str,
    except: Uuid,
) -> Vec<&'a Footprint> {
    board
        .footprints
        .iter()
        .filter(|fp| fp.uuid != except && fp.reference == designator)
        .collect()
}

/// A designator's prefix and number: `R12` is `("R", Some(12))`, `R?` and `R`
/// are `("R", None)`. `None` when there is no prefix or the rest is not a
/// plain number (`12`, `R1A`, an empty one).
pub fn split_designator(designator: &str) -> Option<(&str, Option<u32>)> {
    let at = designator
        .find(|c: char| c.is_ascii_digit() || c == '?')
        .unwrap_or(designator.len());
    let (prefix, rest) = designator.split_at(at);
    if prefix.trim().is_empty() {
        return None;
    }
    match rest {
        "" | "?" => Some((prefix, None)),
        digits if digits.bytes().all(|b| b.is_ascii_digit()) => {
            Some((prefix, Some(digits.parse().ok()?)))
        }
        _ => None,
    }
}

/// Which way Positional Re-Annotate walks the board. The board's y grows
/// downwards on screen, so "top to bottom" is increasing y.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReannotateOrder {
    /// Columns left to right, each top to bottom.
    #[default]
    ColumnsThenRows,
    /// Rows top to bottom, each left to right.
    RowsThenColumns,
}

/// Where the bottom side's parts fall in the numbering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BottomSideOrder {
    /// Every top-side part first, then the bottom side in the same order.
    #[default]
    AfterTop,
    /// Both sides together, by position only.
    WithTop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReannotateOptions {
    pub order: ReannotateOrder,
    pub bottom: BottomSideOrder,
}

/// One footprint's new designator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    pub footprint: Uuid,
    pub from: String,
    pub to: String,
}

/// Why a footprint keeps its designator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeptReason {
    /// Locked footprints keep theirs; its number is not given to another.
    Locked,
    /// Not a prefix and a number (`12`, `R1A`, empty).
    NoPrefix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub footprint: Uuid,
    pub reference: String,
    pub reason: KeptReason,
}

/// What Positional Re-Annotate would do to a board.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReannotatePlan {
    /// In numbering order.
    pub renames: Vec<Rename>,
    pub kept: Vec<Kept>,
    /// Footprints whose designator is already the one they would get.
    pub unchanged: usize,
}

impl ReannotatePlan {
    /// The text commands of the renames, each keeping the footprint's value.
    pub fn commands(&self, board: &PcbBoard) -> Result<Vec<BoardCommand>, BoardCommandError> {
        self.renames
            .iter()
            .map(|rename| {
                let fp = find(board, rename.footprint)?;
                Ok(BoardCommand::SetFootprintText {
                    footprint: rename.footprint,
                    reference: rename.to.clone(),
                    value: fp.value.clone(),
                })
            })
            .collect()
    }
}

/// Number the board's parts by position, within each prefix from 1: `R`s
/// count `R1, R2, ...` and `C`s their own. The prefix stays. Locked
/// footprints keep their designators, and their numbers are skipped.
pub fn plan_reannotate(board: &PcbBoard, options: ReannotateOptions) -> ReannotatePlan {
    let mut plan = ReannotatePlan::default();
    let mut taken: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
    let mut numbered: Vec<(&Footprint, &str)> = Vec::new();
    for fp in &board.footprints {
        let split = split_designator(&fp.reference);
        let reason = match split {
            _ if fp.locked => KeptReason::Locked,
            None => KeptReason::NoPrefix,
            Some((prefix, _)) => {
                numbered.push((fp, prefix));
                continue;
            }
        };
        if let (KeptReason::Locked, Some((prefix, Some(number)))) = (reason, split) {
            taken.entry(prefix).or_default().insert(number);
        }
        plan.kept.push(Kept {
            footprint: fp.uuid,
            reference: fp.reference.clone(),
            reason,
        });
    }
    numbered.sort_by(|a, b| position_order(a.0, b.0, options));
    let mut next: BTreeMap<&str, u32> = BTreeMap::new();
    for (fp, prefix) in numbered {
        let counter = next.entry(prefix).or_insert(1);
        let skip = taken.get(prefix);
        while skip.is_some_and(|s| s.contains(&*counter)) {
            *counter += 1;
        }
        let to = format!("{prefix}{counter}");
        *counter += 1;
        if to == fp.reference {
            plan.unchanged += 1;
        } else {
            plan.renames.push(Rename {
                footprint: fp.uuid,
                from: fp.reference.clone(),
                to,
            });
        }
    }
    plan
}

/// The walk order of two footprints: side (when the bottom comes after the
/// top), then the two axes in the chosen order at 1 µm, then uuid so equal
/// positions still number the same way every time.
fn position_order(a: &Footprint, b: &Footprint, options: ReannotateOptions) -> Ordering {
    let key = |fp: &Footprint| {
        let side = match options.bottom {
            BottomSideOrder::AfterTop => u8::from(is_bottom(fp)),
            BottomSideOrder::WithTop => 0,
        };
        let (x, y) = (micrometres(fp.position.x), micrometres(fp.position.y));
        let (first, second) = match options.order {
            ReannotateOrder::ColumnsThenRows => (x, y),
            ReannotateOrder::RowsThenColumns => (y, x),
        };
        (side, first, second, fp.uuid)
    };
    key(a).cmp(&key(b))
}

fn is_bottom(fp: &Footprint) -> bool {
    parse_pcb_layer(&fp.layer).and_then(|layer| layer.side()) == Some(BoardSide::Bottom)
}

/// A coordinate in whole micrometres; a non-finite one sorts last.
fn micrometres(mm: f64) -> i64 {
    if mm.is_finite() {
        (mm * 1000.0).round() as i64
    } else {
        i64::MAX
    }
}

fn find(board: &PcbBoard, id: Uuid) -> Result<&Footprint, BoardCommandError> {
    board
        .footprints
        .iter()
        .find(|fp| fp.uuid == id)
        .ok_or(BoardCommandError::FootprintNotFound(id))
}

#[cfg(test)]
#[path = "designators_tests.rs"]
mod tests;
