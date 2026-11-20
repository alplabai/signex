//! How the Update PCB dialog shows its change list (#2371, #2533): the
//! groups, each row's action, the summary tiles, the filters, the problems
//! lifted above the list and which groups are folded. Pure functions of the
//! list and the dialog's choices; the list itself stays the engine's
//! (`signex_engine::pcb::eco`).

use std::collections::BTreeMap;

use signex_engine::pcb::eco::{
    EcoBlock, EcoChange, EcoChangeList, EcoGroup, EcoKey, EcoRow, GroupTicks,
};

/// A clean group with more rows than this starts folded.
const FOLD_PAST: usize = 10;

/// The groups the list shows, in this order. Each holds one or more of the
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

/// What a row does to the board, for its action pill and the tiles.
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

/// Which rows a summary tile narrows the list to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowFilter {
    Action(RowAction),
    /// Rows that cannot be ticked until something is fixed.
    Problems,
}

/// The counts the summary tiles show; a group header's split uses the
/// same counts over its own rows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub add: usize,
    pub change: usize,
    pub remove: usize,
    pub problems: usize,
}

impl Summary {
    pub fn of(list: &EcoChangeList) -> Self {
        Self::of_rows(&list.rows)
    }

    /// The counts of `rows` only.
    pub fn of_rows<'a>(rows: impl IntoIterator<Item = &'a EcoRow>) -> Self {
        let mut summary = Self::default();
        for row in rows {
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

/// The fix button a problem line offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemFix {
    /// Choose footprint...: the footprint picker for the part.
    ChooseFootprint,
    /// Annotate...: the Annotate dialog.
    Annotate,
    /// Nothing in the dialog fixes it; the reason says what to do.
    None,
}

impl ProblemFix {
    pub fn of(why: &EcoBlock) -> Self {
        match why {
            EcoBlock::MissingFootprint(_) => Self::ChooseFootprint,
            EcoBlock::NeedsAnnotation | EcoBlock::DuplicateDesignator(_) => Self::Annotate,
            EcoBlock::PadsCannotCarry { .. } | EcoBlock::Unplaceable(_) | EcoBlock::PartNotRead => {
                Self::None
            }
        }
    }
}

/// Whether a row's Cross Probe can show its part on the schematic: only a
/// row keyed by its part (an Add row) names the sheet placement.
pub fn can_cross_probe(key: &EcoKey) -> bool {
    matches!(key, EcoKey::AddFootprint { .. })
}

/// The dialog's view choices over the list: the tile filter, the filter
/// text, the groups the user folded or opened, the Notes group and the row
/// Space ticks.
#[derive(Debug, Clone, Default)]
pub struct TableState {
    pub filter: Option<RowFilter>,
    /// The filter field: a designator, net or pad (`R1.2`), any case.
    pub query: String,
    /// Groups the user folded (`true`) or opened; the rest follow
    /// [`folded_by_default`].
    pub folds: BTreeMap<TableGroup, bool>,
    /// The Notes group was opened; it starts folded.
    pub notes_open: bool,
    /// The row last clicked: Space ticks or unticks it.
    pub focused: Option<EcoKey>,
}

impl TableState {
    /// Whether `row` is listed under the tile filter and the filter text.
    pub fn shows(&self, row: &EcoRow) -> bool {
        let kind = match self.filter {
            None => true,
            Some(RowFilter::Problems) => row.blocked.is_some(),
            Some(RowFilter::Action(action)) => RowAction::of(&row.change) == action,
        };
        kind && matches_query(&row.change, &self.query)
    }

    /// Whether a tile or the filter text narrows the list.
    pub fn is_filtered(&self) -> bool {
        self.filter.is_some() || !self.query.trim().is_empty()
    }

    /// Whether `group` is folded now.
    pub fn is_folded(&self, list: &EcoChangeList, group: TableGroup) -> bool {
        self.folds
            .get(&group)
            .copied()
            .unwrap_or_else(|| folded_by_default(list, group))
    }
}

/// A group with a problem starts open. Pad nets start folded, and so does
/// any other clean group with many rows.
pub fn folded_by_default(list: &EcoChangeList, group: TableGroup) -> bool {
    let rows = rows_of(list, group);
    let problem = rows.clone().any(|r| r.blocked.is_some());
    !problem && (group == TableGroup::PadNets || rows.count() > FOLD_PAST)
}

/// The rows of `group`, in list order.
pub fn rows_of(list: &EcoChangeList, group: TableGroup) -> impl Iterator<Item = &EcoRow> + Clone {
    list.rows
        .iter()
        .filter(move |r| TableGroup::of(r.group) == group)
}

/// The rows the Problems card lifts above the list: every blocked row, in
/// list order. Each also stays in its group, unticked.
pub fn problems(list: &EcoChangeList) -> Vec<&EcoRow> {
    list.rows.iter().filter(|r| r.blocked.is_some()).collect()
}

/// What a row is about: a designator, a net, or a pad as `R1.2`.
pub fn object(change: &EcoChange) -> String {
    match change {
        EcoChange::AddFootprint { footprint } => footprint.reference.clone(),
        EcoChange::RemoveFootprint { reference, .. }
        | EcoChange::ChangeFootprint { reference, .. }
        | EcoChange::SetValue { reference, .. }
        | EcoChange::Link { reference, .. } => reference.clone(),
        EcoChange::SetDesignator { from, .. } => from.clone(),
        EcoChange::AddNet { name, .. } | EcoChange::RemoveNet { name, .. } => name.clone(),
        EcoChange::RenameNet { from, .. } => from.clone(),
        EcoChange::PadNet {
            reference,
            pad_number,
            ..
        } => format!("{reference}.{pad_number}"),
    }
}

/// Whether the filter text names the row: its object, a designator or net
/// it renames to, or a net its pad moves between. Empty text names every
/// row.
pub fn matches_query(change: &EcoChange, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    let hit = |s: &str| s.to_lowercase().contains(&query);
    if hit(&object(change)) {
        return true;
    }
    match change {
        EcoChange::SetDesignator { to, .. } | EcoChange::RenameNet { to, .. } => hit(to),
        EcoChange::PadNet { from, to, .. } => {
            [from, to].into_iter().flatten().any(|net| hit(&net.name))
        }
        _ => false,
    }
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
