//! The track ends that sit on the pads of footprints, or on vias, being
//! moved, so the move takes them along (#2379): the drop and the drag preview
//! read the same answer.
//!
//! An end follows when it lies on the pad's copper, on a copper layer the
//! pad reaches, and the track is on the pad's net (no net counts as 0): the
//! same contact the ratsnest treats as connected. Only that end moves; the
//! other end stays, so whatever it joins stays joined. On a via an end
//! follows only at the via's exact position, and on a dragged track only at
//! one of its ends (#2513).

use std::collections::BTreeSet;

use signex_types::corner::{CornerMode, CornerRule};
use signex_types::pcb::{PcbBoard, Point, Segment};
use signex_types::pcb_arc::TrackArc;
use uuid::Uuid;

use crate::copper::{layer_mask, pad_layers, via_layers};
use crate::free::arc_is_copper;
use crate::shape::Shape;
use crate::transform::pad_shape;
use crate::units::PointNm;

/// A track with an end on a moved pad, and which of its ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackEnds {
    pub track: Uuid,
    /// Its position in `PcbBoard::segments`.
    pub index: usize,
    pub start: bool,
    pub end: bool,
}

impl TrackEnds {
    /// `segment` with the ends on the moved pads shifted by `delta` mm, the
    /// offset the footprints move by.
    pub fn follow(&self, segment: &Segment, delta: Point) -> Segment {
        let shift = |p: Point| Point::new(p.x + delta.x, p.y + delta.y);
        let mut moved = segment.clone();
        if self.start {
            moved.start = shift(segment.start);
        }
        if self.end {
            moved.end = shift(segment.end);
        }
        moved
    }

    /// [`Self::follow`] for a track stretched by a dragged track (#2513),
    /// kept at 0°, 45° or 90°. With one end following and the moved end off
    /// those lines from the fixed end, the track takes `rule`'s two legs from
    /// the fixed end: it keeps the leg at its fixed end, and the leg to the
    /// moved end is the second track returned, with the uuid `added` and the
    /// rest of the track's fields. One leg, or both ends or none following:
    /// the plain follow.
    pub fn stretch(
        &self,
        segment: &Segment,
        delta: Point,
        rule: CornerRule,
        added: Uuid,
    ) -> (Segment, Option<Segment>) {
        let moved = self.follow(segment, delta);
        if self.start == self.end {
            return (moved, None);
        }
        let (fixed, free) = if self.start {
            (moved.end, moved.start)
        } else {
            (moved.start, moved.end)
        };
        let &[(_, corner), _] = rule.legs(fixed, free).as_slice() else {
            return (moved, None);
        };
        let (mut kept, mut leg) = (moved.clone(), moved);
        leg.uuid = added;
        if self.start {
            // Fixed at the end: the corner to the end, the moved start to
            // the corner.
            kept.start = corner;
            leg.end = corner;
        } else {
            kept.end = corner;
            leg.start = corner;
        }
        (kept, Some(leg))
    }
}

/// The corner rule a stretched track bends by (#2513): the router's rule
/// `live` when it bends at 45°, else a straight leg out of the fixed end
/// then the 45° leg to the moved end, so the segments stay at 0°, 45° or 90°.
pub fn stretch_rule(live: CornerRule) -> CornerRule {
    match live.mode {
        CornerMode::Angle45Start | CornerMode::Angle45End => live,
        CornerMode::Ortho90 | CornerMode::FreeAngle => CornerRule {
            mode: CornerMode::Angle45End,
            is_flipped: false,
        },
    }
}

/// A copper arc with an end on a moved via, and which of its ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArcEnds {
    pub arc: Uuid,
    /// Its position in `PcbBoard::free.arcs`.
    pub index: usize,
    pub start: bool,
    pub end: bool,
}

