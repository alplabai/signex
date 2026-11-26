//! Compare Board with Schematic (#2462): both change lists — Update PCB's
//! ([`super::diff_board`]) and Back Annotate's ([`super::diff_schematic`]) —
//! as one report of differences, each with its own direction.
//!
//! A row pairs the Update PCB row and the Back Annotate row that change the
//! same field of the same footprint, so a value that differs is one row
//! with two possible directions (`rows.rs`). Directions start as decided
//! on 2026-11-19: destructive or board-originated rows (Extra on board,
//! Footprint mismatches) and rows Update PCB starts unticked are skipped;
//! any other row takes the schematic's side when it can, else the board's. A
//! blocked side keeps its reason and is never chosen.
//!
//! The report keeps both lists with ticks that follow the directions:
//! [`CompareReport::ticks`] hands them to Update PCB's Execute and Back
//! Annotate's write. A footprint row's direction changes the board the net
//! rows are computed for, so [`CompareReport::set_direction`] says when the
//! caller must run [`CompareReport::refresh_nets`]. The plain-text report:
//! `text.rs`. The command-line answer and its row ids: `answer.rs`; the
//! directions a command-line Apply names: `choose.rs`. Mapping and
//! defaults: `docs/UPDATE_PCB.md`, "Compare Board with Schematic".

mod answer;
mod choose;
mod rows;
mod text;

pub use answer::unavailable_text;
pub use choose::ChooseRefusal;
pub use rows::compare;

use signex_net::cross_probe::PartOccurrence;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::back::{BackBlock, BackChangeList, BackGroup, BackKey, BackNote};
use super::change::{EcoBlock, EcoChangeList, EcoKey, EcoNote};
use super::{EcoInput, refresh_nets};

/// The kinds of difference, in the order the report groups them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CompareKind {
    MissingOnBoard,
    ExtraOnBoard,
    NetDifference,
    FootprintMismatch,
    ValueChange,
}

impl CompareKind {
    pub const ALL: [CompareKind; 5] = [
        Self::MissingOnBoard,
        Self::ExtraOnBoard,
        Self::NetDifference,
        Self::FootprintMismatch,
        Self::ValueChange,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::MissingOnBoard => "Missing on board",
            Self::ExtraOnBoard => "Extra on board",
            Self::NetDifference => "Net differences",
            Self::FootprintMismatch => "Footprint mismatches",
            Self::ValueChange => "Value changes",
        }
    }
}

/// Which side wins a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// The schematic wins: the board changes.
    UpdatePcb,
    /// The board wins: the schematic changes.
    UpdateSchematic,
    /// Nothing changes.
    Skip,
}

impl Direction {
    pub const ALL: [Direction; 3] = [Self::UpdatePcb, Self::UpdateSchematic, Self::Skip];
}

/// What a direction does to a row, for the caption beside its choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareAction {
    AddToBoard,
    LinkFootprint,
    RemoveFromBoard,
    AddNet,
    RemoveNet,
    RenameNet,
    MovePin,
    ConnectPin,
    DisconnectPin,
    SwapFootprint,
    TakeBoardFootprint,
    SetValue,
    TakeBoardValue,
    SetDesignator,
    TakeBoardDesignator,
}

impl CompareAction {
    const fn label(self) -> &'static str {
        match self {
            Self::AddToBoard => "Add to board",
            Self::LinkFootprint => "Link footprint",
            Self::RemoveFromBoard => "Remove from board",
            Self::AddNet => "Add net",
            Self::RemoveNet => "Remove net",
            Self::RenameNet => "Rename net",
            Self::MovePin => "Move pin to net",
            Self::ConnectPin => "Connect pin",
            Self::DisconnectPin => "Disconnect pin",
            Self::SwapFootprint => "Swap footprint",
            Self::TakeBoardFootprint => "Take board footprint",
            Self::SetValue => "Set value",
            Self::TakeBoardValue => "Take board value",
            Self::SetDesignator => "Set designator",
            Self::TakeBoardDesignator => "Take board designator",
        }
    }
}

