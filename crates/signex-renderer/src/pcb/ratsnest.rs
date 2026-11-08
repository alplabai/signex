//! Ratsnest lines for the PCB scene (#2192).
//!
//! `pcb-geom` computes the lines; this turns them into the snapshot's
//! overlay input. [`emit`](super::emit) already draws that input as thin
//! dashed lines in the theme's ratsnest colour (`ColorSlot::Ratsnest`).

use std::collections::BTreeSet;

use pcb_geom::RatLine;

use super::{PcbSnapshot, RatsnestInput};

/// The overlay input for `lines`, leaving out the nets in `hidden`.
pub fn ratsnest_inputs(lines: &[RatLine], hidden: &BTreeSet<u32>) -> Vec<RatsnestInput> {
    lines
        .iter()
        .filter(|line| !hidden.contains(&line.net))
        .map(|line| {
            let (from, to) = (line.from.to_mm(), line.to.to_mm());
            RatsnestInput {
                p0: [from.x as f32, from.y as f32],
                p1: [to.x as f32, to.y as f32],
                net: line.net,
            }
        })
        .collect()
}

impl PcbSnapshot {
    /// This snapshot with the ratsnest `lines`, minus the `hidden` nets.
    pub fn with_ratsnest(self, lines: &[RatLine], hidden: &BTreeSet<u32>) -> Self {
        self.with_ratsnest_lines(ratsnest_inputs(lines, hidden))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcb_geom::PointNm;

    fn line(net: u32, x: i64) -> RatLine {
        RatLine {
            net,
            from: PointNm::new(x * 1_000_000, 0),
            to: PointNm::new(x * 1_000_000 + 2_500_000, 1_000_000),
        }
    }

    #[test]
    fn lines_become_mm_inputs_and_hidden_nets_drop_out() {
        let lines = [line(1, 0), line(2, 4), line(1, 8)];
        let all = ratsnest_inputs(&lines, &BTreeSet::new());
        assert_eq!(all.len(), 3);
        assert_eq!(all[1].p0, [4.0, 0.0]);
        assert_eq!(all[1].p1, [6.5, 1.0]);
        assert_eq!(all[1].net, 2);

        let hidden: BTreeSet<u32> = [1].into();
        let shown = ratsnest_inputs(&lines, &hidden);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].net, 2);
    }

    #[test]
    fn the_snapshot_carries_them_to_the_overlay() {
        let snapshot = PcbSnapshot::default().with_ratsnest(&[line(3, 0)], &BTreeSet::new());
        assert_eq!(snapshot.ratsnest_lines.len(), 1);
    }
}
