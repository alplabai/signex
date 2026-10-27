//! Re-prefix to standard (#1315): the pure decision of which placed parts
//! get which prefix. No IO and no app state: the sheets, the lookup of the
//! libraries' rows and the locks come in, a `uuid -> prefix` map and the list
//! of what was skipped (and why) come out.
//!
//! A part is the unit of decision (units of one part share a designator), so
//! one blocked unit blocks the whole part and a part never ends up split.
//!
//! # What counts as hand-edited
//!
//! There is no "edited by the user" flag in the file, so a part is treated as
//! automatic only when its current alphabetic prefix is one the system could
//! have issued: the library symbol's own letters, the letters ANY standard
//! gives the row's class, or the row's valid override. A prefix that none of
//! those would issue is skipped as hand-edited. This detects exactly that and
//! no more: a hand-typed prefix that happens to equal one of those letters is
//! indistinguishable from an issued one and IS rewritten (a `V1` typed on a
//! diode under IEEE 315 becomes `D`). The confirm list shows every rewrite
//! before it is applied.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

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
    /// No row link, and the rows binding its library symbol disagree on the
    /// class or the prefix override, so the symbol does not name one.
    RowAmbiguous,
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
            Self::RowAmbiguous => {
                "whose library symbol is bound by rows that disagree on the prefix"
            }
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

/// One part to re-issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PartChange {
    /// The part's designator now (`D5`, `U?`).
    pub reference: String,
    pub old_prefix: String,
    pub new_prefix: String,
    /// Every unit of the part, on every sheet.
    pub units: Vec<Uuid>,
}

#[derive(Debug, Default)]
pub(super) struct ReprefixPlan {
    pub changes: Vec<PartChange>,
    /// Distinct sheets holding a unit of a changed part.
    pub sheets_changed: usize,
    /// Parts already on the standard's prefix.
    pub parts_conforming: usize,
    pub skipped: Vec<SkippedPart>,
    /// Rows whose `designator_prefix` was refused: `(row, raw value, why)`.
    pub invalid_overrides: BTreeMap<(String, String), InvalidOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InvalidOverride {
    pub internal_pn: String,
    pub raw: String,
    pub why: &'static str,
}

impl ReprefixPlan {
    /// Every unit of every changed part -> its new prefix.
    pub(super) fn targets(&self) -> HashMap<Uuid, String> {
        self.changes
            .iter()
            .flat_map(|c| c.units.iter().map(|u| (*u, c.new_prefix.clone())))
            .collect()
    }
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
    let mut sheets_changed: BTreeSet<usize> = BTreeSet::new();
    for units in members.values() {
        note_invalid_override(input, units, &mut plan);
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
                let first = &input.sheets[units[0].0].symbols[units[0].1];
                sheets_changed.extend(units.iter().map(|&(sheet, _)| sheet));
                plan.changes.push(PartChange {
                    reference: first.reference.trim().to_owned(),
                    old_prefix: alphabetic_prefix(&first.reference).to_owned(),
                    new_prefix: prefix,
                    units: units
                        .iter()
                        .map(|&(sheet, idx)| input.sheets[sheet].symbols[idx].uuid)
                        .collect(),
                });
            }
        }
    }
    plan.sheets_changed = sheets_changed.len();
    plan
}

