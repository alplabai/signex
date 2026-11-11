//! Back Annotate (#1312): the board's designators and values carried to the
//! schematic, as a reviewable change list. The mirror of [`super::diff_board`]
//! with the board as the source.
//!
//! The rules (decision on #1312, 2026-11-10):
//! - A footprint matches its part only by its stored `SymbolLink` (same
//!   sheet placement, a shared unit). A designator is what may have changed,
//!   so it is never used to match; an unlinked footprint gets a note.
//! - The board wins for each ticked row; every row starts ticked.
//! - A designator row is blocked while the part's schematic designator is
//!   not annotated or is carried by another part, as in Update PCB, and
//!   while the board's designator itself is empty or holds `?`.
//! - A value row is blocked on a sheet placed more than once: the value is
//!   stored once in the sheet file, so it would change every placement. A
//!   designator row writes that placement's designator only.
//! - A footprint row is listed read-only: changing a part's footprint from
//!   the board waits for #2248. Pin and gate swaps have no board model and
//!   are not listed.
//!
//! [`sheet_commands`] turns the ticked rows of one sheet file into engine
//! commands; the caller runs every changed sheet as one undo group.

use std::collections::{BTreeMap, HashMap, HashSet};

use signex_types::pcb::{Footprint, PcbBoard};
use signex_types::schematic::{InstancePath, SchematicSheet};
use uuid::Uuid;

use super::change::GroupTicks;
use super::{EcoPart, is_under};
use crate::{Command, TextTarget, designator_commands};

/// One part of the design, as Back Annotate needs it.
#[derive(Debug, Clone)]
pub struct BackPart {
    pub eco: EcoPart,
    /// The part's sheet file is placed more than once in the project.
    pub shared_sheet: bool,
}

/// What [`diff_schematic`] compares.
#[derive(Debug, Clone, Copy)]
pub struct BackInput<'a> {
    pub board: &'a PcbBoard,
    /// The board revision the list is computed at.
    pub revision: u64,
    pub parts: &'a [BackPart],
    /// Placements whose sheet could not be read.
    pub unreadable: &'a [InstancePath],
}

/// A row's identity: the board footprint it reads from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BackKey {
    Designator(Uuid),
    Value(Uuid),
    Footprint(Uuid),
}

/// The groups the dialog shows rows in, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BackGroup {
    Designators,
    Values,
    Footprints,
}

/// What a row does to the schematic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackChange {
    /// Give the part, at its placement `sheet`, the board's designator.
    Designator {
        footprint: Uuid,
        sheet: InstancePath,
        units: Vec<Uuid>,
        from: String,
        to: String,
    },
    /// Give every unit of the part the board's value.
    Value {
        footprint: Uuid,
        sheet: InstancePath,
        units: Vec<Uuid>,
        reference: String,
        from: String,
        to: String,
    },
    /// The board has another footprint than the part binds; listed only.
    Footprint {
        footprint: Uuid,
        reference: String,
        board: String,
        schematic: String,
    },
}

/// Why a row cannot be ticked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackBlock {
    /// The part's schematic designator is empty or holds `?`.
    NeedsAnnotation,
    /// Another part carries the same schematic designator.
    DuplicateDesignator(String),
    /// The board's designator is empty or holds `?`.
    BoardNotAnnotated,
    /// The sheet is placed more than once, so a value would change every
    /// placement.
    SharedSheet,
    /// Changing a part's footprint from the board waits for #2248.
    FootprintFromBoard,
}

/// Something to know that is not a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackNote {
    UnreadableSheet(InstancePath),
    /// A footprint with no link to a part: nothing to carry back.
    Unlinked {
        reference: String,
    },
    /// A linked footprint whose part is not in the design any more.
    PartGone {
        reference: String,
    },
    /// The part's sheet or first unit could not be read back.
    PartNotRead {
        designator: String,
    },
    /// After the change `designator` would be on `count` parts.
    SharedDesignator {
        designator: String,
        count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackRow {
    pub key: BackKey,
    pub group: BackGroup,
    pub change: BackChange,
    /// Ticked now; always `false` while blocked.
    pub on: bool,
    pub blocked: Option<BackBlock>,
}

impl BackRow {
    fn new(key: BackKey, group: BackGroup, change: BackChange, blocked: Option<BackBlock>) -> Self {
        Self {
            key,
            group,
            change,
            on: blocked.is_none(),
            blocked,
        }
    }

    /// Ticked and not blocked.
    pub fn applies(&self) -> bool {
        self.on && self.blocked.is_none()
    }
}

/// The whole list, for the board at `revision`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BackChangeList {
    pub revision: u64,
    pub rows: Vec<BackRow>,
    pub notes: Vec<BackNote>,
}

