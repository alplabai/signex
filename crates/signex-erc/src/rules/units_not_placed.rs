//! Rule: UnitsNotPlaced (#1286). A multi-unit part (a quad op-amp) whose
//! placed units do not cover every unit its library symbol defines.
//!
//! One decision, two entry points: [`units_not_placed`] judges one sheet run
//! on its own, and `project_units_not_placed` judges every sheet of a project
//! at once, since units are often split across sheets. Both group units with
//! the shared `group_into_parts` and decide in [`missing_units`].

use signex_types::schematic::{PartKey, Point, SelectedKind, group_into_parts, normalized_lib_id};
use uuid::Uuid;

use super::completeness::is_unannotated;
use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// One placed unit, as both entry points see it.
pub(crate) struct PlacedUnit<'a> {
    pub reference: &'a str,
    pub lib_id: &'a str,
    pub unit: u32,
    pub unit_count: u32,
    pub uuid: Uuid,
    pub at: Point,
}

impl PlacedUnit<'_> {
    /// Whether a designator applies: not a power port, `#` marker or `R?`.
    pub fn is_eligible(reference: &str, is_power: bool) -> bool {
        !is_power && !reference.trim().starts_with('#') && !is_unannotated(reference)
    }
}

/// The most units a part may define before the rule refuses to enumerate
/// them. Unit numbers come from a file, so an absurd count (`u32::MAX`) must
/// never size a loop or a list; such a part is reported once as suspect.
pub(crate) const MAX_UNITS_PER_PART: u32 = 256;

/// How many missing unit numbers a message lists before "(+N more)".
const LISTED_UNITS: usize = 8;

/// What is wrong with one part's units.
pub(crate) enum UnitGap {
    /// These unit numbers are placed nowhere, ascending.
    Missing(Vec<u32>),
    /// The library symbol claims more than [`MAX_UNITS_PER_PART`] units.
    TooManyUnits(u32),
}

/// For each part with a gap: the index (into `units`) of its first placed
/// unit and the gap. Work is bounded by [`MAX_UNITS_PER_PART`] per part.
pub(crate) fn missing_units(units: &[PlacedUnit]) -> Vec<(usize, UnitGap)> {
    let parts = group_into_parts(units.iter().map(|u| PartKey {
        reference: u.reference.trim(),
        lib_id: normalized_lib_id(u.lib_id),
        unit: u.unit,
    }));
    parts
        .into_iter()
        .filter_map(|members| {
            let count = members.iter().map(|&m| units[m].unit_count).max()?;
            if count > MAX_UNITS_PER_PART {
                return Some((members[0], UnitGap::TooManyUnits(count)));
            }
            let missing: Vec<u32> = (1..=count)
                .filter(|n| !members.iter().any(|&m| units[m].unit == *n))
                .collect();
            (!missing.is_empty()).then(|| (members[0], UnitGap::Missing(missing)))
        })
        .collect()
}

/// The finding for a part whose first placed unit is `first`.
pub(crate) fn finding(first: &PlacedUnit, gap: &UnitGap) -> Diagnostic {
    let reference = first.reference.trim();
    let message = match gap {
        UnitGap::Missing(missing) => {
            let mut list: Vec<String> = missing
                .iter()
                .take(LISTED_UNITS)
                .map(u32::to_string)
                .collect();
            if missing.len() > LISTED_UNITS {
                list.push(format!("... (+{} more)", missing.len() - LISTED_UNITS));
            }
            format!(
                "Part {reference} has {} of {} units not placed: unit {}",
                missing.len(),
                first.unit_count,
                list.join(", ")
            )
        }
        UnitGap::TooManyUnits(count) => format!(
            "Part {reference}: its library symbol claims {count} units, more than \
             {MAX_UNITS_PER_PART}; the symbol is likely corrupt and its units were not checked"
        ),
    };
    Diagnostic::new(RuleKind::UnitsNotPlaced, message, first.at)
        .with_primary(sel(first.uuid, SelectedKind::Symbol))
}

pub(crate) fn units_not_placed(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let units: Vec<PlacedUnit> = ctx
        .symbols
        .iter()
        .filter(|s| PlacedUnit::is_eligible(&s.reference, s.is_power))
        .map(|s| PlacedUnit {
            reference: &s.reference,
            lib_id: &s.lib_id,
            unit: s.unit,
            unit_count: s.unit_count,
            uuid: s.uuid,
            at: s.position,
        })
        .collect();
    for (first, gap) in missing_units(&units) {
        out.push(finding(&units[first], &gap));
    }
}
