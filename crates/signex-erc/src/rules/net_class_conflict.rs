//! Rule: NetClassConflict (#2656).
//!
//! A Net Parameters directive and a Net Blanket, or two of one kind, can give
//! one net different classes. The netlist settles it by rank
//! (`signex_net::class_assignments`: Net Parameters before Net Blankets, each
//! in sheet order); this rule says so instead of letting the losing class
//! vanish. One finding per overridden directive and net, located at the
//! overridden directive (a Net Blanket's centre, so a Blanket that excludes
//! this rule silences it), with the winning directive as its peer. Conflicts
//! between sheets are found by the project pass (`project_class_conflicts.rs`).

use signex_net::ClassAssignment;
use signex_types::schematic::{DirectiveKind, SelectedKind};

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// The UI name of a class-setting directive.
fn directive_name(kind: DirectiveKind) -> &'static str {
    match kind {
        DirectiveKind::Blanket => "Net Blanket",
        DirectiveKind::NetParameters => "Net Parameters directive",
        DirectiveKind::CompileMask => "Compile Mask",
        DirectiveKind::DiffPair => "Differential Pair directive",
    }
}

/// The finding for `lost`, overridden on net `net_name` by `winner`. The
/// winner is on another sheet when `winner_sheet` names it: the message says
/// which, and it is no peer, since a uuid selects on the finding's own canvas
/// only.
pub(crate) fn conflict_finding(
    net_name: &str,
    winner: &ClassAssignment,
    winner_sheet: Option<&str>,
    lost: &ClassAssignment,
) -> Diagnostic {
    let from = match winner_sheet {
        Some(sheet) => format!("a {} on sheet {sheet}", directive_name(winner.kind)),
        None => format!("a {}", directive_name(winner.kind)),
    };
    let d = Diagnostic::new(
        RuleKind::NetClassConflict,
        format!(
            "Net {net_name} takes class {} from {from}; class {} of this {} is overridden",
            winner.class,
            lost.class,
            directive_name(lost.kind),
        ),
        lost.at,
    )
    .with_primary(sel(lost.directive, SelectedKind::Directive))
    // One Blanket can be overridden on several nets by the same peer; the
    // net keeps those findings apart.
    .tagged(format!("net:{net_name}"));
    match winner_sheet {
        Some(_) => d,
        None => d.with_peer(sel(winner.directive, SelectedKind::Directive)),
    }
}

pub(crate) fn net_class_conflict(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for conflict in &ctx.class_conflicts {
        for lost in &conflict.overridden {
            out.push(conflict_finding(
                &conflict.net_name,
                &conflict.winner,
                None,
                lost,
            ));
        }
    }
}

#[cfg(test)]
#[path = "tests/net_class_conflict_tests.rs"]
mod tests;
