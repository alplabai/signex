//! A routed track that runs along a same-net track already laid keeps only
//! the stretches that track does not cover (#2492): the copper there is the
//! net's already, so a second track over it is duplicate copper. The pieces
//! left end exactly on the covering track's ends, so they stay joined to it.
//! A track covers only when it is at least as wide: a narrower one does not
//! carry the copper the route asked for.

use signex_types::pcb::{PcbBoard, Point, Segment};
use uuid::Uuid;

/// How far off a track's line a point may sit and still be on it, and the
/// shortest stretch that counts as shared, in mm.
const ON_LINE_MM: f64 = 1e-3;

/// A stretch of a track, as distances along it from its start, with the
/// points that bound it.
#[derive(Clone, Copy)]
struct Stretch {
    from: f64,
    from_at: Point,
    to: f64,
    to_at: Point,
}

/// What [`uncovered`] left of a track, and the laid tracks that covered
/// the rest.
pub(super) struct Uncovered<'a> {
    pub(super) pieces: Vec<Segment>,
    pub(super) covers: Vec<&'a Segment>,
}

/// `track` without the stretches that `laid` tracks of its net and layer,
/// at least as wide, cover (on its line and sharing more than
/// [`ON_LINE_MM`]): the pieces left, in order from its start. The first
/// piece keeps the track's id; `ids` names the others. No pieces when
/// `laid` covers all of it; `track` itself when nothing covers it. `laid`
/// must not hold `track` itself (ids are not compared: a preview names
/// every track nil).
pub(super) fn uncovered<'a>(
    track: Segment,
    laid: impl IntoIterator<Item = &'a Segment>,
    ids: &mut impl FnMut() -> Uuid,
) -> Uncovered<'a> {
    let (dx, dy) = (track.end.x - track.start.x, track.end.y - track.start.y);
    let length = dx.hypot(dy);
    if length <= ON_LINE_MM {
        return Uncovered {
            pieces: vec![track],
            covers: Vec::new(),
        };
    }
    let off = |p: Point| (dx * (p.y - track.start.y) - dy * (p.x - track.start.x)) / length;
    let along = |p: Point| (dx * (p.x - track.start.x) + dy * (p.y - track.start.y)) / length;
    // A covered stretch is bounded by the covering track's end, or by the
    // track's own end where the cover runs past it.
    let bound = |p: Point| match along(p) {
        t if t <= 0.0 => (0.0, track.start),
        t if t >= length => (length, track.end),
        t => (t, p),
    };
    let (mut covered, covers): (Vec<Stretch>, Vec<&Segment>) = laid
        .into_iter()
        .filter(|s| s.net == track.net && s.layer == track.layer && s.width >= track.width)
        .filter(|s| off(s.start).abs() <= ON_LINE_MM && off(s.end).abs() <= ON_LINE_MM)
        .filter_map(|s| {
            let (a, b) = (bound(s.start), bound(s.end));
            let ((from, from_at), (to, to_at)) = if a.0 <= b.0 { (a, b) } else { (b, a) };
            let stretch = Stretch {
                from,
                from_at,
                to,
                to_at,
            };
            (to - from > ON_LINE_MM).then_some((stretch, s))
        })
        .unzip();
    if covered.is_empty() {
        return Uncovered {
            pieces: vec![track],
            covers,
        };
    }
    covered.sort_by(|a, b| a.from.total_cmp(&b.from));
    let mut pieces = Vec::new();
    let (mut done, mut done_at) = (0.0, track.start);
    for stretch in covered {
        if stretch.from - done > ON_LINE_MM {
            pieces.push((done_at, stretch.from_at));
        }
        if stretch.to > done {
            (done, done_at) = (stretch.to, stretch.to_at);
        }
    }
    if length - done > ON_LINE_MM {
        pieces.push((done_at, track.end));
    }
    let pieces = pieces
        .into_iter()
        .enumerate()
        .map(|(index, (start, end))| Segment {
            uuid: if index == 0 { track.uuid } else { ids() },
            start,
            end,
            ..track.clone()
        })
        .collect();
    Uncovered { pieces, covers }
}

/// Add to `held` the covers among `covers` that are `board` tracks as it
/// holds them now, once each: what a route trimmed against the board needs
/// still there when it lands.
pub(super) fn hold_board_covers(held: &mut Vec<Segment>, board: &PcbBoard, covers: &[&Segment]) {
    for cover in covers {
        if board.segments.contains(cover) && !held.iter().any(|h| h.uuid == cover.uuid) {
            held.push((*cover).clone());
        }
    }
}
