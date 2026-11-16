//! Copy board objects and paste them onto a board (#2361).
//!
//! Pure: [`BoardClipboard::copy`] reads a board, and
//! [`BoardClipboard::paste_command`] returns the one `AddItems` command a
//! paste runs as one undo step. Geometry is held relative to the copy's
//! origin; nets are held by name, so a paste finds them on any board.

use std::collections::{BTreeMap, HashMap};

use signex_types::pcb::{Footprint, PadNet, PcbBoard, Point, Segment, Via};
use signex_types::pcb_options::GridUse;
use uuid::Uuid;

use super::command::BoardCommand;
use super::error::BoardCommandError;
use super::footprint_content::sync_text_properties;
use super::items::{BoardItem, BoardItemId};
use super::placement::snap_delta;

/// Footprints (with their pads), tracks and vias copied from a board.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardClipboard {
    footprints: Vec<Footprint>,
    segments: Vec<Segment>,
    vias: Vec<Via>,
    /// The name of each net number the tracks and vias carry on the board
    /// they were copied from. A pad carries its net's name itself.
    net_names: BTreeMap<u32, String>,
    /// Where the copy sat on its board: the centre of its items, on the grid.
    origin: Point,
}

impl BoardClipboard {
    /// Copy `items` of `board`. The origin is the centre of the items'
    /// anchors (footprint origins, track ends, via centres) put on the
    /// `grid_mm` grid, so items on the grid stay on it when pasted. Zones
    /// and texts are not carried yet. `None` when nothing in `items` is
    /// carried; an id not on the board refuses the copy.
    pub fn copy(
        board: &PcbBoard,
        items: &[BoardItemId],
        grid_mm: f64,
    ) -> Result<Option<Self>, BoardCommandError> {
        let mut copy = Self {
            footprints: Vec::new(),
            segments: Vec::new(),
            vias: Vec::new(),
            net_names: BTreeMap::new(),
            origin: Point::new(0.0, 0.0),
        };
        for &id in items {
            let missing = BoardCommandError::ItemNotFound(id);
            match id {
                BoardItemId::Footprint(u) => copy.footprints.push(
                    board
                        .footprints
                        .iter()
                        .find(|x| x.uuid == u)
                        .ok_or(missing)?
                        .clone(),
                ),
                BoardItemId::Segment(u) => copy.segments.push(
                    board
                        .segments
                        .iter()
                        .find(|x| x.uuid == u)
                        .ok_or(missing)?
                        .clone(),
                ),
                BoardItemId::Via(u) => copy.vias.push(
                    board
                        .vias
                        .iter()
                        .find(|x| x.uuid == u)
                        .ok_or(missing)?
                        .clone(),
                ),
                BoardItemId::Zone(_) | BoardItemId::Text(_) | BoardItemId::Graphic(_) => {}
            }
        }
        let Some(centre) = copy.anchor_centre() else {
            return Ok(None);
        };
        let snapped = snap_delta(Point::new(0.0, 0.0), centre, grid_mm);
        copy.origin = if snapped.x.is_finite() && snapped.y.is_finite() {
            snapped
        } else {
            centre
        };
        copy.translate(Point::new(-copy.origin.x, -copy.origin.y));
        let used: Vec<u32> = copy
            .segments
            .iter()
            .map(|s| s.net)
            .chain(copy.vias.iter().map(|v| v.net))
            .filter(|net| *net != 0)
            .collect();
        copy.net_names = board
            .nets
            .iter()
            .filter(|net| used.contains(&net.number))
            .map(|net| (net.number, net.name.clone()))
            .collect();
        Ok(Some(copy))
    }

    /// Where the copy sat on the board it was copied from.
    pub fn origin(&self) -> Point {
        self.origin
    }

