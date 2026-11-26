//! Rule: NetClassConflict, project-wide (#2656).
//!
//! The per-sheet rule (`rules/net_class_conflict.rs`) cannot see a directive
//! on another sheet, such as a Net Parameters flag giving global `VBUS` class
//! POWER on one sheet and SIGNAL on another. This pass reads the stitched
//! project netlist (`signex_net::project_net_class_conflicts`), whose ranking
//! is the one that gives the exported net its class.
//!
//! One rule, two entry points: for every directive that classes a project
//! net this pass owns the finding on that net
//! ([`ProjectClassFindings::replaces`]), and the caller drops the sheet's own
//! `NetClassConflict` finding on it; the per-sheet rule remains for a net the
//! project netlist does not carry, for a sheet the project roots never reach
//! and for a sheet checked on its own.

use std::collections::HashSet;

use signex_net::{ProjectGraph, SheetKey};
use signex_types::project::ChannelNaming;
use signex_types::schematic::{Point, SchematicSheet};
use uuid::Uuid;

use crate::context::no_erc_directive_at;
use crate::engine::directives_suppress;
use crate::rules::{conflict_finding, conflict_net};
use crate::{RuleId, RuleKind, Violation};

/// What the project pass found.
#[derive(Debug, Default)]
pub struct ProjectClassFindings {
    /// Each finding, with the key of the sheet of the overridden directive.
    pub findings: Vec<(SheetKey, Violation)>,
    /// Every `(sheet, directive)` the project netlist classes a net with.
    pub covered: HashSet<(SheetKey, Uuid)>,
    /// Every `(sheet, net)` of a sheet's own netlist that is part of a
    /// project net (`signex_net::ProjectClassConflicts::carried`).
    pub carried: HashSet<(SheetKey, String)>,
}

impl ProjectClassFindings {
    /// Whether this pass's answer replaces `v`, a per-sheet finding of the
    /// sheet `key`: a `NetClassConflict` on a covered directive and a
    /// carried net. Any other finding the sheet keeps.
    pub fn replaces(&self, key: &SheetKey, v: &Violation) -> bool {
        if v.builtin_kind() != Some(RuleKind::NetClassConflict) {
            return false;
        }
        let on_covered = v
            .primary
            .is_some_and(|p| self.covered.contains(&(key.clone(), p.uuid)));
        let on_carried = conflict_net(v.tag.as_deref())
            .is_some_and(|net| self.carried.contains(&(key.clone(), net.to_string())));
        on_covered && on_carried
    }
}

/// Every net class conflict of the project, filed under the sheet of each
/// overridden directive. A finding a No-ERC directive, a Compile Mask or a
/// Blanket excluding the rule silences on its sheet is dropped, as the
/// per-sheet run drops it.
pub fn project_net_class_conflicts_named(
    graph: &ProjectGraph,
    naming: &ChannelNaming,
) -> ProjectClassFindings {
    let found = signex_net::project_net_class_conflicts(graph, naming);
    let mut findings = Vec::new();
    for conflict in &found.conflicts {
        let winner = &conflict.winner;
        for lost in &conflict.overridden {
            let Some(sheet) = graph.sheets.get(&lost.sheet) else {
                continue;
            };
            if is_silenced(sheet, lost.assignment.at) {
                continue;
            }
            let winner_sheet = (winner.sheet != lost.sheet).then(|| winner.sheet.as_str());
            let d = conflict_finding(
                &conflict.net_name,
                &winner.assignment,
                winner_sheet,
                &lost.assignment,
            );
            findings.push((lost.sheet.clone(), Violation::from(d)));
        }
    }
    ProjectClassFindings {
        findings,
        covered: found.judged,
        carried: found.carried,
    }
}

/// Whether a `NetClassConflict` finding at `at` is silenced on `sheet`.
fn is_silenced(sheet: &SchematicSheet, at: Point) -> bool {
    let rule = RuleId::builtin(RuleKind::NetClassConflict);
    let no_erc = no_erc_directive_at(
        sheet
            .no_erc_directives
            .iter()
            .map(|d| (&d.position, d.rules.as_slice())),
        &at,
        rule.as_str(),
    );
    no_erc || directives_suppress(&sheet.directives, at, rule.as_str())
}

#[cfg(test)]
#[path = "project_class_conflicts_tests.rs"]
mod tests;
