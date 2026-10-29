//! Part completeness rules (#1286): footprint, value, MPN and annotation.
//!
//! Each is judged per physical part, not per placed unit: a multi-unit part
//! passes when any of its units carries the field, the way the BOM merges
//! units. Power ports and `#` markers are not parts and are skipped. A
//! symbol is judged under the designator each placement shows (#829).

use signex_types::schematic::{PartKey, SelectedKind, group_into_parts, normalized_lib_id};

use crate::context::{ErcContext, ErcSymbol};
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// One placement of a symbol under the designator it shows there.
type Placed<'a> = (&'a ErcSymbol, &'a str);

/// The parts on this sheet, each as its placed units. A symbol placed more
/// than once (a reused sheet) counts once per designator (#829).
fn parts(ctx: &ErcContext) -> Vec<Vec<Placed<'_>>> {
    let eligible: Vec<Placed> = ctx
        .symbols
        .iter()
        .filter(|s| !s.is_power)
        .flat_map(|s| s.placement_designators().into_iter().map(move |d| (s, d)))
        .filter(|(_, d)| !d.trim().starts_with('#'))
        .collect();
    group_into_parts(eligible.iter().map(|(s, d)| PartKey {
        reference: d.trim(),
        lib_id: normalized_lib_id(&s.lib_id),
        unit: s.unit,
    }))
    .into_iter()
    .map(|members| members.into_iter().map(|i| eligible[i]).collect())
    .collect()
}

/// `true` for a designator that still needs annotating: empty or `R?`.
pub(crate) fn is_unannotated(reference: &str) -> bool {
    let r = reference.trim();
    r.is_empty() || r.ends_with('?')
}

fn display(designator: &str) -> &str {
    match designator.trim() {
        "" => "(unnamed)",
        r => r,
    }
}

/// One finding per part none of whose units passes `has`, at its first unit.
fn report_parts(
    ctx: &ErcContext,
    kind: RuleKind,
    applies: impl Fn(Placed) -> bool,
    has: impl Fn(Placed) -> bool,
    message: impl Fn(&str) -> String,
    out: &mut Vec<Diagnostic>,
) {
    for part in parts(ctx) {
        if !part.iter().any(|p| applies(*p)) || part.iter().any(|p| has(*p)) {
            continue;
        }
        let (first, designator) = part[0];
        out.push(
            Diagnostic::new(kind, message(display(designator)), first.position)
                .with_primary(sel(first.uuid, SelectedKind::Symbol)),
        );
    }
}

pub(crate) fn part_without_footprint(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    report_parts(
        ctx,
        RuleKind::PartWithoutFootprint,
        |(s, _)| s.on_board,
        |(s, _)| !s.footprint.trim().is_empty(),
        |r| format!("Part {r} has no footprint"),
        out,
    );
}

pub(crate) fn part_without_value(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    report_parts(
        ctx,
        RuleKind::PartWithoutValue,
        |_| true,
        |(s, _)| !s.value.trim().is_empty(),
        |r| format!("Part {r} has no value"),
        out,
    );
}

/// The MPN is read from the same fields the BOM reads it from.
pub(crate) fn part_without_mpn(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let has_mpn = |(s, _): Placed| {
        s.attrs.iter().any(|(key, value)| {
            signex_types::schematic::MPN_FIELDS
                .iter()
                .any(|f| key.eq_ignore_ascii_case(f))
                && !value.trim().is_empty()
        })
    };
    report_parts(
        ctx,
        RuleKind::PartWithoutMpn,
        |_| true,
        has_mpn,
        |r| format!("Part {r} has no manufacturer part number (MPN)"),
        out,
    );
}

pub(crate) fn unannotated_designator(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    report_parts(
        ctx,
        RuleKind::UnannotatedDesignator,
        |(_, d)| is_unannotated(d),
        |(_, d)| !is_unannotated(d),
        |r| format!("Designator '{r}' is not annotated"),
        out,
    );
}