impl BackChangeList {
    pub fn row(&self, key: &BackKey) -> Option<&BackRow> {
        self.rows.iter().find(|r| &r.key == key)
    }

    /// Tick or untick a row; a blocked row stays unticked.
    pub fn set_on(&mut self, key: &BackKey, on: bool) {
        if let Some(row) = self.rows.iter_mut().find(|r| &r.key == key) {
            row.on = on && row.blocked.is_none();
        }
    }

    pub fn set_group_on(&mut self, group: BackGroup, on: bool) {
        for row in self.rows.iter_mut().filter(|r| r.group == group) {
            row.on = on && row.blocked.is_none();
        }
    }

    pub fn group_ticks(&self, group: BackGroup) -> GroupTicks {
        let open = self
            .rows
            .iter()
            .filter(|r| r.group == group && r.blocked.is_none());
        let (on, open) = open.fold((0, 0), |(on, all), r| (on + usize::from(r.on), all + 1));
        GroupTicks { on, open }
    }

    /// Keep the ticks of `previous` (the list before a Refresh) on rows
    /// that still make the same change.
    pub fn keep_choices_from(&mut self, previous: &BackChangeList) {
        for row in &mut self.rows {
            if let Some(old) = previous.row(&row.key)
                && old.change == row.change
            {
                row.on = old.on && row.blocked.is_none();
            }
        }
    }

    /// The edits of the rows that apply, one per part placement.
    pub fn sheet_edits(&self) -> Vec<SheetEdit> {
        let mut edits: BTreeMap<(InstancePath, Vec<Uuid>), SheetEdit> = BTreeMap::new();
        for row in self.rows.iter().filter(|r| r.applies()) {
            let (sheet, units, designator, value) = match &row.change {
                BackChange::Designator {
                    sheet, units, to, ..
                } => (sheet, units, Some(to), None),
                BackChange::Value {
                    sheet, units, to, ..
                } => (sheet, units, None, Some(to)),
                BackChange::Footprint { .. } => continue,
            };
            let edit = edits
                .entry((sheet.clone(), units.clone()))
                .or_insert_with(|| SheetEdit {
                    placement: sheet.clone(),
                    units: units.clone(),
                    designator: None,
                    value: None,
                });
            if let Some(d) = designator {
                edit.designator = Some(d.clone());
            }
            if let Some(v) = value {
                edit.value = Some(v.clone());
            }
        }
        edits.into_values().collect()
    }
}

/// What one part placement gets on its sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetEdit {
    pub placement: InstancePath,
    pub units: Vec<Uuid>,
    pub designator: Option<String>,
    pub value: Option<String>,
}

/// The rows that bring the schematic in line with `input.board`.
pub fn diff_schematic(input: &BackInput) -> BackChangeList {
    let mut list = BackChangeList {
        revision: input.revision,
        rows: Vec::new(),
        notes: input
            .unreadable
            .iter()
            .cloned()
            .map(BackNote::UnreadableSheet)
            .collect(),
    };
    let duplicates = schematic_duplicates(input.parts);
    for fp in &input.board.footprints {
        let Some(link) = &fp.schematic_link else {
            list.notes.push(BackNote::Unlinked {
                reference: fp.reference.clone(),
            });
            continue;
        };
        let part = input.parts.iter().find(|p| {
            p.eco.part.sheet == link.sheet
                && link.units.iter().any(|u| p.eco.part.units.contains(u))
        });
        match part {
            Some(part) if part.eco.read => footprint_rows(&mut list, fp, part, &duplicates),
            Some(part) => list.notes.push(BackNote::PartNotRead {
                designator: part.eco.part.designator.clone(),
            }),
            None if input.unreadable.iter().any(|u| is_under(&link.sheet, u)) => {}
            None => list.notes.push(BackNote::PartGone {
                reference: fp.reference.clone(),
            }),
        }
    }
    note_shared_designators(&mut list, input.parts);
    list.rows.sort_by_key(|r| r.group);
    list
}

/// Designators carried by more than one annotated part.
fn schematic_duplicates(parts: &[BackPart]) -> HashSet<String> {
    let mut count: HashMap<&str, usize> = HashMap::new();
    for p in parts.iter().filter(|p| !p.eco.part.needs_annotation) {
        *count.entry(p.eco.part.designator.as_str()).or_default() += 1;
    }
    count
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(d, _)| d.to_string())
        .collect()
}

