//! The PCB ratsnest the canvas shows (#2192): the lines for the active
//! board, kept until the board changes, and the nets the user hid.
//!
//! The lines come from `pcb-geom`; drawing them is `signex-renderer`'s.

use std::collections::BTreeSet;
use std::path::PathBuf;

use pcb_geom::RatLine;
use signex_renderer::pcb::PcbSnapshot;
use signex_types::pcb::PcbBoard;

/// A board and the revision of it the lines were computed for.
pub(crate) type RatsnestKey = (PathBuf, u64);

#[derive(Debug, Default)]
pub struct PcbRatsnest {
    key: Option<RatsnestKey>,
    lines: Vec<RatLine>,
    hidden_nets: BTreeSet<u32>,
}

impl PcbRatsnest {
    /// Recompute the lines unless `key` is the board they were computed for.
    /// A `None` key (a board with no revision) always recomputes.
    pub(crate) fn refresh(&mut self, key: Option<RatsnestKey>, board: &PcbBoard) {
        if key.is_some() && key == self.key {
            return;
        }
        self.lines = pcb_geom::ratsnest(board);
        self.key = key;
    }

    pub fn lines(&self) -> &[RatLine] {
        &self.lines
    }

    pub fn is_net_hidden(&self, net: u32) -> bool {
        self.hidden_nets.contains(&net)
    }

    /// Hide `net`'s lines, or show them again.
    pub(crate) fn toggle_net(&mut self, net: u32) {
        if !self.hidden_nets.remove(&net) {
            self.hidden_nets.insert(net);
        }
    }

    /// `snapshot` with the lines of every net that is not hidden.
    pub(crate) fn apply(&self, snapshot: PcbSnapshot) -> PcbSnapshot {
        snapshot.with_ratsnest(&self.lines, &self.hidden_nets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcb_geom::PointNm;
    use signex_types::pcb::{Footprint, Pad, PadNet, PadShape, PadType, Point};
    use uuid::Uuid;

    fn pad(n: u128, x: f64) -> Pad {
        Pad {
            uuid: Uuid::from_u128(n),
            number: n.to_string(),
            pad_type: PadType::Smd,
            shape: PadShape::Rect,
            position: Point::new(x, 0.0),
            size: Point::new(1.0, 1.0),
            drill: None,
            layers: vec!["TopCopper".into()],
            net: Some(PadNet {
                number: 1,
                name: "N1".into(),
            }),
            roundrect_ratio: 0.0,
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }
    }

    fn board(xs: &[f64]) -> PcbBoard {
        let footprint = Footprint {
            uuid: Uuid::from_u128(100),
            reference: "U1".into(),
            value: String::new(),
            footprint_id: String::new(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            layer: "TopCopper".into(),
            locked: false,
            pads: xs
                .iter()
                .enumerate()
                .map(|(i, &x)| pad(i as u128 + 1, x))
                .collect(),
            graphics: Vec::new(),
            properties: Vec::new(),
            unknown: Default::default(),
            unknown_columns: Default::default(),
        };
        PcbBoard {
            uuid: Uuid::nil(),
            version: 1,
            generator: String::new(),
            thickness: 1.6,
            outline: Vec::new(),
            layers: Vec::new(),
            setup: None,
            nets: Vec::new(),
            footprints: vec![footprint],
            segments: Vec::new(),
            vias: Vec::new(),
            zones: Vec::new(),
            graphics: Vec::new(),
            texts: Vec::new(),
            unknown: Default::default(),
        }
    }

    fn key(revision: u64) -> Option<RatsnestKey> {
        Some((PathBuf::from("a.snxpcb"), revision))
    }

    #[test]
    fn lines_are_kept_until_the_revision_changes() {
        let mut state = PcbRatsnest::default();
        state.refresh(key(1), &board(&[0.0, 5.0]));
        assert_eq!(state.lines().len(), 1);
        assert_eq!(state.lines()[0].to, PointNm::new(5_000_000, 0));
        // Same revision: the board is not read again.
        state.refresh(key(1), &board(&[0.0, 5.0, 9.0]));
        assert_eq!(state.lines().len(), 1);
        // A new revision recomputes.
        state.refresh(key(2), &board(&[0.0, 5.0, 9.0]));
        assert_eq!(state.lines().len(), 2);
        // No revision always recomputes.
        state.refresh(None, &board(&[0.0, 5.0]));
        state.refresh(None, &board(&[0.0, 5.0, 9.0, 12.0]));
        assert_eq!(state.lines().len(), 3);
    }

    #[test]
    fn a_hidden_net_drops_out_of_the_snapshot_and_comes_back() {
        let mut state = PcbRatsnest::default();
        state.refresh(key(1), &board(&[0.0, 5.0]));
        assert_eq!(state.apply(PcbSnapshot::default()).ratsnest_lines.len(), 1);
        state.toggle_net(1);
        assert!(state.is_net_hidden(1));
        assert!(
            state
                .apply(PcbSnapshot::default())
                .ratsnest_lines
                .is_empty()
        );
        state.toggle_net(1);
        assert!(!state.is_net_hidden(1));
        assert_eq!(state.apply(PcbSnapshot::default()).ratsnest_lines.len(), 1);
    }
}
