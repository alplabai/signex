//! The PCB ratsnest the canvas shows (#2192): the lines for the active
//! board, kept until the board changes, and the nets the user hid. The
//! board's pour fills (#2216) are kept with them: the lines are computed
//! through the fills, and the canvas draws the same fills.
//!
//! The lines and fills come from `pcb-geom`; drawing them is
//! `signex-renderer`'s.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pcb_geom::{ConnectionCounts, RatLine, ZoneFills};
use signex_renderer::pcb::PcbSnapshot;
use signex_types::pcb::PcbBoard;

/// A board and the revision of it the lines were computed for.
pub(crate) type RatsnestKey = (PathBuf, u64);

#[derive(Debug, Default)]
pub struct PcbRatsnest {
    key: Option<RatsnestKey>,
    lines: Vec<RatLine>,
    /// The pour fills of the board the lines were computed for.
    fills: ZoneFills,
    /// Hidden net numbers per board: a net number means nothing off its board.
    hidden_nets: BTreeMap<PathBuf, BTreeSet<u32>>,
    /// The status bar's Routed and Unrouted counts (#2460) of every board
    /// the lines were computed for, with that board's revision: a board
    /// shown in its own window keeps its counts while another is in view.
    counts: BTreeMap<PathBuf, (u64, ConnectionCounts)>,
}

impl PcbRatsnest {
    /// Recompute the pour fills and the lines unless `key` is the board they
    /// were computed for. A `None` key (a board with no revision) always
    /// recomputes. The one place a board's fills are refreshed, and with
    /// them the board's connection counts.
    pub(crate) fn refresh(&mut self, key: Option<RatsnestKey>, board: &PcbBoard) {
        if key.is_some() && key == self.key {
            return;
        }
        self.fills = pcb_geom::zone_fills(board);
        self.lines = pcb_geom::ratsnest_with_fills(board, &self.fills);
        if let Some((path, revision)) = &key {
            let counts = pcb_geom::connection_counts(board, &self.lines);
            self.counts.insert(path.clone(), (*revision, counts));
        }
        self.key = key;
    }

    /// The connection counts of the board at `path`, if they were computed
    /// for its `revision`; `None` before that.
    pub(crate) fn counts_for(&self, path: &Path, revision: u64) -> Option<&ConnectionCounts> {
        self.counts
            .get(path)
            .filter(|(at, _)| *at == revision)
            .map(|(_, counts)| counts)
    }

    pub fn lines(&self) -> &[RatLine] {
        &self.lines
    }

    /// The pour fills of the board last refreshed.
    pub fn zone_fills(&self) -> &ZoneFills {
        &self.fills
    }

    /// The pour fills held, if they are those of `key` (that board at that
    /// revision); `None` before they are computed for it.
    pub(crate) fn fills_for(&self, key: &RatsnestKey) -> Option<&ZoneFills> {
        (self.key.as_ref() == Some(key)).then_some(&self.fills)
    }

    /// Forget which board revision the lines and fills are of, so the next
    /// refresh computes them again (Repour, #2216).
    pub(crate) fn invalidate(&mut self) {
        self.key = None;
    }

    /// How many nets still have a ratsnest line, if the lines held are the
    /// ones of `key` (#2353); `None` before they are computed for it.
    pub(crate) fn unrouted_nets(&self, key: &RatsnestKey) -> Option<usize> {
        (self.key.as_ref() == Some(key)).then(|| {
            self.lines
                .iter()
                .map(|line| line.net)
                .collect::<BTreeSet<u32>>()
                .len()
        })
    }

    /// How many nets of `board` have their ratsnest hidden.
    pub(crate) fn hidden_net_count(&self, board: &Path) -> usize {
        self.hidden_nets.get(board).map_or(0, BTreeSet::len)
    }

    pub fn is_net_hidden(&self, board: &Path, net: u32) -> bool {
        self.hidden_nets
            .get(board)
            .is_some_and(|nets| nets.contains(&net))
    }

