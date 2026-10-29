//! Where a run of Go to net partner is in the ring of ports of one net.
//!
//! The ring leaves the source's sheet first and ends on the source itself, so
//! repeating the command visits every port once per lap. It belongs to one
//! source and one index: selecting another port, or an edit that rebuilds the
//! index, starts a new lap.

use std::sync::{Arc, Weak};

use signex_net::{NetMember, SheetKey};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

/// A port in one sheet placement.
pub type Landing = (SheetKey, Uuid, InstancePath);

use super::ProjectNetData;

#[derive(Debug)]
pub struct JumpCursor {
    /// The index the ring was read from. Weak, so a replaced index is freed
    /// even while a lap is parked.
    index: Weak<ProjectNetData>,
    /// The port the last jump landed on: the next jump continues from it.
    landed: Landing,
    ring: Vec<NetMember>,
    next: usize,
}

impl JumpCursor {
    /// A lap from `source`: the ports to visit, then `source` again.
    pub fn start(index: Arc<ProjectNetData>, source: NetMember, partners: Vec<NetMember>) -> Self {
        let landed = (source.sheet.clone(), source.id, source.occurrence.clone());
        let mut ring = partners;
        ring.push(source);
        Self {
            index: Arc::downgrade(&index),
            landed,
            ring,
            next: 0,
        }
    }

    /// Whether a jump from `source` on `index` carries on this lap.
    pub fn continues(&self, index: &Arc<ProjectNetData>, source: &Landing) -> bool {
        self.index.ptr_eq(&Arc::downgrade(index)) && &self.landed == source
    }

    /// The port to land on now; the lap wraps after its last.
    pub fn advance(&mut self) -> &NetMember {
        let at = self.next % self.ring.len();
        self.next = at + 1;
        let member = &self.ring[at];
        self.landed = (member.sheet.clone(), member.id, member.occurrence.clone());
        member
    }

    /// Ports in a lap, the source included.
    pub fn lap_len(&self) -> usize {
        self.ring.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_net::{NetIndex, NetMemberKind};
    use signex_types::schematic::{InstancePath, LabelType, Point};

    fn index() -> Arc<ProjectNetData> {
        Arc::new(ProjectNetData {
            index: NetIndex::default(),
            paths: Default::default(),
            base_dir: None,
            fold: Default::default(),
            classes: Vec::new(),
        })
    }

    fn port(sheet: &str, n: u128) -> NetMember {
        NetMember {
            sheet: SheetKey::new(sheet),
            occurrence: InstancePath::root(),
            kind: NetMemberKind::Label(LabelType::Global),
            id: Uuid::from_u128(n),
            at: Point::new(0.0, 0.0),
            text: "VCC".into(),
        }
    }

    fn lap() -> (Arc<ProjectNetData>, JumpCursor) {
        let idx = index();
        let cursor = JumpCursor::start(
            Arc::clone(&idx),
            port("root", 1),
            vec![port("c", 5), port("c", 6), port("root", 2)],
        );
        (idx, cursor)
    }

    #[test]
    fn a_lap_visits_every_port_once_and_wraps_to_the_source() {
        let (_, mut c) = lap();
        let seen: Vec<u128> = (0..5).map(|_| c.advance().id.as_u128()).collect();
        assert_eq!(seen, [5, 6, 2, 1, 5]);
        assert_eq!(c.lap_len(), 4);
    }

    #[test]
    fn it_continues_only_from_where_it_landed_on_the_same_index() {
        let (idx, mut c) = lap();
        let source = (
            SheetKey::new("root"),
            Uuid::from_u128(1),
            InstancePath::root(),
        );
        assert!(c.continues(&idx, &source), "before the first jump");
        c.advance();
        let landed = (SheetKey::new("c"), Uuid::from_u128(5), InstancePath::root());
        let other_placement = (
            SheetKey::new("c"),
            Uuid::from_u128(5),
            InstancePath::from_chain(vec![Uuid::from_u128(9)]),
        );
        assert!(c.continues(&idx, &landed));
        assert!(!c.continues(&idx, &source), "another port starts a new lap");
        assert!(
            !c.continues(&idx, &other_placement),
            "the same port in another placement starts a new lap"
        );
        assert!(
            !c.continues(&index(), &landed),
            "a rebuilt index starts a new lap"
        );
    }
}