fn not_annotated(designator: &str) -> bool {
    designator.trim().is_empty() || designator.contains('?')
}

fn footprint_rows(
    list: &mut BackChangeList,
    fp: &Footprint,
    part: &BackPart,
    duplicates: &HashSet<String>,
) {
    let design = &part.eco.part;
    let (sheet, units) = (design.sheet.clone(), design.units.clone());
    if fp.reference != design.designator {
        let blocked = if design.needs_annotation {
            Some(BackBlock::NeedsAnnotation)
        } else if duplicates.contains(&design.designator) {
            Some(BackBlock::DuplicateDesignator(design.designator.clone()))
        } else if not_annotated(&fp.reference) {
            Some(BackBlock::BoardNotAnnotated)
        } else {
            None
        };
        list.rows.push(BackRow::new(
            BackKey::Designator(fp.uuid),
            BackGroup::Designators,
            BackChange::Designator {
                footprint: fp.uuid,
                sheet: sheet.clone(),
                units: units.clone(),
                from: design.designator.clone(),
                to: fp.reference.clone(),
            },
            blocked,
        ));
    }
    if fp.value != part.eco.value {
        list.rows.push(BackRow::new(
            BackKey::Value(fp.uuid),
            BackGroup::Values,
            BackChange::Value {
                footprint: fp.uuid,
                sheet,
                units,
                reference: fp.reference.clone(),
                from: part.eco.value.clone(),
                to: fp.value.clone(),
            },
            part.shared_sheet.then_some(BackBlock::SharedSheet),
        ));
    }
    if let Ok(resolved) = &part.eco.footprint
        && super::diff::change_reason(fp, resolved) == Some(super::ChangeReason::Footprint)
    {
        list.rows.push(BackRow::new(
            BackKey::Footprint(fp.uuid),
            BackGroup::Footprints,
            BackChange::Footprint {
                footprint: fp.uuid,
                reference: fp.reference.clone(),
                board: fp
                    .library_ref
                    .as_ref()
                    .map_or_else(|| fp.footprint_id.clone(), ToString::to_string),
                schematic: resolved.source.to_string(),
            },
            Some(BackBlock::FootprintFromBoard),
        ));
    }
}

/// A note for each designator the ticked rows would leave on several parts.
fn note_shared_designators(list: &mut BackChangeList, parts: &[BackPart]) {
    let mut after: BTreeMap<String, usize> = BTreeMap::new();
    let renamed: HashMap<(&InstancePath, &[Uuid]), &str> = list
        .rows
        .iter()
        .filter(|r| r.applies())
        .filter_map(|r| match &r.change {
            BackChange::Designator {
                sheet, units, to, ..
            } => Some(((sheet, units.as_slice()), to.as_str())),
            _ => None,
        })
        .collect();
    for p in parts.iter().filter(|p| !p.eco.part.needs_annotation) {
        let key = (&p.eco.part.sheet, p.eco.part.units.as_slice());
        let designator = renamed
            .get(&key)
            .copied()
            .unwrap_or(p.eco.part.designator.as_str());
        *after.entry(designator.to_string()).or_default() += 1;
    }
    let targets: HashSet<&str> = renamed.values().copied().collect();
    for (designator, count) in after {
        if count > 1 && targets.contains(designator.as_str()) {
            list.notes
                .push(BackNote::SharedDesignator { designator, count });
        }
    }
}

/// The commands that apply `edits` to `sheet`, a file whose placements
/// they all name. `single`: the file is placed once, so a designator goes in
/// the symbol's `reference` rather than a per-placement entry.
pub fn sheet_commands(sheet: &SchematicSheet, edits: &[SheetEdit], single: bool) -> Vec<Command> {
    let mut commands = Vec::new();
    for edit in edits {
        for unit in &edit.units {
            if !sheet.symbols.iter().any(|s| s.uuid == *unit) {
                continue;
            }
            if let Some(designator) = &edit.designator {
                commands.extend(designator_commands(
                    sheet,
                    *unit,
                    Some(&edit.placement),
                    single,
                    designator.clone(),
                ));
            }
            if let Some(value) = &edit.value {
                commands.push(Command::UpdateText {
                    target: TextTarget::SymbolValue(*unit),
                    value: value.clone(),
                });
            }
        }
    }
    commands
}

#[cfg(test)]
#[path = "back_tests.rs"]
mod tests;
