//! Rule: NetClassConflict (#2656).
//!
//! A Net Parameters directive and a Net Blanket, or two of one kind, can give
//! one net different classes. The netlist settles it by rank
//! (`signex_net::class_assignments`: Net Parameters before Net Blankets, each
//! in sheet order); this rule says so instead of letting the losing class
//! vanish. One finding per overridden directive and net, located at the
//! overridden directive (a Net Blanket's centre, so a Blanket that excludes
//! this rule silences it), with the winning directive as its peer.

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

pub(crate) fn net_class_conflict(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for conflict in &ctx.class_conflicts {
        let winner = &conflict.winner;
        for lost in &conflict.overridden {
            out.push(
                Diagnostic::new(
                    RuleKind::NetClassConflict,
                    format!(
                        "Net {} takes class {} from a {}; class {} of this {} is overridden",
                        conflict.net_name,
                        winner.class,
                        directive_name(winner.kind),
                        lost.class,
                        directive_name(lost.kind),
                    ),
                    lost.at,
                )
                .with_primary(sel(lost.directive, SelectedKind::Directive))
                .with_peer(sel(winner.directive, SelectedKind::Directive))
                // One Blanket can be overridden on several nets by the same
                // peer; the net keeps those findings apart.
                .tagged(format!("net:{}", conflict.net_name)),
            );
        }
    }
}

#[cfg(test)]
#[path = "tests/net_class_conflict_tests.rs"]
mod tests;
