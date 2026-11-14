//! How the Update PCB dialog shows its change list as a table (#2371): the
//! groups, each row's action, the summary counts, the filters and which
//! groups are folded. Pure functions of the list and the dialog's choices;
//! the list itself stays the engine's (`signex_engine::pcb::eco`).

use std::collections::BTreeMap;

use signex_engine::pcb::eco::{EcoChange, EcoChangeList, EcoGroup, EcoKey, EcoRow, GroupTicks};

/// A clean group with more rows than this starts folded.
const FOLD_PAST: usize = 10;

/// The groups the table shows, in this order. Each holds one or more of the
/// engine's row groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TableGroup {
    /// Footprints added, removed or changed, and their designators, values
    /// and links.
    Components,
    /// The board's net table.
    Nets,
    /// Which net each pad is on.
    PadNets,
}

impl TableGroup {
    pub const ALL: [Self; 3] = [Self::Components, Self::Nets, Self::PadNets];

    /// The table group of an engine row group.
    pub fn of(group: EcoGroup) -> Self {
        match group {
            EcoGroup::AddFootprints
            | EcoGroup::RemoveFootprints
            | EcoGroup::ChangeFootprints
            | EcoGroup::Designators
            | EcoGroup::Values
            | EcoGroup::Links => Self::Components,
            EcoGroup::Nets => Self::Nets,
            EcoGroup::PadNets => Self::PadNets,
        }
    }

    /// The engine row groups this one holds.
    pub fn engine_groups(self) -> &'static [EcoGroup] {
        match self {
            Self::Components => &[
                EcoGroup::AddFootprints,
                EcoGroup::RemoveFootprints,
                EcoGroup::ChangeFootprints,
                EcoGroup::Designators,
                EcoGroup::Values,
                EcoGroup::Links,
            ],
            Self::Nets => &[EcoGroup::Nets],
            Self::PadNets => &[EcoGroup::PadNets],
        }
    }
}

/// What a row does to the board, for its Action column and the summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowAction {
    Add,
    Change,
    Remove,
}

impl RowAction {
    /// The action of `change`. A pad that gets its first net is an add and
    /// one that loses its net a removal, so a new board lists only adds.
    pub fn of(change: &EcoChange) -> Self {
        match change {
            EcoChange::AddFootprint { .. } | EcoChange::AddNet { .. } => Self::Add,
            EcoChange::RemoveFootprint { .. } | EcoChange::RemoveNet { .. } => Self::Remove,
            EcoChange::PadNet { from: None, .. } => Self::Add,
            EcoChange::PadNet { to: None, .. } => Self::Remove,
            EcoChange::ChangeFootprint { .. }
            | EcoChange::SetDesignator { .. }
            | EcoChange::SetValue { .. }
            | EcoChange::Link { .. }
            | EcoChange::RenameNet { .. }
            | EcoChange::PadNet { .. } => Self::Change,
        }
    }
}

/// Which rows the summary chips narrow the table to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowFilter {
    Action(RowAction),
    /// Rows that cannot be ticked until something is fixed.
    Problems,
}

/// The counts the summary chips show.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub add: usize,
    pub change: usize,
    pub remove: usize,
    pub problems: usize,
}

impl Summary {
    pub fn of(list: &EcoChangeList) -> Self {
        let mut summary = Self::default();
        for row in &list.rows {
            match RowAction::of(&row.change) {
                RowAction::Add => summary.add += 1,
                RowAction::Change => summary.change += 1,
                RowAction::Remove => summary.remove += 1,
            }
            summary.problems += usize::from(row.blocked.is_some());
        }
        summary
    }
}

/// The dialog's view choices over the list: a chip filter, Show problems
/// only, the groups the user folded or opened, and the row Space ticks.
#[derive(Debug, Clone, Default)]
pub struct TableState {
    pub filter: Option<RowFilter>,
    pub problems_only: bool,
    /// Groups the user folded (`true`) or opened; the rest follow
    /// [`folded_by_default`].
    pub folds: BTreeMap<TableGroup, bool>,
    /// The row last clicked: Space ticks or unticks it.
    pub focused: Option<EcoKey>,
}

impl TableState {
    /// Whether `row` is listed under the current filters.
    pub fn shows(&self, row: &EcoRow) -> bool {
        let problem = row.blocked.is_some();
        if self.problems_only && !problem {
            return false;
        }
        match self.filter {
            None => true,
            Some(RowFilter::Problems) => problem,
            Some(RowFilter::Action(action)) => RowAction::of(&row.change) == action,
        }
    }

    /// Whether `group` is folded now.
    pub fn is_folded(&self, list: &EcoChangeList, group: TableGroup) -> bool {
        self.folds
            .get(&group)
            .copied()
            .unwrap_or_else(|| folded_by_default(list, group))
    }
}

/// A group with a problem starts open; a clean one with many rows starts
/// folded.
pub fn folded_by_default(list: &EcoChangeList, group: TableGroup) -> bool {
    let rows = rows_of(list, group);
    let problem = rows.clone().any(|r| r.blocked.is_some());
    !problem && rows.count() > FOLD_PAST
}

/// The rows of `group`, in list order.
pub fn rows_of(list: &EcoChangeList, group: TableGroup) -> impl Iterator<Item = &EcoRow> + Clone {
    list.rows
        .iter()
        .filter(move |r| TableGroup::of(r.group) == group)
}

/// The tick state of `group`: how many of its open (not blocked) rows are
/// ticked.
pub fn group_ticks(list: &EcoChangeList, group: TableGroup) -> GroupTicks {
    group
        .engine_groups()
        .iter()
        .map(|g| list.group_ticks(*g))
        .fold(GroupTicks { on: 0, open: 0 }, |sum, t| GroupTicks {
            on: sum.on + t.on,
            open: sum.open + t.open,
        })
}

/// The rows Execute applies: ticked, and every row they need ticked too.
pub fn applied_count(list: &EcoChangeList) -> usize {
    list.rows.iter().filter(|r| list.applies(r)).count()
}

/// Tick or untick every open row of `group`. Returns whether a footprint row
/// changed, after which the net rows are recomputed.
pub fn set_group_on(list: &mut EcoChangeList, group: TableGroup, on: bool) -> bool {
    let mut footprint_changed = false;
    for g in group.engine_groups() {
        footprint_changed |= list.set_group_on(*g, on);
    }
    footprint_changed
}

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
