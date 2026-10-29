//! Queries over the stitched project's net members: partners across sheets,
//! the members to step through, the net under a point, and the hover facts.
//!
//! Built once per stitch ([`build_project_netlist_indexed`]) and read many
//! times; nothing here re-derives connectivity.
//!
//! [`build_project_netlist_indexed`]: super::build_project_netlist_indexed

use std::collections::{HashMap, HashSet};

use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::schematic::{InstancePath, Point};
use uuid::Uuid;

use super::SheetKey;
use super::members::{NetMember, NetMemberKind, NetMembers};

/// The element a navigation or highlight started from: stable across a
/// rebuild of the index, unlike a net's position in it.
/// An element of one sheet placement.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetAnchor {
    pub sheet: SheetKey,
    pub id: Uuid,
    pub occurrence: InstancePath,
    /// For a placed symbol, which pin: one symbol sits on a net per pin.
    pub pin: Option<String>,
}

impl NetAnchor {
    pub fn new(sheet: SheetKey, id: Uuid, occurrence: InstancePath) -> Self {
        Self {
            sheet,
            id,
            occurrence,
            pin: None,
        }
    }

    /// The anchor of `member`.
    fn of(member: &NetMember) -> Self {
        let pin = match &member.kind {
            NetMemberKind::Pin { pin, .. } => Some(pin.clone()),
            _ => None,
        };
        Self {
            sheet: member.sheet.clone(),
            id: member.id,
            occurrence: member.occurrence.clone(),
            pin,
        }
    }
}

/// A sheet element, whatever placement it is in.
type ElementKey = (SheetKey, Uuid);

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
    by_element: HashMap<ElementKey, Vec<usize>>,
    /// The placements the walk reaches each sheet through.
    placements: HashMap<SheetKey, Vec<InstancePath>>,
    by_name: HashMap<String, usize>,
}

