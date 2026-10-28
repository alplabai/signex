//! Outline-following courtyard: the union of the pads, grown by the
//! courtyard slack.

use signex_sketch::geom::{
    Aabb, BoolOp, CornerStyle, Point2, offset_polygon, polygon_op, signed_area,
};

use super::{COURTYARD_SLACK_MM, FootprintEditorState};

/// Extra growth per pad index, 1 nm each. Equal-sized neighbours would share
/// a collinear edge after the grow, which the boolean refuses. The result
/// depends on pad order at the nanometre level, and two pads whose offsets
/// still coincide exactly stay unmerged and are named by `PadsLeftOut`.
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
    /// Position, size and rotation of every pad: what the outline depends on.
    fn pad_geometry(&self) -> Vec<[f64; 5]> {
        self.pads
            .iter()
            .map(|p| {
                [
                    p.position_mm.0,
                    p.position_mm.1,
                    p.size_mm.0,
                    p.size_mm.1,
                    p.rotation_deg,
                ]
            })
            .collect()
    }

    /// The failure naming every pad not in `covered`.
    fn left_out_error(&self, covered: &[usize]) -> CourtyardOutlineError {
        let mut is_covered = vec![false; self.pads.len()];
        for &i in covered {
            is_covered[i] = true;
        }
        CourtyardOutlineError::PadsLeftOut(
            self.pads
                .iter()
                .zip(is_covered)
                .filter(|(_, covered)| !covered)
                .map(|(pad, _)| pad.number.clone())
                .collect(),
        )
    }

    /// The outline, unless a pad was added, removed, moved, resized or
    /// turned since it was computed.
    pub fn courtyard_outline(&self) -> Option<&[(f64, f64)]> {
        if self.pad_geometry() != self.courtyard_outline_pads {
            return None;
        }
        self.courtyard_outline_mm.as_deref()
    }

    /// Build the outline-following courtyard into `courtyard_outline_mm`.
    ///
    /// Each pad is grown by `COURTYARD_SLACK_MM` BEFORE the union, so
    /// neighbours that abut end up overlapping and the union is
    /// well-defined. Returns an error, with the outline cleared, when there
    /// are no pads or when the single outline polygon leaves a pad out.
    pub fn recompute_courtyard_outline(&mut self) -> Result<(), CourtyardOutlineError> {
        self.courtyard_outline_mm = None;
        self.courtyard_outline_pads = self.pad_geometry();
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

        let pieces = union_pieces(grown.into_iter().enumerate().map(|(i, ring)| Piece {
            ring,
            pads: vec![i],
        }));
        let Some(biggest) = pieces.into_iter().max_by(|a, b| {
            signed_area(&a.ring)
                .abs()
                .total_cmp(&signed_area(&b.ring).abs())
        }) else {
            return Err(self.left_out_error(&[]));
        };
        let outline = biggest.ring;
        let left_out = self.left_out_error(&biggest.pads);
        if biggest.pads.len() < self.pads.len() {
            return Err(left_out);
        }
        self.courtyard_outline_mm = Some(outline.iter().map(|p| (p.x, p.y)).collect());
        Ok(())
    }
}

/// A ring and the pads merged into it.
struct Piece {
    ring: Vec<Point2>,
    pads: Vec<usize>,
}

/// Union the pieces, merging whatever the boolean accepts; a pair it
/// refuses or that is disjoint stays separate. The work is split by
/// position (halve along the longer axis, union each half, then join), so
/// the boolean mostly sees compact rings with short perimeters instead of
/// one ever-growing ring: a dense pad grid stays fast.
fn union_pieces(pieces: impl IntoIterator<Item = Piece>) -> Vec<Piece> {
    let mut items: Vec<Piece> = pieces.into_iter().filter(|p| p.ring.len() >= 3).collect();
    if items.len() <= 1 {
        return items;
    }
    let span = |axis: fn(&Point2) -> f64| {
        let (lo, hi) = items
            .iter()
            .flat_map(|p| p.ring.iter())
            .map(axis)
            .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
        hi - lo
    };
    let along_x = span(|p| p.x) >= span(|p| p.y);
    let key = |p: &Piece| {
        let n = p.ring.len() as f64;
        let sum: f64 = p.ring.iter().map(|q| if along_x { q.x } else { q.y }).sum();
        sum / n
    };
    items.sort_by(|a, b| key(a).total_cmp(&key(b)));
    let right = items.split_off(items.len() / 2);
    let mut joined = union_pieces(items);
    for piece in union_pieces(right) {
        joined = add_piece(joined, piece);
    }
    joined
}

/// Merge `next` into every piece of `acc` it can be merged with.
fn add_piece(acc: Vec<Piece>, next: Piece) -> Vec<Piece> {
    let mut merged = next;
    let mut rest = Vec::with_capacity(acc.len());
    for piece in acc {
        if !boxes_overlap(&piece.ring, &merged.ring) {
            rest.push(piece);
            continue;
        }
        match polygon_op(&piece.ring, &merged.ring, BoolOp::Union) {
            Ok(parts) => {
                // Outer rings wind CCW; a CW ring is a hole of the union.
                let mut outers: Vec<Vec<Point2>> =
                    parts.into_iter().filter(|p| signed_area(p) > 0.0).collect();
                if outers.len() == 1 {
                    let mut pads = piece.pads;
                    pads.extend(merged.pads);
                    merged = Piece {
                        ring: outers.remove(0),
                        pads,
                    };
                } else {
                    rest.push(piece);
                }
            }
            Err(_) => rest.push(piece),
        }
    }
    rest.push(merged);
    rest
}

fn boxes_overlap(a: &[Point2], b: &[Point2]) -> bool {
    match (Aabb::from_points(a), Aabb::from_points(b)) {
        (Some(x), Some(y)) => x.overlaps(y),
        _ => true,
    }
}
