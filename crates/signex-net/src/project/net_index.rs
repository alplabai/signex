//! Queries over the stitched project's net members: partners across sheets,
//! the members to step through, the net under a point, and the hover facts.
//!
//! Built once per stitch ([`build_project_netlist_indexed`]) and read many
//! times; nothing here re-derives connectivity.
//!
//! [`build_project_netlist_indexed`]: super::build_project_netlist_indexed

use std::collections::{HashMap, HashSet};

use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::schematic::Point;
use uuid::Uuid;

use super::SheetKey;
use super::members::{NetMember, NetMemberKind, NetMembers};

/// The element a navigation or highlight started from: stable across a
/// rebuild of the index, unlike a net's position in it.
pub type NetAnchor = (SheetKey, Uuid);

/// A net's pins, for the hover card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverPin {
    pub reference: String,
    pub pin: String,
    pub sheet: SheetKey,
}

/// What the hover insight shows for a net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetHover {
    pub name: String,
    /// The first net class that contains the net's name.
    pub class: Option<String>,
    pub pins: Vec<HoverPin>,
}

/// Every net of one stitched project with its members, indexed by element.
#[derive(Debug, Clone, Default)]
pub struct NetIndex {
    nets: Vec<NetMembers>,
    by_element: HashMap<NetAnchor, Vec<usize>>,
    by_name: HashMap<String, usize>,
}

impl NetIndex {
    pub(super) fn from_members(nets: Vec<NetMembers>) -> Self {
        let mut by_element: HashMap<NetAnchor, Vec<usize>> = HashMap::new();
        let mut by_name: HashMap<String, usize> = HashMap::new();
        for (i, net) in nets.iter().enumerate() {
            if !net.name.is_empty() {
                by_name.entry(net.name.clone()).or_insert(i);
            }
            for m in &net.members {
                let nets_of = by_element.entry((m.sheet.clone(), m.id)).or_default();
                if nets_of.last() != Some(&i) {
                    nets_of.push(i);
                }
            }
        }
        Self {
            nets,
            by_element,
            by_name,
        }
    }

    pub fn net_count(&self) -> usize {
        self.nets.len()
    }

    pub fn net(&self, index: usize) -> Option<&NetMembers> {
        self.nets.get(index)
    }

    /// The nets an element is on (more than one when its sheet is placed more
    /// than once).
    pub fn nets_of(&self, sheet: &SheetKey, id: Uuid) -> &[usize] {
        self.by_element
            .get(&(sheet.clone(), id))
            .map_or(&[], Vec::as_slice)
    }

    /// The net an earlier highlight named, after the index was rebuilt: the
    /// anchor's own net when it still exists, else the net of that name.
    pub fn resolve(&self, name: &str, anchor: &NetAnchor) -> Option<usize> {
        let own = self.nets_of(&anchor.0, anchor.1);
        own.iter()
            .copied()
            .find(|&i| self.nets[i].name == name)
            .or_else(|| self.by_name.get(name).copied())
            .or_else(|| own.first().copied())
    }

    /// The other ports on the nets `id` is on: Global and Hierarchical labels
    /// and sheet entries, one per element, in ring order starting after `id`
    /// itself, so taking the first again and again cycles through all of them.
    /// Empty when the net has no other port.
    pub fn partners(&self, sheet: &SheetKey, id: Uuid) -> Vec<&NetMember> {
        let mut seen: HashSet<NetAnchor> = HashSet::new();
        let ports: Vec<&NetMember> = self
            .nets_of(sheet, id)
            .iter()
            .flat_map(|&i| &self.nets[i].members)
            .filter(|m| m.is_port() && seen.insert((m.sheet.clone(), m.id)))
            .collect();
        let at = ports
            .iter()
            .position(|m| m.id == id && &m.sheet == sheet)
            .map_or(0, |p| p + 1);
        let (head, tail) = ports.split_at(at.min(ports.len()));
        tail.iter()
            .chain(head)
            .filter(|m| !(m.id == id && &m.sheet == sheet))
            .copied()
            .collect()
    }

    /// The members to step through on net `net`, one per element and pin, in
    /// walk order.
    pub fn steps(&self, net: usize) -> Vec<&NetMember> {
        let Some(net) = self.nets.get(net) else {
            return Vec::new();
        };
        let mut seen: HashSet<(SheetKey, Uuid, String)> = HashSet::new();
        net.members
            .iter()
            .filter(|m| m.is_steppable())
            .filter(|m| {
                let pin = match &m.kind {
                    NetMemberKind::Pin { pin, .. } => pin.clone(),
                    _ => String::new(),
                };
                seen.insert((m.sheet.clone(), m.id, pin))
            })
            .collect()
    }

    /// Every element of net `net` that is drawn, per sheet, for dimming
    /// everything else: members plus the sheet symbol a sheet entry sits on.
    pub fn highlight_elements(&self, net: usize) -> HashMap<SheetKey, HashSet<Uuid>> {
        let mut out: HashMap<SheetKey, HashSet<Uuid>> = HashMap::new();
        let Some(net) = self.nets.get(net) else {
            return out;
        };
        for m in &net.members {
            let set = out.entry(m.sheet.clone()).or_default();
            set.insert(m.id);
            if let NetMemberKind::SheetPin { sheet_symbol } = m.kind {
                set.insert(sheet_symbol);
            }
        }
        out
    }

    /// The pin of `sheet` nearest `at` within `tolerance_mm`, with its net.
    pub fn pin_near(
        &self,
        sheet: &SheetKey,
        at: Point,
        tolerance_mm: f64,
    ) -> Option<(usize, &NetMember)> {
        let dist = |m: &NetMember| ((m.at.x - at.x).powi(2) + (m.at.y - at.y).powi(2)).sqrt();
        self.nets
            .iter()
            .enumerate()
            .flat_map(|(i, n)| n.members.iter().map(move |m| (i, m)))
            .filter(|(_, m)| &m.sheet == sheet && matches!(m.kind, NetMemberKind::Pin { .. }))
            .map(|(i, m)| (dist(m), i, m))
            .filter(|(d, ..)| *d <= tolerance_mm)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, i, m)| (i, m))
    }

    /// The facts the hover card shows for net `net`.
    pub fn hover(&self, net: usize, classes: &[NetClass]) -> Option<NetHover> {
        let net = self.nets.get(net)?;
        let mut seen: HashSet<(SheetKey, Uuid, String)> = HashSet::new();
        let pins = net
            .members
            .iter()
            .filter_map(|m| match &m.kind {
                NetMemberKind::Pin { reference, pin }
                    if seen.insert((m.sheet.clone(), m.id, pin.clone())) =>
                {
                    Some(HoverPin {
                        reference: reference.clone(),
                        pin: pin.clone(),
                        sheet: m.sheet.clone(),
                    })
                }
                _ => None,
            })
            .collect();
        Some(NetHover {
            name: net.name.clone(),
            class: first_matching_class(classes, &net.name).map(|c| c.name.clone()),
            pins,
        })
    }
}
