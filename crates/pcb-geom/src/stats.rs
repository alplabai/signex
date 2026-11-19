//! Board totals for the Properties panel's Board Information (#2353): the
//! outline's area, the area the footprint bodies take and object counts,
//! all read from the board model.

use signex_types::pcb::PcbBoard;

use crate::body::footprint_body;
use crate::pick_filter::PickKind;
use crate::shape::Shape;
use crate::transform::Side;
use crate::units::{NM_PER_MM, PointNm};

/// What the Board Information section counts on a board.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardStats {
    /// Area inside the outline, mm²; `None` without an outline of three
    /// points or more, or with a point the geometry cannot hold.
    pub area_mm2: Option<f64>,
    /// The footprint bodies' areas summed, mm² (overlaps count twice). A
    /// footprint whose body cannot be built adds nothing.
    pub components_area_mm2: f64,
    pub components_top: usize,
    pub components_bottom: usize,
    pub pads: usize,
    /// Track segments.
    pub tracks: usize,
    pub vias: usize,
    /// Zones that are not a fill, a solid region or a keepout (#2386).
    pub polygons: usize,
    /// Fills, solid regions and keepouts drawn on the board (#2373), as
    /// [`PickKind::of_zone`](crate::PickKind::of_zone) names them.
    pub fills: usize,
    pub regions: usize,
    pub keepouts: usize,
    /// Board texts.
    pub strings: usize,
    /// Pads with a drill.
    pub pad_holes: usize,
    /// Vias with a drill.
    pub via_holes: usize,
}

impl BoardStats {
    /// The totals of `board`.
    pub fn of(board: &PcbBoard) -> Self {
        let mut stats = BoardStats {
            area_mm2: outline_area_mm2(board),
            tracks: board.segments.len(),
            vias: board.vias.len(),
            strings: board.texts.len(),
            via_holes: board.vias.iter().filter(|v| v.drill > 0.0).count(),
            ..BoardStats::default()
        };
        for zone in &board.zones {
            match PickKind::of_zone(zone) {
                Some(PickKind::Fills) => stats.fills += 1,
                Some(PickKind::Regions) => stats.regions += 1,
                Some(PickKind::Keepouts) => stats.keepouts += 1,
                _ => stats.polygons += 1,
            }
        }
        for footprint in &board.footprints {
            match Side::of_layer(&footprint.layer) {
                Side::Top => stats.components_top += 1,
                Side::Bottom => stats.components_bottom += 1,
            }
            stats.pads += footprint.pads.len();
            stats.pad_holes += footprint
                .pads
                .iter()
                .filter(|pad| pad.drill.as_ref().is_some_and(|d| d.diameter > 0.0))
                .count();
            if let Ok(body) = footprint_body(footprint) {
                stats.components_area_mm2 += shape_area_mm2(&body);
            }
        }
        // Free pads count with the footprints' (#2373).
        stats.pads += board.free.pads.len();
        stats.pad_holes += board
            .free
            .pads
            .iter()
            .filter(|pad| pad.drill.as_ref().is_some_and(|d| d.diameter > 0.0))
            .count();
        stats
    }

    pub fn components(&self) -> usize {
        self.components_top + self.components_bottom
    }

    /// The share of the board area the footprint bodies take, `0.0` and up;
    /// `None` without a board area.
    pub fn density(&self) -> Option<f64> {
        self.area_mm2
            .filter(|area| *area > 0.0)
            .map(|area| self.components_area_mm2 / area)
    }
}

/// The area of a body: its turned rectangle when it is one, else its box.
fn shape_area_mm2(shape: &Shape) -> f64 {
    let nm2 = match *shape {
        Shape::Rect { half_w, half_h, .. } => 4 * i128::from(half_w) * i128::from(half_h),
        _ => {
            let aabb = shape.aabb();
            i128::from(aabb.width()) * i128::from(aabb.height())
        }
    };
    nm2_to_mm2(nm2)
}

/// The shoelace area of the outline polygon.
fn outline_area_mm2(board: &PcbBoard) -> Option<f64> {
    if board.outline.len() < 3 {
        return None;
    }
    let points: Vec<PointNm> = board
        .outline
        .iter()
        .map(|p| PointNm::from_mm(*p, "outline"))
        .collect::<Result<_, _>>()
        .ok()?;
    let twice: i128 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a.cross(*b))
        .sum();
    Some(nm2_to_mm2(twice.abs() / 2))
}

fn nm2_to_mm2(nm2: i128) -> f64 {
    let per_mm = NM_PER_MM as f64;
    nm2 as f64 / (per_mm * per_mm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb::{DrillDef, Footprint, Pad, PadShape, PadType, Point, Via, ViaType};
    use uuid::Uuid;

    fn pad(n: u128, x: f64, drill: Option<f64>) -> Pad {
        Pad {
            uuid: Uuid::from_u128(n),
            number: n.to_string(),
            pad_type: if drill.is_some() {
                PadType::Thru
            } else {
                PadType::Smd
            },
            shape: PadShape::Rect,
            position: Point::new(x, 0.0),
            size: Point::new(2.0, 2.0),
            drill: drill.map(|diameter| DrillDef {
                diameter,
                shape: String::new(),
            }),
            layers: vec!["TopCopper".into()],
            net: None,
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

    fn footprint(n: u128, layer: &str, pads: Vec<Pad>) -> Footprint {
        Footprint {
            uuid: Uuid::from_u128(n),
            reference: String::new(),
            value: String::new(),
            footprint_id: String::new(),
            position: Point::new(10.0, 10.0),
            rotation: 0.0,
            layer: layer.into(),
            locked: false,
            pads,
            graphics: Vec::new(),
            properties: Vec::new(),
            schematic_link: None,
            library_ref: None,
            component: Default::default(),
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }
    }

    /// A 50 x 40 mm board; a top part with a drilled and an SMD pad, a bare
    /// bottom part, one drilled via.
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
            footprints: vec![
                footprint(
                    1,
                    "TopCopper",
                    vec![pad(2, -1.0, Some(1.0)), pad(3, 1.0, None)],
                ),
                footprint(4, "BottomCopper", Vec::new()),
            ],
            segments: Vec::new(),
            vias: vec![Via {
                attributes: Default::default(),
                uuid: Uuid::from_u128(5),
                position: Point::new(5.0, 5.0),
                diameter: 0.6,
                drill: 0.3,
                layers: Vec::new(),
                net: 0,
                locked: false,
                layer_sizes: None,
                via_type: ViaType::Through,
                unknown_columns: Default::default(),
            }],
            zones: Vec::new(),
            graphics: Vec::new(),
            texts: Vec::new(),
            free: Default::default(),
            unknown: Default::default(),
        }
    }

    #[test]
    fn the_totals_read_the_board() {
        let stats = BoardStats::of(&board());
        assert_eq!(stats.area_mm2, Some(2000.0));
        assert_eq!((stats.components_top, stats.components_bottom), (1, 1));
        assert_eq!((stats.pads, stats.pad_holes), (2, 1));
        assert_eq!((stats.vias, stats.via_holes), (1, 1));
        // The pads span 4 x 2 mm; the bare footprint is a point.
        assert!((stats.components_area_mm2 - 8.0).abs() < 1e-9);
        assert!((stats.density().unwrap() - 0.004).abs() < 1e-12);
    }

    #[test]
    fn no_outline_has_no_area_or_density() {
        let mut b = board();
        b.outline.truncate(2);
        let stats = BoardStats::of(&b);
        assert_eq!(stats.area_mm2, None);
        assert_eq!(stats.density(), None);
    }
}