/// Record a refused `designator_prefix` of the part's row, once per row.
fn note_invalid_override(input: &PlanInput<'_>, units: &[Member], plan: &mut ReprefixPlan) {
    let Ok(facts) = linked_row(input, units) else {
        return;
    };
    let Some(raw) = facts.prefix_override.as_deref() else {
        return;
    };
    if let Err(why) = normalize_prefix_override(raw) {
        plan.invalid_overrides
            .entry((facts.internal_pn.clone(), raw.to_owned()))
            .or_insert_with(|| InvalidOverride {
                internal_pn: facts.internal_pn.clone(),
                raw: raw.to_owned(),
                why,
            });
    }
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

/// The row facts of a part. A part placed in this session carries its row
/// (`library_id` + `row_id`). The `.snxsch` file does not store that link, so a
/// reopened part has only its `lib_id` (`"<library>:<symbol>"`): the rows that
/// bind that symbol stand in for it when they all agree on the class and the
/// override, and the part is skipped as ambiguous when they do not.
fn linked_row<'a>(input: &'a PlanInput<'_>, units: &[Member]) -> Result<&'a RowFacts, SkipReason> {
    let first = &input.sheets[units[0].0].symbols[units[0].1];
    let unlinked = |s: &Symbol| s.library_id.is_none() || s.row_id.is_none();
    if units
        .iter()
        .all(|&(sheet, idx)| unlinked(&input.sheets[sheet].symbols[idx]))
    {
        return row_by_symbol(input, &first.lib_id);
    }
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

/// The rows binding the symbol a `lib_id` names, when they agree.
fn row_by_symbol<'a>(input: &'a PlanInput<'_>, lib_id: &str) -> Result<&'a RowFacts, SkipReason> {
    let parsed = lib_id.trim().split_once(':').and_then(|(library, symbol)| {
        Some((
            Uuid::parse_str(library).ok()?,
            Uuid::parse_str(symbol).ok()?,
        ))
    });
    let Some(symbol) = parsed else {
        return Err(SkipReason::NoRowLink);
    };
    let mut candidates = input
        .lookup
        .libraries
        .values()
        .flat_map(HashMap::values)
        .filter(|facts| facts.symbol == symbol);
    let Some(first) = candidates.next() else {
        return Err(if input.lookup.libraries.contains_key(&symbol.0) {
            SkipReason::RowMissing
        } else {
            SkipReason::LibraryNotMounted
        });
    };
    let agree = |other: &RowFacts| {
        other.class == first.class && other.prefix_override == first.prefix_override
    };
    if candidates.all(agree) {
        Ok(first)
    } else {
        Err(SkipReason::RowAmbiguous)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::SymbolInstance;

    const LIB: Uuid = Uuid::from_u128(1);
    const ROW: Uuid = Uuid::from_u128(2);

    fn facts(class: &str, prefix_override: Option<&str>) -> RowFacts {
        RowFacts {
            internal_pn: "PN-1".into(),
            symbol: (Uuid::from_u128(3), Uuid::from_u128(4)),
            class: class.into(),
            prefix_override: prefix_override.map(str::to_owned),
        }
    }

    fn lookup_of(facts: RowFacts) -> ReprefixLookup {
        ReprefixLookup {
            libraries: HashMap::from([(LIB, HashMap::from([(ROW, facts)]))]),
            ..Default::default()
        }
    }

    fn linked(reference: &str, unit: u32) -> Symbol {
        Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "L:D".into(),
            reference: reference.into(),
            unit,
            library_id: Some(LIB),
            row_id: Some(ROW),
            ..Default::default()
        }
    }

    /// `D?` library symbol under `L:D`.
    fn lib_symbols() -> HashMap<String, LibSymbol> {
        let mut symbol = signex_library::primitive::Symbol::empty("D");
        symbol.designator = "D?".into();
        HashMap::from([("L:D".to_owned(), symbol.to_lib_symbol("L:D"))])
    }

    /// Plan over `sheets` (symbols, read-only) under `standard`.
    fn plan(
        standard: DesignatorStandard,
        lookup: &ReprefixLookup,
        sheets: &[(&[Symbol], bool)],
        with_lib_symbols: bool,
    ) -> ReprefixPlan {
        let symbols = if with_lib_symbols {
            lib_symbols()
        } else {
            HashMap::new()
        };
        let sheets: Vec<PlanSheet<'_>> = sheets
            .iter()
            .map(|&(syms, read_only)| PlanSheet {
                symbols: syms,
                lib_symbols: &symbols,
                read_only,
            })
            .collect();
        plan_reprefix(&PlanInput {
            standard,
            sheets: &sheets,
            lookup,
            locked: &HashSet::new(),
            order: AnnotateOrder::default(),
        })
    }

    fn only_change(plan: &ReprefixPlan) -> &PartChange {
        assert_eq!(plan.changes.len(), 1, "{plan:?}");
        &plan.changes[0]
    }

    fn only_skip(plan: &ReprefixPlan) -> SkipReason {
        assert_eq!(plan.skipped.len(), 1, "{plan:?}");
        plan.skipped[0].reason
    }

    #[test]
    fn a_letter_from_another_standard_counts_as_issued_and_is_rewritten() {
        // `V` is DIN's diode letter; under IEEE 315 the part goes back to `D`.
        let symbols = [linked("V1", 1)];
        let p = plan(
            DesignatorStandard::Ieee315,
            &lookup_of(facts("diode", None)),
            &[(&symbols, false)],
            true,
        );
        assert_eq!(only_change(&p).new_prefix, "D");
    }

    #[test]
    fn a_prefix_nothing_would_issue_is_hand_edited() {
        let symbols = [linked("X9", 1)];
        let p = plan(
            DesignatorStandard::Din40719_2,
            &lookup_of(facts("diode", None)),
            &[(&symbols, false)],
            true,
        );
        assert_eq!(only_skip(&p), SkipReason::UserEdited);
    }

    #[test]
    fn one_blocked_unit_blocks_a_part_split_across_sheets() {
        let lookup = lookup_of(facts("diode", None));
        let mut locked = linked("D1", 2);
        locked.locked = true;
        let mut unlinked = linked("D1", 2);
        unlinked.row_id = None;
        for (blocked, reason, read_only) in [
            (locked, SkipReason::Locked, false),
            (unlinked, SkipReason::NoRowLink, false),
            (linked("D1", 2), SkipReason::ReadOnlySheet, true),
        ] {
            let first = [linked("D1", 1)];
            let second = [blocked];
            let p = plan(
                DesignatorStandard::Din40719_2,
                &lookup,
                &[(&first, false), (&second, read_only)],
                true,
            );
            assert_eq!(only_skip(&p), reason);
            assert!(p.changes.is_empty(), "the part must not be split: {p:?}");
        }
    }

    #[test]
    fn a_row_that_left_the_library_is_listed() {
        let symbols = [linked("D1", 1)];
        let lookup = ReprefixLookup {
            libraries: HashMap::from([(LIB, HashMap::new())]),
            ..Default::default()
        };
        let p = plan(
            DesignatorStandard::Din40719_2,
            &lookup,
            &[(&symbols, false)],
            true,
        );
        assert_eq!(only_skip(&p), SkipReason::RowMissing);
    }

    #[test]
    fn a_per_placement_designator_blocks_the_part() {
        let mut symbol = linked("D1", 1);
        symbol.instances.push(SymbolInstance {
            path: "/".into(),
            reference: "D7".into(),
            ..Default::default()
        });
        let symbols = [symbol];
        let p = plan(
            DesignatorStandard::Din40719_2,
            &lookup_of(facts("diode", None)),
            &[(&symbols, false)],
            true,
        );
        assert_eq!(only_skip(&p), SkipReason::RetargetedByInstance);
    }

    #[test]
    fn a_valid_override_is_the_target() {
        let symbols = [linked("D1", 1)];
        let p = plan(
            DesignatorStandard::Din40719_2,
            &lookup_of(facts("diode", Some("zd"))),
            &[(&symbols, false)],
            true,
        );
        assert_eq!(only_change(&p).new_prefix, "ZD");
    }

    #[test]
    fn an_invalid_override_falls_back_and_is_collected() {
        let symbols = [linked("D1", 1)];
        let p = plan(
            DesignatorStandard::Din40719_2,
            &lookup_of(facts("diode", Some("R1"))),
            &[(&symbols, false)],
            true,
        );
        assert_eq!(only_change(&p).new_prefix, "V");
        let invalid = &p.invalid_overrides[&("PN-1".to_owned(), "R1".to_owned())];
        assert_eq!(
            (invalid.raw.as_str(), invalid.internal_pn.as_str()),
            ("R1", "PN-1")
        );
    }

    #[test]
    fn the_generic_class_keeps_the_symbol_letters() {
        let symbols = [linked("D1", 1)];
        let p = plan(
            DesignatorStandard::Din40719_2,
            &lookup_of(facts("generic", None)),
            &[(&symbols, false)],
            true,
        );
        assert!(p.changes.is_empty() && p.skipped.is_empty(), "{p:?}");
        assert_eq!(p.parts_conforming, 1);
    }

    #[test]
    fn a_missing_lib_symbol_still_plans_from_the_class_table() {
        let symbols = [linked("D1", 1)];
        let p = plan(
            DesignatorStandard::Din40719_2,
            &lookup_of(facts("diode", None)),
            &[(&symbols, false)],
            false,
        );
        assert_eq!(only_change(&p).new_prefix, "V");
    }

    #[test]
    fn a_lowercase_reference_is_compared_case_insensitively() {
        let symbols = [linked("d1", 1)];
        let p = plan(
            DesignatorStandard::Din40719_2,
            &lookup_of(facts("diode", None)),
            &[(&symbols, false)],
            true,
        );
        assert_eq!(only_change(&p).new_prefix, "V");
    }

    fn by_lib_id(reference: &str) -> Symbol {
        Symbol {
            lib_id: format!("{}:{}", Uuid::from_u128(3), Uuid::from_u128(4)),
            library_id: None,
            row_id: None,
            ..linked(reference, 1)
        }
    }

    #[test]
    fn a_part_without_a_row_link_is_matched_through_its_lib_id() {
        // What a reopened sheet gives: the file stores no row link.
        let symbols = [by_lib_id("D1")];
        let p = plan(
            DesignatorStandard::Din40719_2,
            &lookup_of(facts("diode", None)),
            &[(&symbols, false)],
            false,
        );
        assert_eq!(only_change(&p).new_prefix, "V");
    }

    #[test]
    fn rows_that_disagree_on_a_symbol_make_the_part_ambiguous() {
        let symbols = [by_lib_id("D1")];
        let mut lookup = lookup_of(facts("diode", None));
        lookup
            .libraries
            .get_mut(&LIB)
            .expect("library")
            .insert(Uuid::from_u128(9), facts("led", None));
        let p = plan(
            DesignatorStandard::Din40719_2,
            &lookup,
            &[(&symbols, false)],
            false,
        );
        assert_eq!(only_skip(&p), SkipReason::RowAmbiguous);
    }

    #[test]
    fn units_of_one_part_on_two_sheets_change_together() {
        let (a, b) = ([linked("D1", 1)], [linked("D1", 2)]);
        let p = plan(
            DesignatorStandard::Din40719_2,
            &lookup_of(facts("diode", None)),
            &[(&a, false), (&b, false)],
            true,
        );
        assert_eq!(only_change(&p).units.len(), 2);
        assert_eq!(p.sheets_changed, 2);
    }
}