impl ArcEnds {
    /// `arc` with the named ends shifted by `delta` mm. With both ends the
    /// whole arc shifts. With one, the arc is turned and scaled about its
    /// other end so that end stays and the arc keeps its bend and sweep.
    pub fn follow(&self, arc: &TrackArc, delta: Point) -> TrackArc {
        let shift = |p: Point| Point::new(p.x + delta.x, p.y + delta.y);
        let mut moved = arc.clone();
        let (fixed, old, new) = match (self.start, self.end) {
            (true, true) => {
                moved.start = shift(arc.start);
                moved.mid = shift(arc.mid);
                moved.end = shift(arc.end);
                return moved;
            }
            (true, false) => (arc.end, arc.start, shift(arc.start)),
            (false, true) => (arc.start, arc.end, shift(arc.end)),
            (false, false) => return moved,
        };
        let (ox, oy) = (old.x - fixed.x, old.y - fixed.y);
        let (nx, ny) = (new.x - fixed.x, new.y - fixed.y);
        let chord_sq = ox * ox + oy * oy;
        if chord_sq <= 0.0 {
            return moved;
        }
        // The turn and scale taking the old chord onto the new one.
        let (c, s) = (
            (nx * ox + ny * oy) / chord_sq,
            (ny * ox - nx * oy) / chord_sq,
        );
        let (mx, my) = (arc.mid.x - fixed.x, arc.mid.y - fixed.y);
        moved.mid = Point::new(fixed.x + c * mx - s * my, fixed.y + c * my + s * mx);
        if self.start {
            moved.start = new;
        } else {
            moved.end = new;
        }
        moved
    }

    /// [`Self::follow`], refused when one end moving would collapse the arc
    /// (two of its points on one nanometre) or scale it past
    /// [`ARC_FOLLOW_MAX_GROWTH`]. An arc shifted whole is never refused.
    pub fn checked_follow(&self, arc: &TrackArc, delta: Point) -> Result<TrackArc, ArcFollowFault> {
        let moved = self.follow(arc, delta);
        if self.start == self.end {
            return Ok(moved);
        }
        let nm = |p: Point| PointNm::from_mm(p, "arc point").map_err(|_| ArcFollowFault::Collapses);
        let (start, mid, end) = (nm(moved.start)?, nm(moved.mid)?, nm(moved.end)?);
        if start == end || mid == start || mid == end {
            return Err(ArcFollowFault::Collapses);
        }
        // Turning and scaling about one end scales the radius as the chord.
        let chord = |a: &TrackArc| (a.end.x - a.start.x).hypot(a.end.y - a.start.y);
        if chord(&moved) > ARC_FOLLOW_MAX_GROWTH * chord(arc) {
            return Err(ArcFollowFault::GrowsTooMuch);
        }
        Ok(moved)
    }
}

/// How many times its radius an arc may grow when one end follows a via.
pub const ARC_FOLLOW_MAX_GROWTH: f64 = 3.0;

/// Why an arc cannot follow a moved via.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcFollowFault {
    /// Its moved end would land on its fixed end, or its middle on an end.
    Collapses,
    /// It would grow past [`ARC_FOLLOW_MAX_GROWTH`] times its radius.
    GrowsTooMuch,
}

/// A pad of a moved footprint: its copper, layers and net.
struct MovedPad {
    shape: Shape,
    layers: u32,
    net: u32,
}

/// The tracks of `board` with an end on a pad of one of `footprints`, in
/// board order. Geometry that does not convert is left out, and so is a
/// locked track (#2407), which stays where it is.
pub fn tracks_on_pads(board: &PcbBoard, footprints: &BTreeSet<Uuid>) -> Vec<TrackEnds> {
    let pads = moved_pads(board, footprints);
    if pads.is_empty() {
        return Vec::new();
    }
    ends_where(board, |net, layers, p| {
        pads.iter()
            .any(|pad| pad.net == net && pad.layers & layers != 0 && pad.shape.contains(p, 0))
    })
}

/// The tracks of `board` with an end at the position of one of `vias`, to
/// the nanometre, on a layer the via reaches and on its net, in board order.
/// An end elsewhere on the via's copper stays, so a short stub keeps the pad
/// it joins. A locked track is left out, as for [`tracks_on_pads`].
pub fn tracks_on_vias(board: &PcbBoard, vias: &BTreeSet<Uuid>) -> Vec<TrackEnds> {
    let spots = via_spots(board, vias);
    if spots.is_empty() {
        return Vec::new();
    }
    ends_where(board, |net, layers, p| at_spot(&spots, net, layers, p))
}