    /// How many items the copy holds (a footprint counts once).
    pub fn len(&self) -> usize {
        self.footprints.len() + self.segments.len() + self.vias.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Which grid a paste of the copy snaps to (#2374): the components
    /// grid when it holds a footprint.
    pub fn grid_use(&self) -> GridUse {
        if self.footprints.is_empty() {
            GridUse::NonComponents
        } else {
            GridUse::Components
        }
    }

    /// The copied items with the origin at (0, 0), on a board that is
    /// otherwise empty and has `like`'s layers and nets: what a preview of
    /// the paste draws.
    pub fn preview_board(&self, like: &PcbBoard) -> PcbBoard {
        PcbBoard {
            uuid: like.uuid,
            version: like.version,
            generator: like.generator.clone(),
            thickness: like.thickness,
            outline: Vec::new(),
            layers: like.layers.clone(),
            setup: like.setup.clone(),
            nets: like.nets.clone(),
            footprints: self.footprints.clone(),
            segments: self.segments.clone(),
            vias: self.vias.clone(),
            zones: Vec::new(),
            graphics: Vec::new(),
            texts: Vec::new(),
            sketch: None,
            options: Default::default(),
            unknown: Default::default(),
        }
    }

    /// The command that adds a copy to `target` with its origin at `at`,
    /// and the ids it adds. Every item and pad takes an id from `new_id`. A
    /// footprint takes the designator `rename` gives it, or keeps its own on
    /// `None`; it keeps its schematic link only when no footprint of
    /// `target` holds that link, so a cut part pasted back stays linked and a
    /// copy of a placed part does not claim its symbol. Copper keeps its net
    /// when `target` has a net of that name, and has none otherwise.
    pub fn paste_command(
        &self,
        target: &PcbBoard,
        at: Point,
        new_id: &mut impl FnMut() -> Uuid,
        rename: &mut impl FnMut(&str) -> Option<String>,
    ) -> (BoardCommand, Vec<BoardItemId>) {
        let nets: HashMap<&str, u32> = target
            .nets
            .iter()
            .map(|net| (net.name.as_str(), net.number))
            .collect();
        let net_of = |number: u32| {
            self.net_names
                .get(&number)
                .and_then(|name| nets.get(name.as_str()))
                .copied()
                .unwrap_or(0)
        };
        let shift = |p: Point| Point::new(p.x + at.x, p.y + at.y);
        let mut items = Vec::with_capacity(self.len());
        for source in &self.footprints {
            let mut fp = source.clone();
            fp.uuid = new_id();
            fp.position = shift(fp.position);
            if let Some(designator) = rename(&fp.reference) {
                fp.reference = designator;
                sync_text_properties(&mut fp);
            }
            let linked_elsewhere = fp.schematic_link.as_ref().is_some_and(|link| {
                target
                    .footprints
                    .iter()
                    .any(|other| other.schematic_link.as_ref() == Some(link))
            });
            if linked_elsewhere {
                fp.schematic_link = None;
            }
            for pad in &mut fp.pads {
                pad.uuid = new_id();
                pad.net = pad.net.take().and_then(|net| {
                    nets.get(net.name.as_str()).map(|&number| PadNet {
                        number,
                        name: net.name,
                    })
                });
            }
            items.push(BoardItem::Footprint(fp));
        }
        for source in &self.segments {
            let mut segment = source.clone();
            segment.uuid = new_id();
            segment.start = shift(segment.start);
            segment.end = shift(segment.end);
            segment.net = net_of(segment.net);
            items.push(BoardItem::Segment(segment));
        }
        for source in &self.vias {
            let mut via = source.clone();
            via.uuid = new_id();
            via.position = shift(via.position);
            via.net = net_of(via.net);
            items.push(BoardItem::Via(via));
        }
        let ids = items.iter().map(BoardItem::id).collect();
        (BoardCommand::AddItems { items }, ids)
    }

    /// The centre of the bounds of the items' anchors; `None` with no items.
    fn anchor_centre(&self) -> Option<Point> {
        let anchors = self
            .footprints
            .iter()
            .map(|fp| fp.position)
            .chain(self.segments.iter().flat_map(|s| [s.start, s.end]))
            .chain(self.vias.iter().map(|v| v.position));
        let mut bounds: Option<(Point, Point)> = None;
        for p in anchors {
            let (lo, hi) = bounds.get_or_insert((p, p));
            *lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            *hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        bounds.map(|(lo, hi)| Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
    }

    fn translate(&mut self, by: Point) {
        let shift = |p: &mut Point| *p = Point::new(p.x + by.x, p.y + by.y);
        self.footprints
            .iter_mut()
            .for_each(|fp| shift(&mut fp.position));
        for segment in &mut self.segments {
            shift(&mut segment.start);
            shift(&mut segment.end);
        }
        self.vias
            .iter_mut()
            .for_each(|via| shift(&mut via.position));
    }
}

#[cfg(test)]
#[path = "clipboard_tests.rs"]
mod tests;