/// What a row is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareObject {
    /// A part or footprint, by designator.
    Designator(String),
    Net(String),
    /// One pad of a footprint.
    Pin {
        reference: String,
        pad: String,
    },
}

/// What one side holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareValue {
    Text(String),
    /// A part: its value and footprint name.
    Part {
        value: String,
        footprint: String,
    },
    /// A pad on no net.
    NoNet,
}

/// Where a row's object is, for its jump links: the part on its sheet, the
/// footprint on the board, and the net on each side.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompareJump {
    pub part: Option<PartOccurrence>,
    pub footprint: Option<Uuid>,
    pub schematic_net: Option<String>,
    pub board_net: Option<String>,
}

/// Why a direction cannot be chosen for a row.
#[derive(Debug, Clone, PartialEq)]
pub enum Unavailable {
    /// Update PCB has no change for this row.
    NoBoardChange,
    /// Update PCB lists the change but cannot make it.
    Board(EcoBlock),
    /// Taking this kind of difference into the schematic is not built yet.
    SchematicLater,
    /// Back Annotate lists the change but cannot make it.
    Schematic(BackBlock),
}

/// A row's identity: the change list rows it pairs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompareRowId {
    pub pcb: Option<EcoKey>,
    pub back: Option<BackKey>,
}

/// One difference.
#[derive(Debug, Clone, PartialEq)]
pub struct CompareRow {
    pub kind: CompareKind,
    pub object: CompareObject,
    /// The schematic's side; `None`: not in the schematic.
    pub schematic: Option<CompareValue>,
    /// The board's side; `None`: not on the board.
    pub board: Option<CompareValue>,
    /// The Update PCB row, when there is one.
    pub pcb: Option<EcoKey>,
    /// The Back Annotate row, when there is one.
    pub back: Option<BackKey>,
    pub pcb_blocked: Option<EcoBlock>,
    pub back_blocked: Option<BackBlock>,
    /// What Update PCB does to the row, when it has a change.
    pub pcb_action: Option<CompareAction>,
    /// What Update schematic does to the row, when it has a change.
    pub schematic_action: Option<CompareAction>,
    pub direction: Direction,
    /// Update PCB deletes something from the board.
    pub removes_from_board: bool,
    pub jump: CompareJump,
}

impl CompareRow {
    pub fn id(&self) -> CompareRowId {
        CompareRowId {
            pcb: self.pcb.clone(),
            back: self.back,
        }
    }

    /// Whether `direction` can be chosen.
    pub fn allows(&self, direction: Direction) -> bool {
        self.unavailable(direction).is_none()
    }

    /// Why `direction` cannot be chosen; `None` when it can.
    pub fn unavailable(&self, direction: Direction) -> Option<Unavailable> {
        match direction {
            Direction::UpdatePcb => match (&self.pcb, &self.pcb_blocked) {
                (None, _) => Some(Unavailable::NoBoardChange),
                (Some(_), Some(why)) => Some(Unavailable::Board(why.clone())),
                (Some(_), None) => None,
            },
            Direction::UpdateSchematic => match (&self.back, &self.back_blocked) {
                (None, _) => Some(Unavailable::SchematicLater),
                (Some(_), Some(why)) => Some(Unavailable::Schematic(why.clone())),
                (Some(_), None) => None,
            },
            Direction::Skip => None,
        }
    }

    /// What `direction` does to the row; `None` for Skip.
    pub fn action(&self, direction: Direction) -> Option<CompareAction> {
        match direction {
            Direction::UpdatePcb => self.pcb_action,
            Direction::UpdateSchematic => self.schematic_action,
            Direction::Skip => None,
        }
    }
}

/// How much was compared, for the in-sync card.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Compared {
    pub parts: usize,
    pub nets: usize,
    pub footprints: usize,
}

/// Where the rows go with the directions chosen now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompareCounts {
    pub to_pcb: usize,
    pub to_schematic: usize,
    /// Skipped rows, and Update PCB rows held back by a skipped row they
    /// need.
    pub skipped: usize,
    pub total: usize,
}

