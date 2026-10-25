//! Rule: NetMultipleNames.
//!
//! One net can carry several names: a local label `SDA` and a global label
//! `I2C_SDA` on one wire, or power ports `+3V3` and `VDD` joined by a wire. The
//! netlist shows exactly one of them (`signex_net::best_net_name_of`) and keeps
//! the others as aliases for joining; this rule tells the user it happened and
//! which name won. The naming rule is described in `docs/NET_NAMING.md`.
//!
//! Overlap with `NetLabelConflict`: that rule already reports a net whose
//! Net / Global / Power labels disagree, so such a net is skipped here. This
//! rule covers what it cannot see: a Hierarchical label, or a power-port
//! symbol, whose name differs from the net's other names. Single sheet only:
//! names that meet through a sheet pin are compared by the project pass
//! (`project_names.rs`), which the app runs for every sheet the project roots
//! reach and which replaces this rule's findings there wherever it emits one.
//! This one covers a sheet checked on its own.

use std::collections::{HashMap, HashSet};

use signex_net::{best_net_name_of, is_power_flag, pt_key};
use signex_types::schematic::{LabelType, Point, SelectedKind};

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

use super::label_conflict_groups;

/// One place a net gets a name from.
struct NameSource<'a> {
    name: &'a str,
    /// `None` for a power-port pin, which names the net like a Power label.
    label_type: Option<LabelType>,
    at: Point,
    uuid: uuid::Uuid,
    kind: SelectedKind,
}

pub(crate) fn net_multiple_names(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let (mut conn, _) =
        crate::context::merged_connectivity(&ctx.wires, &ctx.labels, &ctx.junctions, &ctx.symbols);
    let port_symbols = || {
        ctx.symbols
            .iter()
            .filter(|s| s.is_power && !s.value.is_empty() && !is_power_flag(&s.lib_id))
    };

    let mut by_root: HashMap<(i64, i64), Vec<NameSource>> = HashMap::new();
    for l in ctx.labels.iter().filter(|l| !l.text.is_empty()) {
        by_root
            .entry(conn.root_of(&l.position))
            .or_default()
            .push(NameSource {
                name: &l.text,
                label_type: Some(l.label_type),
                at: l.position,
                uuid: l.uuid,
                kind: SelectedKind::Label,
            });
    }
    for s in port_symbols() {
        for pin in &s.pins {
            by_root
                .entry(conn.root_of(&pin.world_pos))
                .or_default()
                .push(NameSource {
                    name: &s.value,
                    label_type: None,
                    at: pin.world_pos,
                    uuid: s.uuid,
                    kind: SelectedKind::Symbol,
                });
        }
    }

    // Nets `NetLabelConflict` already reports, by the position of any label.
    let reported: HashSet<(i64, i64)> = label_conflict_groups(ctx)
        .iter()
        .flatten()
        .map(|l| pt_key(&l.position))
        .collect();

    let mut nets: Vec<Vec<NameSource>> = by_root.into_values().collect();
    for sources in &mut nets {
        sources.sort_by(|a, b| (a.name, pt_key(&a.at)).cmp(&(b.name, pt_key(&b.at))));
    }
    nets.sort_by_key(|s| (s[0].name, pt_key(&s[0].at)));

    for sources in nets {
        let mut names: Vec<&str> = sources.iter().map(|s| s.name).collect();
        names.dedup();
        // A No-ERC directive on any of the net's names suppresses it, as in
        // the project pass (`project_names.rs`).
        let skipped = sources
            .iter()
            .any(|s| reported.contains(&pt_key(&s.at)) || ctx.has_no_erc_directive_at(&s.at));
        if names.len() < 2 || skipped {
            continue;
        }
        let power: Vec<&str> = sources
            .iter()
            .filter(|s| s.label_type.is_none())
            .map(|s| s.name)
            .collect();
        let labelled = sources
            .iter()
            .filter_map(|s| s.label_type.map(|t| (t, s.name)));
        let Some((_, winner)) = best_net_name_of(labelled, &power) else {
            continue;
        };
        let first = sources
            .iter()
            .find(|s| s.name == winner)
            .unwrap_or(&sources[0]);
        let peer = sources
            .iter()
            .find(|s| s.name != winner)
            .unwrap_or(&sources[0]);
        let listing: Vec<String> = sources
            .iter()
            .map(|s| format!("'{}' at ({:.2}, {:.2})", s.name, s.at.x, s.at.y))
            .collect();
        out.push(
            Diagnostic::new(
                RuleKind::NetMultipleNames,
                format!(
                    "Net has multiple names; on this sheet, '{winner}' wins, but the exported name may be qualified or overridden across sheets: {}",
                    listing.join("; "),
                ),
                first.at,
            )
            .with_primary(sel(first.uuid, first.kind))
            .with_peer(sel(peer.uuid, peer.kind)),
        );
    }
}