/// The tracks of `board` other than `tracks` with an end on an end of one of
/// `tracks`, to the nanometre, on its layer and net, in board order (#2513):
/// a dragged track stretches the tracks joined to it. Only the joined end
/// follows, so their other end stays, on a pad or wherever it is. A locked
/// track is left out, as for [`tracks_on_pads`].
pub fn tracks_on_tracks(board: &PcbBoard, tracks: &BTreeSet<Uuid>) -> Vec<TrackEnds> {
    let spots: Vec<(PointNm, u32, u32)> = (board.segments.iter())
        .filter(|segment| tracks.contains(&segment.uuid))
        .flat_map(|segment| {
            let layers = layer_mask(std::iter::once(segment.layer.as_str()));
            [segment.start, segment.end]
                .into_iter()
                .filter_map(move |p| {
                    let at = PointNm::from_mm(p, "track end").ok()?;
                    Some((at, layers, segment.net))
                })
        })
        .collect();
    if spots.is_empty() {
        return Vec::new();
    }
    let mut ends = ends_where(board, |net, layers, p| at_spot(&spots, net, layers, p));
    ends.retain(|e| !tracks.contains(&e.track));
    ends
}

/// The copper arcs of `board` with an end at one of `vias`, by the rule of
/// [`tracks_on_vias`], in board order.
pub fn arcs_on_vias(board: &PcbBoard, vias: &BTreeSet<Uuid>) -> Vec<ArcEnds> {
    let spots = via_spots(board, vias);
    if spots.is_empty() {
        return Vec::new();
    }
    board
        .free
        .arcs
        .iter()
        .enumerate()
        .filter(|(_, arc)| arc_is_copper(arc))
        .filter_map(|(index, arc)| {
            let layers = layer_mask(std::iter::once(arc.layer.as_str()));
            let on = |p: Point| {
                PointNm::from_mm(p, "arc end").is_ok_and(|p| at_spot(&spots, arc.net, layers, p))
            };
            let (start, end) = (on(arc.start), on(arc.end));
            (start || end).then_some(ArcEnds {
                arc: arc.uuid,
                index,
                start,
                end,
            })
        })
        .collect()
}

/// Where each of `vias` is, the layers it reaches and its net.
fn via_spots(board: &PcbBoard, vias: &BTreeSet<Uuid>) -> Vec<(PointNm, u32, u32)> {
    board
        .vias
        .iter()
        .filter(|via| vias.contains(&via.uuid))
        .filter_map(|via| {
            let at = PointNm::from_mm(via.position, "via").ok()?;
            Some((at, via_layers(via), via.net))
        })
        .collect()
}

/// Whether `p` is exactly on one of `spots` (a point, the layers it reaches
/// and its net), on a shared layer and on its net.
fn at_spot(spots: &[(PointNm, u32, u32)], net: u32, layers: u32, p: PointNm) -> bool {
    spots
        .iter()
        .any(|(at, reach, spot_net)| *spot_net == net && reach & layers != 0 && *at == p)
}

/// The unlocked tracks with an end `touches` accepts, given the track's net,
/// its layer mask and the end.
fn ends_where(board: &PcbBoard, touches: impl Fn(u32, u32, PointNm) -> bool) -> Vec<TrackEnds> {
    board
        .segments
        .iter()
        .enumerate()
        .filter_map(|(index, segment)| {
            let layers = layer_mask(std::iter::once(segment.layer.as_str()));
            if layers == 0 || segment.locked {
                return None;
            }
            let on = |p: Point| {
                PointNm::from_mm(p, "track end").is_ok_and(|p| touches(segment.net, layers, p))
            };
            let (start, end) = (on(segment.start), on(segment.end));
            (start || end).then_some(TrackEnds {
                track: segment.uuid,
                index,
                start,
                end,
            })
        })
        .collect()
}

fn moved_pads(board: &PcbBoard, footprints: &BTreeSet<Uuid>) -> Vec<MovedPad> {
    board
        .footprints
        .iter()
        .filter(|fp| footprints.contains(&fp.uuid))
        .flat_map(|fp| fp.pads.iter().map(move |pad| (fp, pad)))
        .filter_map(|(fp, pad)| {
            Some(MovedPad {
                shape: pad_shape(fp, pad).ok()?,
                layers: pad_layers(fp, pad),
                net: pad.net.as_ref().map_or(0, |n| n.number),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "track_follow_tests.rs"]
mod tests;
