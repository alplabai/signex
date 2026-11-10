//! What an edit does to the names of the project's nets (#2199), judged on
//! the stitched project by net identity (the stitcher's own groups, tracked
//! per sheet placement). Each edit is judged by what it changes: a child's
//! local labels already behind a hierarchical label before it play no part.
//! Rule (#2199 comment 6761836527): `docs/CLI.md`, "Wire intent"; net scope:
//! `docs/NET_NAMING.md`.

use std::collections::{BTreeSet, HashMap, HashSet};

use signex_types::schematic::{InstancePath, LabelType};
use uuid::Uuid;

use super::members::{NetMember, NetMemberKind, NetMembers};
use super::{ProjectGraph, SheetKey, Want, stitch_project};
use crate::join_rule::{NetNames, needs_allow_merge};

/// One named element of a sheet placement: a label or a power port.
type NamedId = (SheetKey, InstancePath, Uuid);

/// A net of the project that carries at least one name.
struct NamedNet {
    ids: Vec<NamedId>,
    names: NetNames,
}

fn id_of(member: &NetMember) -> NamedId {
    (member.sheet.clone(), member.occurrence.clone(), member.id)
}

/// A local name on one sheet placement. Local labels of one text on one
/// placement are one net, so a later copy of a hidden name is hidden too.
type LocalName = (SheetKey, InstancePath, String);

fn local_name_of(member: &NetMember) -> LocalName {
    let (sheet, occurrence, _) = id_of(member);
    (sheet, occurrence, member.text.clone())
}

/// The local net label names of `nets` that sit on the same sheet placement
/// as a hierarchical label of their net: names that stay inside the child.
// shortcut: per placement, not per sheet net; a placement whose second sheet
// net reaches the net only through a global label would have its local labels
// dropped too. Exact needs the sheet-net root on `NetMember`; upgrade if that
// case matters.
fn behind_port(nets: &[NetMembers]) -> HashSet<LocalName> {
    let mut out = HashSet::new();
    for net in nets {
        let ported: HashSet<(&SheetKey, &InstancePath)> = net
            .members
            .iter()
            .filter(|m| m.kind == NetMemberKind::Label(LabelType::Hierarchical))
            .map(|m| (&m.sheet, &m.occurrence))
            .collect();
        let local = net.members.iter().filter(|m| {
            m.kind == NetMemberKind::Label(LabelType::Net)
                && ported.contains(&(&m.sheet, &m.occurrence))
        });
        out.extend(local.map(local_name_of));
    }
    out
}

/// The named nets of a stitch; the local names in `hidden` play no part, a
/// later copy on the same placement included.
fn named_nets(nets: Vec<NetMembers>, hidden: &HashSet<LocalName>) -> Vec<NamedNet> {
    nets.into_iter()
        .filter_map(|net| {
            let mut out = NamedNet {
                ids: Vec::new(),
                names: NetNames::default(),
            };
            for member in net.members.iter().filter(|m| !m.text.is_empty()) {
                let side = match member.kind {
                    NetMemberKind::Label(LabelType::Hierarchical) => &mut out.names.aliases,
                    NetMemberKind::Label(LabelType::Net)
                        if hidden.contains(&local_name_of(member)) =>
                    {
                        continue;
                    }
                    NetMemberKind::Label(_) | NetMemberKind::PowerPort => &mut out.names.own,
                    _ => continue,
                };
                side.insert(member.text.clone());
                out.ids.push(id_of(member));
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
/// a name a net already carries, alias included, is no change. A local label
/// of another sheet than `edited` that was behind a hierarchical label BEFORE
/// the edit plays no part on either side; on the edited sheet it stays a name,
/// and one the edit itself brings behind a port still counts.
pub fn project_name_changes(
    before: &ProjectGraph,
    after: &ProjectGraph,
    edited: &SheetKey,
) -> Vec<ProjectNetChange> {
    let before = stitch_project(before, Want::Members).members;
    let after = stitch_project(after, Want::Members).members;
    let mut hidden = behind_port(&before);
    hidden.retain(|(sheet, ..)| sheet != edited);
    let before = named_nets(before, &hidden);
    let mut owner: HashMap<&NamedId, usize> = HashMap::new();
    for (index, net) in before.iter().enumerate() {
        owner.extend(net.ids.iter().map(|id| (id, index)));
    }
    named_nets(after, &hidden)
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
                needs_allow_merge: needs_allow_merge(&nets, &net.names.own),
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
