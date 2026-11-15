//! The track ends that sit on the pads of footprints being moved, so the
//! move takes them along (#2379): the drop and the drag preview read the
//! same answer.
//!
//! An end follows when it lies on the pad's copper, on a copper layer the
//! pad reaches, and the track is on the pad's net (no net counts as 0): the
//! same contact the ratsnest treats as connected. Only that end moves; the
//! other end stays, so whatever it joins stays joined.

use std::collections::BTreeSet;

use signex_types::pcb::{PcbBoard, Point, Segment};
use uuid::Uuid;

use crate::copper::{layer_mask, pad_layers};
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
}

/// A pad of a moved footprint: its copper, layers and net.
struct MovedPad {
    shape: Shape,
    layers: u32,
    net: u32,
}

/// The tracks of `board` with an end on a pad of one of `footprints`, in
/// board order. Geometry that does not convert is left out.
pub fn tracks_on_pads(board: &PcbBoard, footprints: &BTreeSet<Uuid>) -> Vec<TrackEnds> {
    let pads = moved_pads(board, footprints);
    if pads.is_empty() {
        return Vec::new();
    }
    board
        .segments
        .iter()
        .enumerate()
        .filter_map(|(index, segment)| {
            let layers = layer_mask(std::iter::once(segment.layer.as_str()));
            if layers == 0 {
                return None;
            }
            let on_pad = |p: Point| {
                PointNm::from_mm(p, "track end").is_ok_and(|p| {
                    pads.iter().any(|pad| {
                        pad.net == segment.net
                            && pad.layers & layers != 0
                            && pad.shape.contains(p, 0)
                    })
                })
            };
            let (start, end) = (on_pad(segment.start), on_pad(segment.end));
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
