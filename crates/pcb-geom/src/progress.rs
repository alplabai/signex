//! How far a board is from done (#2460), for `board_status`: the parts not
//! placed on the board yet and the board's total routed length. The routed
//! and unrouted connections are [`crate::connection_counts`], read with the
//! status bar's ratsnest.

use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::net_length::net_routed_length_mm;
use crate::shape_overlap::point_in_polygon;
use crate::units::PointNm;

/// The footprints whose origin is not inside the board outline (Update PCB
/// stages a new part beside the board, `staging`), in board order. `None`
/// without an outline of three points or more, or with an outline point the
/// geometry cannot hold: then nothing says where the board is. A footprint
/// whose position the geometry cannot hold counts as unplaced.
pub fn unplaced_footprints(board: &PcbBoard) -> Option<Vec<Uuid>> {
    if board.outline.len() < 3 {
        return None;
    }
    let outline: Vec<PointNm> = board
        .outline
        .iter()
        .map(|p| PointNm::from_mm(*p, "outline"))
        .collect::<Result<_, _>>()
        .ok()?;
    let unplaced = board
        .footprints
        .iter()
        .filter(|fp| {
            !matches!(
                PointNm::from_mm(fp.position, "footprint position"),
                Ok(at) if point_in_polygon(&outline, at)
            )
        })
        .map(|fp| fp.uuid)
        .collect();
    Some(unplaced)
}

/// The routed length of every named net of the board summed, mm: the
/// figure `query_board_nets` gives per net. A net with an item the geometry
/// cannot hold adds nothing, as it reads 0 there.
pub fn total_routed_length_mm(board: &PcbBoard) -> f64 {
    board
        .nets
        .iter()
        .filter(|def| def.number != 0)
        .map(|def| net_routed_length_mm(board, def.number).unwrap_or(0.0))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb::{Footprint, NetDef, Point, Segment};

    fn footprint(n: u128, x: f64, y: f64) -> Footprint {
        Footprint {
            uuid: Uuid::from_u128(n),
            reference: format!("R{n}"),
            value: "10k".into(),
            footprint_id: String::new(),
            position: Point::new(x, y),
            rotation: 0.0,
            layer: "TopCopper".into(),
            locked: false,
            pads: Vec::new(),
            graphics: Vec::new(),
            properties: Vec::new(),
            schematic_link: None,
            library_ref: None,
            component: Default::default(),
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }
    }

    fn track(n: u128, net: u32, length: f64) -> Segment {
        Segment {
            locked: false,
            attributes: Default::default(),
            uuid: Uuid::from_u128(0x2000 + n),
            start: Point::new(0.0, 0.0),
            end: Point::new(length, 0.0),
            width: 0.25,
            layer: "TopCopper".into(),
            net,
            unknown_columns: Default::default(),
        }
    }

    /// A 50 x 40 mm board: R1 on it, R2 staged to its right.
    fn board() -> PcbBoard {
        PcbBoard {
            sketch: None,
            stackup: None,
            rooms: Vec::new(),
            options: Default::default(),
            uuid: Uuid::nil(),
            version: 1,
            generator: String::new(),
            thickness: 1.6,
            outline: vec![
                Point::new(0.0, 0.0),
                Point::new(50.0, 0.0),
                Point::new(50.0, 40.0),
                Point::new(0.0, 40.0),
            ],
            layers: Vec::new(),
            setup: None,
            nets: Vec::new(),
            footprints: vec![footprint(1, 10.0, 10.0), footprint(2, 80.0, 10.0)],
            segments: Vec::new(),
            vias: Vec::new(),
            zones: Vec::new(),
            graphics: Vec::new(),
            texts: Vec::new(),
            free: Default::default(),
            unknown: Default::default(),
        }
    }

    #[test]
    fn a_footprint_outside_the_outline_is_unplaced() {
        let board = board();
        assert_eq!(unplaced_footprints(&board), Some(vec![Uuid::from_u128(2)]));
    }

    #[test]
    fn without_an_outline_nothing_says_what_is_unplaced() {
        let mut board = board();
        board.outline.truncate(2);
        assert_eq!(unplaced_footprints(&board), None);
    }

    #[test]
    fn the_routed_length_sums_the_named_nets_and_leaves_out_net_zero() {
        let mut board = board();
        board.nets = vec![
            NetDef {
                number: 1,
                name: "GND".into(),
            },
            NetDef {
                number: 2,
                name: "VCC".into(),
            },
        ];
        board.segments = vec![track(1, 1, 10.0), track(2, 2, 2.5), track(3, 0, 100.0)];
        let total = total_routed_length_mm(&board);
        assert!((total - 12.5).abs() < 1e-9, "{total}");
    }
}
