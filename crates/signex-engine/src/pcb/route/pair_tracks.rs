//! The tracks of a differential pair that run side by side (#2585): for a
//! track of one net, the track of the other net coupled to it, and the gap
//! between their copper. The Track panel shows the gap and the paired net,
//! and a width edit of a pair track reaches its partner too.
//!
//! Two tracks are coupled when they sit on the same layer, run parallel
//! and overlap along their length; of several, the nearest is the partner.

use signex_types::pcb::{PcbBoard, Segment};
use uuid::Uuid;

use super::pair::PairNets;

/// Sine of the largest angle between two tracks still read as parallel. A
/// coupled pair is drawn parallel; this only absorbs float rounding.
const PARALLEL_SINE: f64 = 1.0e-6;

/// A track's partner on the other net of `pair` and the copper gap between
/// them in mm (edge to edge; below zero when they overlap). `None` for a
/// track on neither net, or with no parallel track of the other net beside
/// it.
pub fn coupled_partner<'a>(
    board: &'a PcbBoard,
    pair: PairNets,
    track: &Segment,
) -> Option<(&'a Segment, f64)> {
    let other = if track.net == pair.positive {
        pair.negative
    } else if track.net == pair.negative {
        pair.positive
    } else {
        return None;
    };
    board
        .segments
        .iter()
        .filter(|s| s.net == other && s.layer == track.layer)
        .filter_map(|s| centre_distance(track, s).map(|d| (s, d - (track.width + s.width) / 2.0)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// `tracks` and every partner of theirs, each once, in board order: what
/// an edit of the pair's tracks reaches.
pub fn with_partners(board: &PcbBoard, pair: PairNets, tracks: &[Uuid]) -> Vec<Uuid> {
    let mut reached: Vec<Uuid> = tracks.to_vec();
    for segment in board.segments.iter().filter(|s| tracks.contains(&s.uuid)) {
        if let Some((partner, _)) = coupled_partner(board, pair, segment)
            && !reached.contains(&partner.uuid)
        {
            reached.push(partner.uuid);
        }
    }
    board
        .segments
        .iter()
        .map(|s| s.uuid)
        .filter(|id| reached.contains(id))
        .collect()
}

/// The gap every track of `tracks` keeps to its partner, in mm; `None` when
/// one has no partner or the gaps differ (by more than a nanometre).
pub fn shared_gap_mm(board: &PcbBoard, pair: PairNets, tracks: &[Uuid]) -> Option<f64> {
    let mut gaps = board
        .segments
        .iter()
        .filter(|s| tracks.contains(&s.uuid))
        .map(|s| coupled_partner(board, pair, s).map(|(_, gap)| gap));
    let first = gaps.next()??;
    for gap in gaps {
        if (gap? - first).abs() > 1.0e-6 {
            return None;
        }
    }
    Some(first)
}

/// The distance between the centre lines of `a` and `b` in mm when they
/// run parallel and overlap along their length; `None` otherwise.
fn centre_distance(a: &Segment, b: &Segment) -> Option<f64> {
    let (ax, ay) = (a.end.x - a.start.x, a.end.y - a.start.y);
    let length = ax.hypot(ay);
    let (bx, by) = (b.end.x - b.start.x, b.end.y - b.start.y);
    let other = bx.hypot(by);
    if length.is_nan() || other.is_nan() || length <= 0.0 || other <= 0.0 {
        return None;
    }
    let (ux, uy) = (ax / length, ay / length);
    let sine = (ux * by - uy * bx) / other;
    if sine.is_nan() || sine.abs() > PARALLEL_SINE {
        return None;
    }
    // Where b's ends fall along a, from a's start.
    let along = |x: f64, y: f64| (x - a.start.x) * ux + (y - a.start.y) * uy;
    let (t0, t1) = (along(b.start.x, b.start.y), along(b.end.x, b.end.y));
    let overlap = t0.max(t1).min(length) - t0.min(t1).max(0.0);
    if overlap.is_nan() || overlap <= 0.0 {
        return None;
    }
    let (dx, dy) = (b.start.x - a.start.x, b.start.y - a.start.y);
    Some((ux * dy - uy * dx).abs())
}
