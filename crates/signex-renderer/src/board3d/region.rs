//! Exact areas for the 3D board: rings in the 3D frame as `pcb_geom`
//! polygons, and those polygons filled into the mesh. A fill with many
//! holes is cut in half until each piece fills, so one awkward opening
//! cannot close the others (a refused fill would cover them).

use pcb_geom::{PointNm, Polygon, intersection, mm_to_nm, nm_to_mm};
use signex_gfx::ring_fill::triangulate_with_holes;
use signex_gfx::scene3d::Scene3d;

/// The most holes one ear clip takes; a piece with more is cut in half.
const MAX_HOLES_PER_FILL: usize = 48;
/// How many times a piece may be cut before a refused fill is left out.
const MAX_CUTS: u8 = 14;

fn to_nm(p: [f32; 2]) -> Option<PointNm> {
    Some(PointNm::new(
        mm_to_nm(f64::from(p[0]), "board3d.x").ok()?,
        mm_to_nm(f64::from(p[1]), "board3d.y").ok()?,
    ))
}

fn ring_nm(ring: &[[f32; 2]]) -> Option<Vec<PointNm>> {
    ring.iter().map(|&p| to_nm(p)).collect()
}

fn ring_mm(ring: &[PointNm]) -> Vec<[f32; 2]> {
    ring.iter()
        .map(|p| [nm_to_mm(p.x) as f32, nm_to_mm(p.y) as f32])
        .collect()
}

/// `outer` less `holes`, both in the 3D frame; `None` for an outer ring
/// under 3 points or a point the geometry cannot hold. A hole under 3
/// points is skipped.
pub(super) fn polygon(outer: &[[f32; 2]], holes: &[Vec<[f32; 2]>]) -> Option<Polygon> {
    if outer.len() < 3 {
        return None;
    }
    let holes = holes
        .iter()
        .filter(|hole| hole.len() >= 3)
        .map(|hole| ring_nm(hole))
        .collect::<Option<Vec<_>>>()?;
    Some(Polygon::new(ring_nm(outer)?, holes))
}

/// A polygon in board nm (y down) in the 3D frame (y up).
pub(super) fn in_view_frame(polygon: &Polygon) -> Polygon {
    let flip = |ring: &[PointNm]| -> Vec<PointNm> {
        ring.iter().map(|p| PointNm::new(p.x, -p.y)).collect()
    };
    Polygon::new(
        flip(&polygon.outer),
        polygon.holes.iter().map(|hole| flip(hole)).collect(),
    )
}

/// Fill `polygons` at height `z` in `color`.
pub(super) fn append_polygons(scene: &mut Scene3d, polygons: &[Polygon], z: f32, color: [f32; 4]) {
    for polygon in polygons {
        append_polygon(scene, polygon, z, color, 0);
    }
}

fn append_polygon(scene: &mut Scene3d, polygon: &Polygon, z: f32, color: [f32; 4], cuts: u8) {
    if polygon.outer.len() < 3 {
        return;
    }
    let outer = ring_mm(&polygon.outer);
    if polygon.holes.is_empty() {
        scene.append_face(&outer, &[], z, color);
        return;
    }
    if polygon.holes.len() <= MAX_HOLES_PER_FILL {
        let holes: Vec<Vec<[f32; 2]>> = polygon.holes.iter().map(|h| ring_mm(h)).collect();
        if let Some((points, triangles)) = triangulate_with_holes(&outer, &holes) {
            let positions: Vec<[f32; 3]> = points.iter().map(|p| [p[0], p[1], z]).collect();
            scene.append(&positions, &triangles, color);
            return;
        }
    }
    if cuts >= MAX_CUTS {
        return;
    }
    for half in halves(polygon).into_iter().flatten() {
        for piece in intersection(std::slice::from_ref(polygon), std::slice::from_ref(&half)) {
            append_polygon(scene, &piece, z, color, cuts + 1);
        }
    }
}

/// The two halves of `polygon`'s box, cut across its longer side; `None`
/// for a box too small to cut.
fn halves(polygon: &Polygon) -> Option<[Polygon; 2]> {
    let first = polygon.outer.first()?;
    let (mut x0, mut x1, mut y0, mut y1) = (first.x, first.x, first.y, first.y);
    for p in &polygon.outer {
        (x0, x1) = (x0.min(p.x), x1.max(p.x));
        (y0, y1) = (y0.min(p.y), y1.max(p.y));
    }
    let rect = |xa: i64, ya: i64, xb: i64, yb: i64| {
        Polygon::new(
            vec![
                PointNm::new(xa, ya),
                PointNm::new(xb, ya),
                PointNm::new(xb, yb),
                PointNm::new(xa, yb),
            ],
            Vec::new(),
        )
    };
    if x1 - x0 >= y1 - y0 {
        let cut = x0 + (x1 - x0) / 2;
        (cut > x0).then(|| [rect(x0, y0, cut, y1), rect(cut, y0, x1, y1)])
    } else {
        let cut = y0 + (y1 - y0) / 2;
        (cut > y0).then(|| [rect(x0, y0, x1, cut), rect(x0, cut, x1, y1)])
    }
}