    /// Hide `net`'s lines on `board`, or show them again.
    pub(crate) fn toggle_net(&mut self, board: &Path, net: u32) {
        let nets = self.hidden_nets.entry(board.to_path_buf()).or_default();
        if !nets.remove(&net) {
            nets.insert(net);
        }
        if nets.is_empty() {
            self.hidden_nets.remove(board);
        }
    }

    /// `snapshot` of a board the cache does not hold (an undocked window's
    /// board that is not the active tab), with its ratsnest as the active
    /// tab draws it. Computes the lines through `fills` (the board's) without
    /// replacing the cache.
    pub(crate) fn apply_uncached(
        &self,
        path: &Path,
        board: &PcbBoard,
        fills: &ZoneFills,
        snapshot: PcbSnapshot,
    ) -> PcbSnapshot {
        let lines = pcb_geom::ratsnest_with_fills(board, fills);
        let none = BTreeSet::new();
        let hidden = self.hidden_nets.get(path).unwrap_or(&none);
        snapshot.with_ratsnest(&lines, hidden)
    }

    /// `snapshot` with the lines of every net of `board` that is not hidden.
    pub(crate) fn apply(&self, board: &Path, snapshot: PcbSnapshot) -> PcbSnapshot {
        let none = BTreeSet::new();
        let hidden = self.hidden_nets.get(board).unwrap_or(&none);
        snapshot.with_ratsnest(&self.lines, hidden)
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
            trapezoid_delta: Default::default(),
            custom_outline: Vec::new(),
            rotation: 0.0,
            layer_sizes: None,
            net_locked: false,
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
            schematic_link: None,
            library_ref: None,
            component: Default::default(),
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
            sketch: None,
            stackup: None,
            rooms: Vec::new(),
            options: Default::default(),
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
            free: Default::default(),
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

    /// #2460: the counts follow the lines, per board and revision, and a
    /// board's counts outlive a refresh for another board.
    #[test]
    fn the_connection_counts_are_kept_per_board_and_revision() {
        let mut state = PcbRatsnest::default();
        let a = Path::new("a.snxpcb");
        assert!(state.counts_for(a, 1).is_none());
        state.refresh(key(1), &board(&[0.0, 5.0, 9.0]));
        let counts = state.counts_for(a, 1).expect("counted");
        assert_eq!((counts.total, counts.unrouted), (2, 2));
        assert!(state.counts_for(a, 2).is_none(), "another revision");
        state.refresh(Some((PathBuf::from("b.snxpcb"), 7)), &board(&[0.0]));
        assert!(state.counts_for(a, 1).is_some(), "kept for its board");
        let b = state.counts_for(Path::new("b.snxpcb"), 7).expect("b");
        assert_eq!((b.total, b.unrouted), (0, 0));
        // No revision: nothing to key the counts by.
        state.refresh(None, &board(&[0.0, 5.0]));
        assert!(state.counts_for(a, 1).is_some());
    }

    #[test]
    fn a_hidden_net_drops_out_of_the_snapshot_and_comes_back() {
        let mut state = PcbRatsnest::default();
        state.refresh(key(1), &board(&[0.0, 5.0]));
        let a = Path::new("a.snxpcb");
        let b = Path::new("b.snxpcb");
        assert_eq!(
            state.apply(a, PcbSnapshot::default()).ratsnest_lines.len(),
            1
        );
        state.toggle_net(a, 1);
        assert!(state.is_net_hidden(a, 1));
        assert!(
            state
                .apply(a, PcbSnapshot::default())
                .ratsnest_lines
                .is_empty()
        );
        // The same net number on another board is a different net.
        assert!(!state.is_net_hidden(b, 1));
        assert_eq!(
            state.apply(b, PcbSnapshot::default()).ratsnest_lines.len(),
            1
        );
        state.toggle_net(a, 1);
        assert!(!state.is_net_hidden(a, 1));
        assert_eq!(
            state.apply(a, PcbSnapshot::default()).ratsnest_lines.len(),
            1
        );
    }
}