impl CompareCounts {
    /// The changes Apply makes.
    pub fn applied(self) -> usize {
        self.to_pcb + self.to_schematic
    }
}

/// The whole report, for the board at the revision of its lists.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompareReport {
    /// In kind order, then in change-list order.
    pub rows: Vec<CompareRow>,
    pub compared: Compared,
    eco: EcoChangeList,
    back: BackChangeList,
}

impl CompareReport {
    /// The board revision both lists were read at.
    pub fn revision(&self) -> u64 {
        self.eco.revision
    }

    /// The Update PCB list, ticked as the directions say.
    pub fn eco(&self) -> &EcoChangeList {
        &self.eco
    }

    /// The Back Annotate list, ticked as the directions say.
    pub fn back(&self) -> &BackChangeList {
        &self.back
    }

    pub fn eco_notes(&self) -> &[EcoNote] {
        &self.eco.notes
    }

    pub fn back_notes(&self) -> &[BackNote] {
        &self.back.notes
    }

    /// No difference at all.
    pub fn in_sync(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn row(&self, id: &CompareRowId) -> Option<&CompareRow> {
        self.rows.iter().find(|r| &r.id() == id)
    }

    /// How many rows are of `kind`; `None`: every row.
    pub fn kind_count(&self, kind: Option<CompareKind>) -> usize {
        self.rows
            .iter()
            .filter(|r| kind.is_none_or(|k| r.kind == k))
            .count()
    }

    /// Choose `direction` for the row `id`. A direction the row does not
    /// allow changes nothing. Returns whether a footprint row's tick
    /// changed, after which the net rows must be recomputed
    /// ([`Self::refresh_nets`]).
    pub fn set_direction(&mut self, id: &CompareRowId, direction: Direction) -> bool {
        let Some(index) = self.rows.iter().position(|r| &r.id() == id) else {
            return false;
        };
        let footprint_changed = self.set_direction_unfolded(index, direction);
        rows::tick_folded(&mut self.eco);
        footprint_changed
    }

    /// [`Self::set_direction`] for the row at `index`, leaving the folded
    /// rows' ticks to the caller: one [`rows::tick_folded`] after a batch,
    /// not one per row.
    fn set_direction_unfolded(&mut self, index: usize, direction: Direction) -> bool {
        let row = &mut self.rows[index];
        if !row.allows(direction) || row.direction == direction {
            return false;
        }
        row.direction = direction;
        let (pcb, back) = (row.pcb.clone(), row.back);
        let mut footprint_changed = false;
        if let Some(key) = pcb {
            footprint_changed = self.eco.set_on(&key, direction == Direction::UpdatePcb);
        }
        if let Some(key) = back {
            self.back
                .set_on(&key, direction == Direction::UpdateSchematic);
        }
        footprint_changed
    }

    /// Choose `direction` for every row of `kind` (`None`: every row) that
    /// allows it; the others keep theirs. Returns whether a footprint row's
    /// tick changed, as [`Self::set_direction`] does.
    pub fn set_shown(&mut self, kind: Option<CompareKind>, direction: Direction) -> bool {
        let mut footprint_changed = false;
        for index in 0..self.rows.len() {
            if kind.is_none_or(|k| self.rows[index].kind == k) {
                footprint_changed |= self.set_direction_unfolded(index, direction);
            }
        }
        rows::tick_folded(&mut self.eco);
        footprint_changed
    }

    /// An Update PCB row chosen but held back: a row it needs is skipped.
    pub fn waits(&self, row: &CompareRow) -> bool {
        row.direction == Direction::UpdatePcb
            && row
                .pcb
                .as_ref()
                .and_then(|key| self.eco.row(key))
                .is_some_and(|eco| !self.eco.applies(eco))
    }

    pub fn counts(&self) -> CompareCounts {
        let mut counts = CompareCounts {
            total: self.rows.len(),
            ..CompareCounts::default()
        };
        for row in &self.rows {
            match row.direction {
                Direction::UpdatePcb if !self.waits(row) => counts.to_pcb += 1,
                Direction::UpdateSchematic => counts.to_schematic += 1,
                _ => counts.skipped += 1,
            }
        }
        counts
    }

    /// The rows whose chosen change deletes something from the board, for
    /// the confirmation before Apply.
    pub fn board_deletions(&self) -> Vec<&CompareRow> {
        self.rows
            .iter()
            .filter(|r| {
                r.removes_from_board && r.direction == Direction::UpdatePcb && !self.waits(r)
            })
            .collect()
    }

    /// Both lists with only the chosen rows ticked: the Update PCB rows of
    /// rows set to Update PCB, the Back Annotate rows of rows set to Update
    /// schematic. A blocked row stays unticked.
    pub fn ticks(&self) -> (EcoChangeList, BackChangeList) {
        let mut eco = self.eco.clone();
        let mut back = self.back.clone();
        for row in &mut eco.rows {
            row.on = false;
        }
        for group in [
            BackGroup::Designators,
            BackGroup::Values,
            BackGroup::Footprints,
        ] {
            back.set_group_on(group, false);
        }
        for row in &self.rows {
            if let (Some(key), Direction::UpdatePcb) = (&row.pcb, row.direction) {
                eco.set_on(key, true);
            }
            if let (Some(key), Direction::UpdateSchematic) = (&row.back, row.direction) {
                back.set_on(key, true);
            }
        }
        rows::tick_folded(&mut eco);
        (eco, back)
    }

    fn sync_ticks(&mut self) {
        let (eco, back) = self.ticks();
        self.eco = eco;
        self.back = back;
    }

    /// Carry the directions over from `previous` (the report before a
    /// Compare again, or before a net refresh): a row found there whose
    /// changes are still the same choice keeps its direction where it is
    /// allowed.
    pub fn keep_choices_from(&mut self, previous: &CompareReport) {
        for index in 0..self.rows.len() {
            let row = &self.rows[index];
            let Some(old) = previous.row(&row.id()) else {
                continue;
            };
            let same_pcb = match &row.pcb {
                Some(key) => match (self.eco.row(key), previous.eco.row(key)) {
                    (Some(now), Some(then)) => then.change.same_choice(&now.change),
                    _ => false,
                },
                None => true,
            };
            let same_back = match &row.back {
                Some(key) => match (self.back.row(key), previous.back.row(key)) {
                    (Some(now), Some(then)) => now.change == then.change,
                    _ => false,
                },
                None => true,
            };
            if same_pcb && same_back {
                let direction = old.direction;
                self.set_direction_unfolded(index, direction);
            }
        }
        rows::tick_folded(&mut self.eco);
    }

    /// Recompute the net rows for the footprint directions chosen now,
    /// keeping every direction that still applies, and describe the board's
    /// side of footprint rows from `input.board`.
    pub fn refresh_nets(&mut self, input: &EcoInput) {
        refresh_nets(&mut self.eco, input);
        let mut fresh = compare(&self.eco, &self.back);
        fresh.compared = self.compared;
        fresh.keep_choices_from(self);
        fresh.describe_board(input.board);
        *self = fresh;
    }

    /// The board's side of Extra on board rows (value and footprint) and of
    /// footprint mismatches (the footprint's name), read from `board`.
    pub fn describe_board(&mut self, board: &PcbBoard) {
        for row in &mut self.rows {
            let Some(fp) = row
                .jump
                .footprint
                .and_then(|id| board.footprints.iter().find(|f| f.uuid == id))
            else {
                continue;
            };
            match row.kind {
                CompareKind::ExtraOnBoard => {
                    row.board = Some(CompareValue::Part {
                        value: fp.value.clone(),
                        footprint: fp.footprint_id.clone(),
                    });
                }
                CompareKind::FootprintMismatch if !fp.footprint_id.is_empty() => {
                    row.board = Some(CompareValue::Text(fp.footprint_id.clone()));
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod answer_tests;
#[cfg(test)]
mod lock_row_tests;
#[cfg(test)]
mod tests;
