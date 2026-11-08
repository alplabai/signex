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

    fn pad_json(uuid: &str, x: f64) -> String {
        format!(
            r#"{{ "uuid": "{uuid}", "number": "1", "pad_type": "smd", "shape": "rect",
               "position": {{ "x": {x}, "y": 0.0 }}, "size": {{ "x": 1.0, "y": 1.0 }},
               "drill": null, "layers": ["BottomCopper"],
               "net": {{ "number": 1, "name": "N1" }}, "roundrect_ratio": 0.0 }}"#
        )
    }

    #[test]
    fn a_bottom_side_pad_is_drawn_where_its_ratsnest_line_ends() {
        let json = format!(
            r#"{{ "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "version": 1,
               "generator": "test", "thickness": 1.6, "outline": [], "layers": [],
               "setup": null, "nets": [{{ "number": 1, "name": "N1" }}],
               "footprints": [{{ "uuid": "8e98ba95-1d0f-4d9f-84e0-27ddf90e10b5",
                 "reference": "U1", "value": "", "footprint_id": "",
                 "position": {{ "x": 20.0, "y": 0.0 }}, "rotation": 0.0,
                 "layer": "BottomCopper", "locked": false,
                 "pads": [{}, {}], "graphics": [], "properties": [] }}],
               "segments": [], "vias": [], "zones": [], "graphics": [], "texts": [] }}"#,
            pad_json("47ceef20-d496-4e62-b8d8-7c34195c6818", 4.0),
            pad_json("47ceef20-d496-4e62-b8d8-7c34195c6819", -4.0),
        );
        let board: signex_types::pcb::PcbBoard = serde_json::from_str(&json).expect("board");
        let rat = pcb_geom::ratsnest(&board);
        assert_eq!(rat.len(), 1);
        let snapshot = PcbSnapshot::from_board(&board);
        let mut drawn: Vec<[f32; 2]> = snapshot.pads.iter().map(|p| p.center).collect();
        drawn.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let ends = [rat[0].from.to_mm(), rat[0].to.to_mm()];
        assert_eq!(drawn.len(), 2);
        for (center, end) in drawn.iter().zip(ends) {
            assert_eq!(*center, [end.x as f32, end.y as f32]);
        }
        // Mirrored: local +4 sits at world 16.
        assert_eq!(drawn[0], [16.0, 0.0]);
    }

    #[test]
    fn the_snapshot_carries_them_to_the_overlay() {
        let snapshot = PcbSnapshot::default().with_ratsnest(&[line(3, 0)], &BTreeSet::new());
        assert_eq!(snapshot.ratsnest_lines.len(), 1);
    }
}
