//! Part completeness rules (#1286): footprint, value, MPN and annotation.
//!
//! Each is judged per physical part, not per placed unit: a multi-unit part
//! passes when any of its units carries the field, the way the BOM merges
//! units. Power ports and `#` markers are not parts and are skipped.

use signex_types::schematic::{PartKey, SelectedKind, group_into_parts, normalized_lib_id};

use crate::context::{ErcContext, ErcSymbol};
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// The parts on this sheet, each as its placed units.
fn parts(ctx: &ErcContext) -> Vec<Vec<&ErcSymbol>> {
    let eligible: Vec<&ErcSymbol> = ctx
        .symbols
        .iter()
        .filter(|s| !s.is_power && !s.reference.trim().starts_with('#'))
        .collect();
    group_into_parts(eligible.iter().map(|s| PartKey {
        reference: s.reference.trim(),
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

fn display_ref(s: &ErcSymbol) -> &str {
    match s.reference.trim() {
        "" => "(unnamed)",
        r => r,
    }
}

/// One finding per part none of whose units passes `has`, at its first unit.
fn report_parts(
    ctx: &ErcContext,
    kind: RuleKind,
    applies: impl Fn(&ErcSymbol) -> bool,
    has: impl Fn(&ErcSymbol) -> bool,
    message: impl Fn(&str) -> String,
    out: &mut Vec<Diagnostic>,
) {
    for part in parts(ctx) {
        if !part.iter().any(|s| applies(s)) || part.iter().any(|s| has(s)) {
            continue;
        }
        let first = part[0];
        out.push(
            Diagnostic::new(kind, message(display_ref(first)), first.position)
                .with_primary(sel(first.uuid, SelectedKind::Symbol)),
        );
    }
}

pub(crate) fn part_without_footprint(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    report_parts(
        ctx,
        RuleKind::PartWithoutFootprint,
        |s| s.on_board,
        |s| !s.footprint.trim().is_empty(),
        |r| format!("Part {r} has no footprint"),
        out,
    );
}

pub(crate) fn part_without_value(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    report_parts(
        ctx,
        RuleKind::PartWithoutValue,
        |_| true,
        |s| !s.value.trim().is_empty(),
        |r| format!("Part {r} has no value"),
        out,
    );
}

/// The MPN is read from the same fields the BOM reads it from.
pub(crate) fn part_without_mpn(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let has_mpn = |s: &ErcSymbol| {
        s.attrs.iter().any(|(key, value)| {
            signex_bom::MPN_FIELDS
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
        |s| is_unannotated(&s.reference),
        |s| !is_unannotated(&s.reference),
        |r| format!("Designator '{r}' is not annotated"),
        out,
    );
}
