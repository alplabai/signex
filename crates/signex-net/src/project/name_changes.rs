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
//! sheet-entry link) is an alias: the same signal as seen from the child.

use std::collections::{BTreeSet, HashMap};

use signex_types::schematic::{InstancePath, LabelType};
use uuid::Uuid;

use super::members::NetMemberKind;
use super::{ProjectGraph, SheetKey, Want, stitch_project};

/// One named element of a sheet placement: a label or a power port.
type NamedId = (SheetKey, InstancePath, Uuid);

/// A net of the project that carries at least one name.
struct NamedNet {
    ids: Vec<NamedId>,
    own: BTreeSet<String>,
    aliases: BTreeSet<String>,
}

impl NamedNet {
    fn all_names(&self) -> BTreeSet<String> {
        self.own.union(&self.aliases).cloned().collect()
    }
}

fn named_nets(graph: &ProjectGraph) -> Vec<NamedNet> {
    stitch_project(graph, Want::Members)
        .members
        .into_iter()
        .filter_map(|net| {
            let mut out = NamedNet {
                ids: Vec::new(),
                own: BTreeSet::new(),
                aliases: BTreeSet::new(),
            };
            for member in net.members {
                let side = match member.kind {
                    NetMemberKind::Label(LabelType::Hierarchical) => Some(&mut out.aliases),
                    NetMemberKind::Label(_) | NetMemberKind::PowerPort => Some(&mut out.own),
                    _ => None,
                };
                if let Some(side) = side.filter(|_| !member.text.is_empty()) {
                    side.insert(member.text);
                    out.ids.push((member.sheet, member.occurrence, member.id));
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
/// more named nets, or gained a name. It `needs_allow_merge` when
/// * two absorbed nets have own names and those differ, or the net gained an
///   own name that the absorbed nets' own names (all one set) did not carry; or
/// * no absorbed net has an own name and two have different aliases (a
///   sheet-entry net named `SDA` by one child and `SCL` by another).
///
/// Everything else joins freely: nets with the same names (two placements of
/// one child sheet joined on the same entry share a bus), unnamed nets, a net
/// with no own name gaining one or joining an own-named one (the parent's
/// `I2C_SDA` wired to the entry whose child label is `SDA`).
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
            let nets: Vec<&NamedNet> = absorbed.iter().map(|&i| &before[i]).collect();
            let unchanged = matches!(
                nets.as_slice(),
                [only] if only.own == net.own && only.aliases == net.aliases
            );
            if nets.is_empty() || unchanged {
                return None;
            }
            Some(ProjectNetChange {
                needs_allow_merge: needs_allow_merge(&nets, &net),
                names: net.all_names().into_iter().collect(),
                aliases: net.aliases.difference(&net.own).cloned().collect(),
            })
        })
        .collect()
}

fn needs_allow_merge(absorbed: &[&NamedNet], after: &NamedNet) -> bool {
    // Nets with own names are compared by those; when none has any, by their
    // aliases (a net with no own name may gain one freely).
    let owned: Vec<&BTreeSet<String>> = absorbed
        .iter()
        .map(|net| &net.own)
        .filter(|own| !own.is_empty())
        .collect();
    // The net after carries every absorbed own name, so it is inside the first
    // net's own names only when all the absorbed ones are the same set and
    // nothing was gained.
    if let [first, ..] = owned.as_slice() {
        return !after.own.is_subset(first);
    }
    let mut aliased = absorbed.iter().map(|net| &net.aliases);
    aliased
        .next()
        .is_some_and(|first| aliased.any(|other| other != first))
}
