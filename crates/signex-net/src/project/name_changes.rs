//! What an edit does to the names of the project's nets (#2199): a name that
//! reaches a net from another sheet is invisible to a one-sheet check, so the
//! name side of the headless intent checks is judged on the stitched project.
//! Nets are compared by identity, not by name text: the labels and power ports
//! that name a net are tracked per placement of their sheet, so two nets that
//! share a name set (a pair of globals elsewhere) are still two nets. The nets
//! are the stitcher's own groups; there is no second project connectivity
//! model. Net scope: `docs/NET_NAMING.md`.
//!
//! A net's own names come from net labels, global labels and power ports. A
//! name that reaches it only through a hierarchical label of a child (the
//! sheet-entry link) is an alias: the same signal as seen from the child. So
//! is a local net label on the same sheet placement as such a hierarchical
//! label: it never reaches the parent (decision 6759237104).

use std::collections::{BTreeSet, HashMap, HashSet};

use signex_types::schematic::{InstancePath, LabelType};
use uuid::Uuid;

use super::members::NetMemberKind;
use super::{ProjectGraph, SheetKey, Want, stitch_project};
use crate::join_rule::{NetNames, needs_allow_merge};

/// One named element of a sheet placement: a label or a power port.
type NamedId = (SheetKey, InstancePath, Uuid);

/// A net of the project that carries at least one name.
struct NamedNet {
    ids: Vec<NamedId>,
    names: NetNames,
}

fn named_nets(graph: &ProjectGraph) -> Vec<NamedNet> {
    stitch_project(graph, Want::Members)
        .members
        .into_iter()
        .filter_map(|net| {
            // The placements whose hierarchical label is on this net.
            // shortcut: per placement, not per sheet net; a placement whose
            // second sheet net reaches this one only through a global label
            // would count its local labels as aliases too. Exact needs the
            // sheet-net root on `NetMember`; upgrade if that case matters.
            let behind_port: HashSet<(&SheetKey, &InstancePath)> = net
                .members
                .iter()
                .filter(|m| m.kind == NetMemberKind::Label(LabelType::Hierarchical))
                .map(|m| (&m.sheet, &m.occurrence))
                .collect();
            let mut out = NamedNet {
                ids: Vec::new(),
                names: NetNames::default(),
            };
            for member in &net.members {
                let side = match member.kind {
                    NetMemberKind::Label(LabelType::Hierarchical) => &mut out.names.aliases,
                    NetMemberKind::Label(LabelType::Net)
                        if behind_port.contains(&(&member.sheet, &member.occurrence)) =>
                    {
                        &mut out.names.aliases
                    }
                    NetMemberKind::Label(_) | NetMemberKind::PowerPort => &mut out.names.own,
                    _ => continue,
                };
                if !member.text.is_empty() {
                    side.insert(member.text.clone());
                    let id = (member.sheet.clone(), member.occurrence.clone(), member.id);
                    out.ids.push(id);
                }
            }
            (!out.ids.is_empty()).then_some(out)
        })
        .collect()
}

/// A net of the project after an edit that is not simply a net as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectNetChange {
    /// Every name the net carries, own and alias, sorted.
    pub names: Vec<String>,
    /// The names it carries only through a child's hierarchical label.
    pub aliases: Vec<String>,
    /// Whether the edit needs `allow_merge`: see [`project_name_changes`].
    pub needs_allow_merge: bool,
}

/// The nets the edit turned into, for each net after it that absorbed two or
/// more named nets, or gained a name it did not carry. Whether one
/// `needs_allow_merge` is the shared rule (`join_rule::needs_allow_merge`):
/// nets are compared by their effective names (own names, else aliases), and
/// a name a net already carries, alias included, is no change.
pub fn project_name_changes(before: &ProjectGraph, after: &ProjectGraph) -> Vec<ProjectNetChange> {
    let before = named_nets(before);
    let mut owner: HashMap<&NamedId, usize> = HashMap::new();
    for (index, net) in before.iter().enumerate() {
        owner.extend(net.ids.iter().map(|id| (id, index)));
    }
    named_nets(after)
        .into_iter()
        .filter_map(|net| {
            let absorbed: BTreeSet<usize> = net
                .ids
                .iter()
                .filter_map(|id| owner.get(id))
                .copied()
                .collect();
            let nets: Vec<&NetNames> = absorbed.iter().map(|&i| &before[i].names).collect();
            let names = net.names.all();
            // One net that gained no name it did not carry is no change.
            let unchanged = matches!(nets.as_slice(), [only] if names.is_subset(&only.all()));
            if nets.is_empty() || unchanged {
                return None;
            }
            Some(ProjectNetChange {
                needs_allow_merge: needs_allow_merge(&nets, &names),
                names: names.into_iter().collect(),
                aliases: net
                    .names
                    .aliases
                    .difference(&net.names.own)
                    .cloned()
                    .collect(),
            })
        })
        .collect()
}
