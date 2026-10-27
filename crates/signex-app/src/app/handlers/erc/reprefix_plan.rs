//! Re-prefix to standard (#1315): the pure decision of which placed parts
//! get which prefix. No IO and no app state: the sheets, the lookup of the
//! libraries' rows and the locks come in, a `uuid -> prefix` map and the list
//! of what was skipped (and why) come out.
//!
//! A part is the unit of decision (units of one part share a designator), so
//! one blocked unit blocks the whole part and a part never ends up split.

use std::collections::{BTreeMap, HashMap, HashSet};

use signex_types::designator::{
    DesignatorStandard, alphabetic_prefix, class_prefix, normalize_prefix_override,
    placement_prefix, resolve_prefix,
};
use signex_types::schematic::{AnnotateOrder, LibSymbol, ProjectParts, Symbol};
use uuid::Uuid;

use crate::library::reprefix_lookup::{ReprefixLookup, RowFacts};

/// One sheet of the project, in the order the annotate pass walks them.
pub(super) struct PlanSheet<'a> {
    pub symbols: &'a [Symbol],
    pub lib_symbols: &'a HashMap<String, LibSymbol>,
    /// Written by a newer Signex: this build edits and saves nothing there.
    pub read_only: bool,
}

pub(super) struct PlanInput<'a> {
    pub standard: DesignatorStandard,
    pub sheets: &'a [PlanSheet<'a>],
    pub lookup: &'a ReprefixLookup,
    /// The Annotate dialog's locks. A symbol's own `locked` flag counts too.
    pub locked: &'a HashSet<Uuid>,
    pub order: AnnotateOrder,
}

/// Why a part was left alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SkipReason {
    /// A unit sits on a read-only sheet; moving the others would split it.
    ReadOnlySheet,
    /// A unit is locked (`Symbol.locked` or the Annotate dialog's lock).
    Locked,
    /// Not placed from a library row (`library_id` / `row_id` are unset).
    NoRowLink,
    /// Its library is not mounted, or could not be read.
    LibraryNotMounted,
    /// Its library is mounted but the row is gone.
    RowMissing,
    /// An instance entry gives some placement a different designator.
    RetargetedByInstance,
    /// The prefix is not one the system could have issued: the user edited it.
    UserEdited,
}

impl SkipReason {
    /// The Messages-panel heading of the group.
    pub(super) fn heading(self) -> &'static str {
        match self {
            Self::ReadOnlySheet => "on a read-only sheet",
            Self::Locked => "locked",
            Self::NoRowLink => "not placed from a library row",
            Self::LibraryNotMounted => "from a library that is not mounted",
            Self::RowMissing => "whose library row no longer exists",
            Self::RetargetedByInstance => "with a per-placement designator",
            Self::UserEdited => "with a hand-edited prefix",
        }
    }
}

#[derive(Debug)]
pub(super) struct SkippedPart {
    pub reason: SkipReason,
    /// The part's designator as shown to the user (`R5`, `U?`).
    pub reference: String,
}

#[derive(Debug, Default)]
pub(super) struct ReprefixPlan {
    /// Every unit of every part to re-issue -> its new prefix.
    pub targets: HashMap<Uuid, String>,
    pub parts_changed: usize,
    /// Parts already on the standard's prefix.
    pub parts_conforming: usize,
    pub skipped: Vec<SkippedPart>,
}

type Member = (usize, usize);

pub(super) fn plan_reprefix(input: &PlanInput<'_>) -> ReprefixPlan {
    let slices: Vec<&[Symbol]> = input.sheets.iter().map(|s| s.symbols).collect();
    let parts = ProjectParts::new(&slices, input.locked, input.order);
    let mut members: BTreeMap<usize, Vec<Member>> = BTreeMap::new();
    for (sheet, symbols) in slices.iter().enumerate() {
        for idx in 0..symbols.len() {
            if let Some(part) = parts.part(sheet, idx) {
                members.entry(part).or_default().push((sheet, idx));
            }
        }
    }
    let mut plan = ReprefixPlan::default();
    for units in members.values() {
        match decide(input, units) {
            Decision::Skip(reason) => {
                let (sheet, idx) = units[0];
                plan.skipped.push(SkippedPart {
                    reason,
                    reference: input.sheets[sheet].symbols[idx].reference.trim().to_owned(),
                });
            }
            Decision::Conforms => plan.parts_conforming += 1,
            Decision::Change(prefix) => {
                plan.parts_changed += 1;
                for &(sheet, idx) in units {
                    let uuid = input.sheets[sheet].symbols[idx].uuid;
                    plan.targets.insert(uuid, prefix.clone());
                }
            }
        }
    }
    plan
}