impl NetIndex {
    pub(super) fn from_members(nets: Vec<NetMembers>, walk: Vec<(SheetKey, InstancePath)>) -> Self {
        let mut by_element: HashMap<ElementKey, Vec<usize>> = HashMap::new();
        let mut placements: HashMap<SheetKey, Vec<InstancePath>> = HashMap::new();
        for (sheet, placement) in walk {
            placements.entry(sheet).or_default().push(placement);
        }
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
            placements,
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

    /// The nets element `id` is on in the placement `occurrence`; every net
    /// it is on when the placement is unknown (`None`).
    pub fn nets_in(
        &self,
        sheet: &SheetKey,
        id: Uuid,
        occurrence: Option<&InstancePath>,
    ) -> Vec<usize> {
        let Some(occurrence) = occurrence else {
            return self.nets_of(sheet, id).to_vec();
        };
        self.nets_of(sheet, id)
            .iter()
            .copied()
            .filter(|&i| {
                self.nets[i]
                    .members
                    .iter()
                    .any(|m| m.id == id && &m.sheet == sheet && &m.occurrence == occurrence)
            })
            .collect()
    }

    /// The member for element `id` on `sheet` (its port member when it has one), in
    /// the placement `occurrence` when given.
    pub fn member(
        &self,
        sheet: &SheetKey,
        id: Uuid,
        occurrence: Option<&InstancePath>,
    ) -> Option<&NetMember> {
        let net = *self.nets_in(sheet, id, occurrence).first()?;
        let here = |m: &&NetMember| {
            m.id == id && &m.sheet == sheet && occurrence.is_none_or(|o| &m.occurrence == o)
        };
        let members = &self.nets[net].members;
        // A power symbol is both a pin and a power port: the port wins.
        members
            .iter()
            .filter(here)
            .find(|m| m.is_port())
            .or_else(|| members.iter().find(here))
    }

    /// An element of net `net` that a later index can find it by: the first
    /// member worth stepping to, else any.
    pub fn anchor_of(&self, net: usize) -> Option<NetAnchor> {
        let members = &self.nets.get(net)?.members;
        let pick = members
            .iter()
            .find(|m| m.is_steppable())
            .or_else(|| members.first())?;
        Some(NetAnchor::of(pick))
    }

    /// The anchor for element `id` of `sheet` as it sits on net `net`: the
    /// placement and, for a symbol, the pin the net has it at.
    pub fn anchor_in(&self, net: usize, sheet: &SheetKey, id: Uuid) -> Option<NetAnchor> {
        let m = self
            .nets
            .get(net)?
            .members
            .iter()
            .find(|m| m.id == id && &m.sheet == sheet)?;
        Some(NetAnchor::of(m))
    }

    /// The placements the walk reaches `sheet` through.
    pub fn placements(&self, sheet: &SheetKey) -> &[InstancePath] {
        self.placements.get(sheet).map_or(&[], Vec::as_slice)
    }

    /// The net an earlier highlight named, after the index was rebuilt: the
    /// anchor's own net while the anchor exists, however the net is named now.
    /// Only when the anchor is gone is the name used, and never a positional
    /// one ([`is_positional_name`]): it can belong to an unrelated net.
    pub fn resolve(&self, name: &str, anchor: &NetAnchor) -> Option<usize> {
        let mut own = self.nets_in(&anchor.sheet, anchor.id, Some(&anchor.occurrence));
        // A symbol is on a net per pin: only the anchor's own pin counts.
        if let Some(pin) = &anchor.pin {
            own.retain(|&i| {
                self.nets[i].members.iter().any(|m| {
                    m.id == anchor.id
                        && m.sheet == anchor.sheet
                        && m.occurrence == anchor.occurrence
                        && matches!(&m.kind, NetMemberKind::Pin { pin: p, .. } if p == pin)
                })
            });
        }
        if let Some(i) = own.iter().copied().find(|&i| self.nets[i].name == name) {
            return Some(i);
        }
        if let Some(&i) = own.first() {
            return Some(i);
        }
        if self.is_positional(name) {
            return None;
        }
        self.by_name.get(name).copied()
    }

    /// Whether `name` follows position rather than identity: an auto name, or
    /// the `<base>_<n>` a duplicate of the existing net `<base>` gets. After
    /// an edit it can belong to a different net.
    pub fn is_positional(&self, name: &str) -> bool {
        is_auto_name(name) || dedup_base(name).is_some_and(|base| self.by_name.contains_key(base))
    }

    /// The other ports on the nets `id` is on: Global and Hierarchical labels
    /// and sheet entries, one per element, in ring order starting after `id`
    /// itself, so taking the first again and again cycles through all of them.
    /// Empty when the net has no other port.
    pub fn partners(
        &self,
        sheet: &SheetKey,
        id: Uuid,
        occurrence: Option<&InstancePath>,
    ) -> Vec<&NetMember> {
        let mut seen: HashSet<ElementKey> = HashSet::new();
        let ports: Vec<&NetMember> = self
            .nets_in(sheet, id, occurrence)
            .into_iter()
            .flat_map(|i| &self.nets[i].members)
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

    /// [`Self::partners`] for a jump: ports on another sheet or placement
    /// first, so the first jump leaves the source's sheet. It holds the same
    /// ports as the plain ring, so a cursor that walks it visits each once.
    pub fn jump_ring(
        &self,
        sheet: &SheetKey,
        id: Uuid,
        occurrence: Option<&InstancePath>,
    ) -> Vec<&NetMember> {
        let mut ring = self.partners(sheet, id, occurrence);
        if let Some(source) = self.member(sheet, id, occurrence) {
            ring.sort_by_key(|m| m.sheet == source.sheet && m.occurrence == source.occurrence);
        }
        ring
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
    pub fn highlight_elements(
        &self,
        net: usize,
    ) -> HashMap<(SheetKey, InstancePath), HashSet<Uuid>> {
        let mut out: HashMap<(SheetKey, InstancePath), HashSet<Uuid>> = HashMap::new();
        let Some(net) = self.nets.get(net) else {
            return out;
        };
        for m in &net.members {
            let set = out
                .entry((m.sheet.clone(), m.occurrence.clone()))
                .or_default();
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
        occurrence: Option<&InstancePath>,
    ) -> Option<(usize, &NetMember)> {
        let dist = |m: &NetMember| ((m.at.x - at.x).powi(2) + (m.at.y - at.y).powi(2)).sqrt();
        self.nets
            .iter()
            .enumerate()
            .flat_map(|(i, n)| n.members.iter().map(move |m| (i, m)))
            .filter(|(_, m)| {
                &m.sheet == sheet
                    && matches!(m.kind, NetMemberKind::Pin { .. })
                    && occurrence.is_none_or(|o| &m.occurrence == o)
            })
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

/// An auto-assigned net name: `N$` and the net's number.
pub fn is_auto_name(name: &str) -> bool {
    name.strip_prefix("N$")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The `<base>` of a `<base>_<n>` name, the shape `dedup_net_names` gives the
/// later of two nets with one name.
fn dedup_base(name: &str) -> Option<&str> {
    let (base, n) = name.rsplit_once('_')?;
    (!base.is_empty() && !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then_some(base)
}
