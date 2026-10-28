//! Buses across sheets (#1049): one bus drawn as two separate runs on a sheet.
//!
//! `bus_bit_width_mismatch` and `bus_name_conflict` compare the names on one
//! connected run. Two runs that never touch still join, member by member,
//! whenever their names share a [`join_key`](signex_types::schematic::BusName::join_key):
//! a member's net is named by its member label, and a name is one net on a
//! sheet. So `D[0..7]` on one run and `D[0..3]` on another are one bus at two
//! widths, and `D0..D3` of the two merge. This rule reports that.

use std::collections::BTreeMap;

use signex_net::BusAnalysis;
use signex_types::schematic::{LabelType, SelectedKind, parse_bus_name};

use crate::context::{ErcContext, ErcLabel};
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// One bus-name label with the run it sits on.
struct RunName<'a> {
    run: usize,
    label: &'a ErcLabel,
    width: usize,
    members: &'a std::collections::HashSet<String>,
}

/// Two runs on this sheet naming the same bus with member sets that overlap
/// but differ. Reports each bus once, against the first pair found. Runs whose
/// members are disjoint merge nothing and are left alone, so a wide bus split
/// as `D[0..7]` and `D[8..15]` stays clean. Power labels are skipped; every
/// other label kind is at least sheet-local.
pub(crate) fn bus_runs_width_mismatch(
    ctx: &ErcContext,
    bus: &BusAnalysis,
    out: &mut Vec<Diagnostic>,
) {
    let mut by_key: BTreeMap<String, Vec<RunName>> = BTreeMap::new();
    for (run, group) in bus.groups.iter().enumerate() {
        for name in &group.names {
            let label = &ctx.labels[name.label];
            if label.label_type == LabelType::Power {
                continue;
            }
            let Ok(parsed) = parse_bus_name(&label.text) else {
                continue;
            };
            by_key.entry(parsed.join_key()).or_default().push(RunName {
                run,
                label,
                width: name.width,
                members: &name.members,
            });
        }
    }
    for names in by_key.values() {
        let pair = names.iter().enumerate().find_map(|(i, a)| {
            names[i + 1..]
                .iter()
                .find(|b| {
                    a.run != b.run && a.members != b.members && !a.members.is_disjoint(b.members)
                })
                .map(|b| (a, b))
        });
        if let Some((first, other)) = pair {
            out.push(diagnostic(first, other));
        }
    }
}

fn diagnostic(first: &RunName, other: &RunName) -> Diagnostic {
    let (kind, message) = if first.width == other.width {
        (
            RuleKind::BusNameConflict,
            format!(
                "Bus names disagree across runs: '{}' and '{}' each carry {} signals, but \
                 different ones; the signals they share join",
                first.label.text, other.label.text, first.width,
            ),
        )
    } else {
        (
            RuleKind::BusBitWidthMismatch,
            format!(
                "Bus width mismatch across runs: '{}' carries {} signals, '{}' carries {}; \
                 only the shared signals join",
                first.label.text, first.width, other.label.text, other.width,
            ),
        )
    };
    Diagnostic::new(kind, message, other.label.position)
        .with_primary(sel(other.label.uuid, SelectedKind::Label))
        .with_peer(sel(first.label.uuid, SelectedKind::Label))
}