enum Decision {
    Skip(SkipReason),
    Conforms,
    Change(String),
}

fn decide(input: &PlanInput<'_>, units: &[Member]) -> Decision {
    let symbol_at = |&(sheet, idx): &Member| &input.sheets[sheet].symbols[idx];
    if units
        .iter()
        .any(|&(sheet, _)| input.sheets[sheet].read_only)
    {
        return Decision::Skip(SkipReason::ReadOnlySheet);
    }
    if units
        .iter()
        .map(symbol_at)
        .any(|s| s.locked || input.locked.contains(&s.uuid))
    {
        return Decision::Skip(SkipReason::Locked);
    }
    let first = symbol_at(&units[0]);
    let facts = match linked_row(input, units) {
        Ok(facts) => facts,
        Err(reason) => return Decision::Skip(reason),
    };
    if units
        .iter()
        .map(symbol_at)
        .any(has_per_placement_designator)
    {
        return Decision::Skip(SkipReason::RetargetedByInstance);
    }
    let lib_symbol = input.sheets[units[0].0].lib_symbols.get(&first.lib_id);
    let symbol_letters = lib_symbol.map_or("", |l| l.reference.as_str());
    let current = alphabetic_prefix(&first.reference).to_ascii_uppercase();
    if !is_system_issued(&current, symbol_letters, facts) {
        return Decision::Skip(SkipReason::UserEdited);
    }
    let target = resolve_prefix(
        Some(input.standard),
        Some(&facts.class),
        facts.prefix_override.as_deref(),
        if symbol_letters.is_empty() {
            &first.reference
        } else {
            symbol_letters
        },
    );
    if target.is_empty() || target.starts_with('#') || target == current {
        return Decision::Conforms;
    }
    Decision::Change(target)
}

/// The row facts of a part: every unit must link to a row, and the row must
/// be in a library the lookup read.
fn linked_row<'a>(input: &'a PlanInput<'_>, units: &[Member]) -> Result<&'a RowFacts, SkipReason> {
    let first = &input.sheets[units[0].0].symbols[units[0].1];
    let unlinked = |s: &Symbol| s.library_id.is_none() || s.row_id.is_none();
    if units
        .iter()
        .any(|&(sheet, idx)| unlinked(&input.sheets[sheet].symbols[idx]))
    {
        return Err(SkipReason::NoRowLink);
    }
    let (Some(library), Some(row)) = (first.library_id, first.row_id) else {
        return Err(SkipReason::NoRowLink);
    };
    input
        .lookup
        .libraries
        .get(&library)
        .ok_or(SkipReason::LibraryNotMounted)?
        .get(&row)
        .ok_or(SkipReason::RowMissing)
}

/// An instance entry that names a designator other than `reference`: the
/// engine rewrites only `reference`, which such a placement does not show.
fn has_per_placement_designator(symbol: &Symbol) -> bool {
    symbol
        .valid_instance_entries()
        .any(|(_, designator)| designator != symbol.reference.trim())
}

/// Could the system itself have issued `current` for this part? The legacy
/// prefix of the library symbol, any standard's letters for the row's class,
/// or the row's valid override. Anything else was typed by a user, and is
/// never rewritten: there is no flag for it in the file.
fn is_system_issued(current: &str, symbol_letters: &str, facts: &RowFacts) -> bool {
    if !symbol_letters.is_empty() && placement_prefix(symbol_letters) == current {
        return true;
    }
    if DesignatorStandard::ALL
        .iter()
        .any(|&s| class_prefix(s, &facts.class) == Some(current))
    {
        return true;
    }
    facts
        .prefix_override
        .as_deref()
        .and_then(|raw| normalize_prefix_override(raw).ok())
        .is_some_and(|p| p == current)
}
