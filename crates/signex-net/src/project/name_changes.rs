//! What an edit does to the names of the project's nets (#2199): a name that
//! reaches a net from another sheet is invisible to a one-sheet check, so the
//! name side of the headless intent checks is judged on the stitched project.
//! Nets are compared by identity, not by name text: the labels and power ports
//! that name a net are tracked per placement of their sheet, so two nets that
//! share a name set (two placements of one child, a pair of globals elsewhere)
//! are still two nets. The nets are the stitcher's own groups; there is no
//! second project connectivity model. Net scope: `docs/NET_NAMING.md`.

use std::collections::{BTreeSet, HashMap};

use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::members::NetMemberKind;
use super::{ProjectGraph, SheetKey, Want, stitch_project};

/// One named element of a sheet placement: a label or a power port.
type NamedId = (SheetKey, InstancePath, Uuid);

/// A net of the project that carries at least one name.
struct NamedNet {
    ids: Vec<NamedId>,
    names: BTreeSet<String>,
}

fn named_nets(graph: &ProjectGraph) -> Vec<NamedNet> {
    stitch_project(graph, Want::Members)
        .members
        .into_iter()
        .filter_map(|net| {
            let mut out = NamedNet {
                ids: Vec::new(),
                names: BTreeSet::new(),
            };
            for member in net.members {
                let names_the_net = matches!(
                    member.kind,
                    NetMemberKind::Label(_) | NetMemberKind::PowerPort
                );
                if names_the_net && !member.text.is_empty() {
                    out.ids.push((member.sheet, member.occurrence, member.id));
                    out.names.insert(member.text);
                }
            }
            (!out.ids.is_empty()).then_some(out)
        })
        .collect()
}

/// The nets the edit turned into: for each net after it, the named nets of
/// before that it absorbed. It is a change when it absorbed two or more named
/// nets, or one and gained a name that net did not carry; its names are
/// returned. A net with no name before may gain one or join another freely.
/// The caller refuses unless the edit said `allow_merge`.
pub fn project_name_changes(before: &ProjectGraph, after: &ProjectGraph) -> Vec<Vec<String>> {
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
            let changed = match absorbed.iter().next() {
                None => false,
                Some(&only) if absorbed.len() == 1 => !net.names.is_subset(&before[only].names),
                Some(_) => true,
            };
            changed.then(|| net.names.into_iter().collect())
        })
        .collect()
}
