//! Which elements of the project sit on each stitched net, sheet by sheet.
//!
//! The stitcher already groups every per-sheet net into the project's nets;
//! this module keeps what it knows about the members of each group (pins,
//! labels, sheet pins, power ports, wires, junctions) instead of discarding it,
//! so cross-sheet navigation, net highlighting and the hover insight all read
//! the same connectivity the netlist export does and none re-derives it.

use std::collections::HashMap;

use signex_types::net::NetId;
use signex_types::schematic::{InstancePath, LabelType, Point};
use uuid::Uuid;

use super::{Analysis, L2, Occ, SheetKey};

/// What kind of element a [`NetMember`] is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetMemberKind {
    Wire,
    Junction,
    /// A component pin; `id` of the member is the placed symbol's uuid.
    Pin {
        reference: String,
        pin: String,
    },
    /// A power-port symbol that names the net; `id` is the symbol's uuid.
    PowerPort,
    Label(LabelType),
    /// A sheet entry on a parent sheet's sheet symbol; `id` is the pin's uuid.
    SheetPin {
        sheet_symbol: Uuid,
    },
}

/// One element on a net, with the sheet (and placement) it sits on.
#[derive(Debug, Clone, PartialEq)]
pub struct NetMember {
    pub sheet: SheetKey,
    /// The `ChildSheet.uuid` chain of the placement; two placements of one
    /// sheet file are two occurrences with distinct members.
    pub occurrence: InstancePath,
    pub kind: NetMemberKind,
    pub id: Uuid,
    /// Where the element sits on its sheet (a wire's midpoint).
    pub at: Point,
    /// Label text, sheet-pin name or power value; empty for the rest.
    pub text: String,
}

impl NetMember {
    /// A member that carries a net across sheets: a Global or Hierarchical
    /// label, a sheet entry or a power port.
    pub fn is_port(&self) -> bool {
        matches!(
            self.kind,
            NetMemberKind::Label(LabelType::Global | LabelType::Hierarchical)
                | NetMemberKind::SheetPin { .. }
                | NetMemberKind::PowerPort
        )
    }

    /// A member worth stopping at when stepping through a net: everything but
    /// the wiring.
    pub fn is_steppable(&self) -> bool {
        !matches!(self.kind, NetMemberKind::Wire | NetMemberKind::Junction)
    }
}

/// One project net and everything on it. `net` is the netlist's id; `None` for
/// a net with no component pin, which the netlist exports no net for but whose
/// labels and sheet entries still join across sheets.
#[derive(Debug, Clone, PartialEq)]
pub struct NetMembers {
    pub net: Option<NetId>,
    /// The netlist's name (dedup suffix included), or the name the net would
    /// get; empty for an unnamed pinless net.
    pub name: String,
    /// In walk order: occurrence by occurrence, then document order.
    pub members: Vec<NetMember>,
}

/// A sheet entry's position, kept per net root by the level-1 analysis.
#[derive(Debug, Clone)]
pub(super) struct SheetPinAt {
    pub sheet_symbol: Uuid,
    pub pin: Uuid,
    pub name: String,
    pub at: Point,
}

/// Lazily built wire-midpoint and junction positions per occurrence.
#[derive(Default)]
pub(super) struct ElementPoints(HashMap<usize, HashMap<Uuid, Point>>);

impl ElementPoints {
    fn of(&mut self, oid: usize, occ: &Occ) -> &HashMap<Uuid, Point> {
        self.0.entry(oid).or_insert_with(|| {
            let wires = occ.sheet.wires.iter().map(|w| {
                let mid = Point::new((w.start.x + w.end.x) / 2.0, (w.start.y + w.end.y) / 2.0);
                (w.uuid, mid)
            });
            let junctions = occ.sheet.junctions.iter().map(|j| (j.uuid, j.position));
            wires.chain(junctions).collect()
        })
    }
}

/// The members of one level-2 group, in walk order. `nodes` is the group's
/// `(occurrence, net root)` list, sorted.
pub(super) fn collect_members(
    nodes: &[L2],
    occs: &[Occ],
    analyses: &[Analysis],
    points: &mut ElementPoints,
) -> Vec<NetMember> {
    let mut out = Vec::new();
    for &(oid, root) in nodes {
        let (occ, a) = (&occs[oid], &analyses[oid]);
        let member = |kind: NetMemberKind, id: Uuid, at: Point, text: String| NetMember {
            sheet: occ.key.clone(),
            occurrence: occ.path.clone(),
            kind,
            id,
            at,
            text,
        };
        if let Some((wires, junctions)) = a.membership.get(&root) {
            let at = points.of(oid, occ);
            for (ids, kind) in [
                (wires, NetMemberKind::Wire),
                (junctions, NetMemberKind::Junction),
            ] {
                for id in ids {
                    // Every wire and junction of a net is on its sheet; one
                    // with no position is skipped rather than put at the origin.
                    let Some(p) = at.get(id).copied() else {
                        debug_assert!(false, "net element {id} is not on its sheet");
                        continue;
                    };
                    out.push(member(kind.clone(), *id, p, String::new()));
                }
            }
        }
        if let Some(terminals) = a.terminals.get(&root) {
            let positions = a.terminal_at.get(&root);
            for (i, t) in terminals.iter().enumerate() {
                let Some(at) = positions.and_then(|p| p.get(i)).copied() else {
                    debug_assert!(false, "pin {} has no tip position", t.pin);
                    continue;
                };
                let kind = NetMemberKind::Pin {
                    reference: t.reference.clone(),
                    pin: t.pin.clone(),
                };
                out.push(member(kind, t.symbol, at, String::new()));
            }
        }
        for (value, at, symbol) in a.power_by_root.get(&root).into_iter().flatten() {
            out.push(member(
                NetMemberKind::PowerPort,
                *symbol,
                *at,
                value.clone(),
            ));
        }
        for label in a.net_labels.get(&root).into_iter().flatten() {
            let kind = NetMemberKind::Label(label.label_type);
            out.push(member(kind, label.uuid, label.position, label.text.clone()));
        }
        for pin in a.sheet_pins.get(&root).into_iter().flatten() {
            let kind = NetMemberKind::SheetPin {
                sheet_symbol: pin.sheet_symbol,
            };
            out.push(member(kind, pin.pin, pin.at, pin.name.clone()));
        }
    }
    out
}
