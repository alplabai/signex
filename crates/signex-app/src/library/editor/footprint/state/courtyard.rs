//! Outline-following courtyard: the union of the pads, grown by the
//! courtyard slack.

use signex_sketch::geom::{
    BoolOp, CornerStyle, Point2, offset_polygon, point_in_polygon, polygon_op, signed_area,
};

use super::{COURTYARD_SLACK_MM, FootprintEditorState};

/// Per-pad growth step that keeps two equal-sized neighbours from sharing
/// a collinear edge after the grow, which the boolean refuses. 1 nm each.
const GROW_PERTURBATION_MM: f64 = 1e-6;

/// Why no courtyard outline was produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CourtyardOutlineError {
    NoPads,
    /// The outline holds one polygon (schema limit), so pads in a separate
    /// island, or ones the union could not merge, are not covered.
    PadsLeftOut(Vec<String>),
}

impl std::fmt::Display for CourtyardOutlineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPads => write!(f, "there are no pads to outline"),
            Self::PadsLeftOut(pads) => write!(
                f,
                "the single outline polygon does not cover pad(s) {}",
                pads.join(", ")
            ),
        }
    }
}

impl FootprintEditorState {
    /// Build the outline-following courtyard into `courtyard_outline_mm`.
    ///
    /// Each pad is grown by `COURTYARD_SLACK_MM` BEFORE the union, so
    /// neighbours that abut end up overlapping and the union is
    /// well-defined. Returns an error, with the outline cleared, when there
    /// are no pads or when the single outline polygon leaves a pad out.
    pub fn recompute_courtyard_outline(&mut self) -> Result<(), CourtyardOutlineError> {
        self.courtyard_outline_mm = None;
        if self.pads.is_empty() {
            return Err(CourtyardOutlineError::NoPads);
        }
        // Rotated corners, so a turned pad's courtyard hugs the copper.
        let pad_polys: Vec<Vec<Point2>> = self
            .pads
            .iter()
            .map(|p| {
                p.rotated_corners_mm()
                    .iter()
                    .map(|&(x, y)| Point2::new(x, y))
                    .collect()
            })
            .collect();
        let grown: Vec<Vec<Point2>> = pad_polys
            .iter()
            .enumerate()
            .map(|(i, poly)| {
                offset_polygon(
                    poly,
                    COURTYARD_SLACK_MM + i as f64 * GROW_PERTURBATION_MM,
                    CornerStyle::Round { arc_segments: 4 },
                )
            })
            .collect();

        let outline = largest_ring(union_rings(&grown));
        let left_out: Vec<String> = self
            .pads
            .iter()
            .zip(&pad_polys)
            .filter(|(_, poly)| !poly.iter().all(|&c| point_in_polygon(c, &outline)))
            .map(|(pad, _)| pad.number.clone())
            .collect();
        if !left_out.is_empty() {
            return Err(CourtyardOutlineError::PadsLeftOut(left_out));
        }
        self.courtyard_outline_mm = Some(outline.iter().map(|p| (p.x, p.y)).collect());
        Ok(())
    }
}

/// Union the rings, merging whatever the boolean accepts. A pair it
/// refuses or that is disjoint stays a separate ring.
fn union_rings(rings: &[Vec<Point2>]) -> Vec<Vec<Point2>> {
    let mut acc: Vec<Vec<Point2>> = Vec::new();
    for next in rings.iter().filter(|r| r.len() >= 3) {
        let mut merged = next.clone();
        let mut rest = Vec::with_capacity(acc.len());
        for ring in acc.drain(..) {
            match polygon_op(&ring, &merged, BoolOp::Union) {
                Ok(parts) => {
                    // Outer rings wind CCW; a CW ring is a hole of the union.
                    let mut outers: Vec<Vec<Point2>> =
                        parts.into_iter().filter(|p| signed_area(p) > 0.0).collect();
                    if outers.len() == 1 {
                        merged = outers.remove(0);
                    } else {
                        rest.push(ring);
                    }
                }
                Err(_) => rest.push(ring),
            }
        }
        rest.push(merged);
        acc = rest;
    }
    acc
}

fn largest_ring(rings: Vec<Vec<Point2>>) -> Vec<Point2> {
    rings
        .into_iter()
        .max_by(|a, b| signed_area(a).abs().total_cmp(&signed_area(b).abs()))
        .unwrap_or_default()
}
