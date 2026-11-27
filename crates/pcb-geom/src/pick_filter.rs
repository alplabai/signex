//! Which kinds of board object a click or a box may pick (#2353): the PCB
//! selection filter. Pure data; the app keeps one for the board editor.

use std::collections::BTreeSet;

use signex_types::pcb::{Point, SOLID_FILL, Zone};
use signex_types::pcb_graphic::BoardGraphicKind;
use signex_types::zone_cutout::POUR_CUTOUT_FILL;
use signex_types::zone_keepout::KEEPOUT_FILL;
use signex_types::zone_pour::POUR_FILL;

use crate::index::ItemRef;

/// One kind of board object the selection filter names. Its serde name is
/// what a saved filter preset stores (#2383).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum PickKind {
    Components,
    Bodies3d,
    Keepouts,
    Tracks,
    Arcs,
    Pads,
    Vias,
    Regions,
    Polygons,
    Fills,
    Texts,
    Rooms,
    Other,
}

impl PickKind {
    /// Every kind, in the order the filter shows them.
    pub const ALL: [PickKind; 13] = [
        PickKind::Components,
        PickKind::Bodies3d,
        PickKind::Keepouts,
        PickKind::Tracks,
        PickKind::Arcs,
        PickKind::Pads,
        PickKind::Vias,
        PickKind::Regions,
        PickKind::Polygons,
        PickKind::Fills,
        PickKind::Texts,
        PickKind::Rooms,
        PickKind::Other,
    ];

    /// The kind `item` is.
    pub fn of(item: ItemRef) -> Self {
        match item {
            ItemRef::Footprint(_) => PickKind::Components,
            ItemRef::Pad { .. } => PickKind::Pads,
            ItemRef::Track(_) => PickKind::Tracks,
            ItemRef::Via(_) => PickKind::Vias,
            ItemRef::Zone { kind, .. } | ItemRef::Graphic { kind, .. } => kind,
            ItemRef::Text(_) | ItemRef::FootprintText { .. } => PickKind::Texts,
            ItemRef::Arc(_) => PickKind::Arcs,
            ItemRef::FreePad(_) => PickKind::Pads,
            // A dimension has no kind of its own in the filter (#2373).
            ItemRef::Dimension(_) => PickKind::Other,
            ItemRef::Room(_) => PickKind::Rooms,
        }
    }

    /// The kind a board drawing is (#2424): a text frame is a text, every
    /// other drawing is Other.
    pub fn of_graphic(kind: BoardGraphicKind) -> Self {
        match kind {
            BoardGraphicKind::TextFrame => PickKind::Texts,
            _ => PickKind::Other,
        }
    }

    /// The kind a drawn area is (#2383): a keepout, a Fill (solid copper on
    /// an upright rectangle, as the Fill tool draws it), a Solid Region or a
    /// pour cutout (#2871, a region holding no copper), or a polygon pour
    /// (#2216). `None` for a zone of a kind no tool draws.
    pub fn of_zone(zone: &Zone) -> Option<Self> {
        match zone.fill_type.as_str() {
            KEEPOUT_FILL => Some(PickKind::Keepouts),
            SOLID_FILL if is_upright_rectangle(&zone.outline) => Some(PickKind::Fills),
            SOLID_FILL | POUR_CUTOUT_FILL => Some(PickKind::Regions),
            POUR_FILL => Some(PickKind::Polygons),
            _ => None,
        }
    }

    /// Whether the selection index holds objects of this kind, so turning
    /// it off changes what a click or box picks.
    pub fn is_pickable(self) -> bool {
        matches!(
            self,
            PickKind::Components
                | PickKind::Tracks
                | PickKind::Pads
                | PickKind::Vias
                | PickKind::Keepouts
                | PickKind::Regions
                | PickKind::Fills
                | PickKind::Arcs
                | PickKind::Texts
                | PickKind::Other
                | PickKind::Polygons
                | PickKind::Rooms
        )
    }
}

/// The kinds a click or box may pick: every kind unless turned off.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PickFilter {
    off: BTreeSet<PickKind>,
}

impl PickFilter {
    pub fn is_on(&self, kind: PickKind) -> bool {
        !self.off.contains(&kind)
    }

    /// Whether a click or box may pick `item`.
    pub fn allows(&self, item: ItemRef) -> bool {
        self.is_on(PickKind::of(item))
    }

    /// Turn `kind` off when on, on when off.
    pub fn toggle(&mut self, kind: PickKind) {
        if !self.off.remove(&kind) {
            self.off.insert(kind);
        }
    }

    pub fn is_all_on(&self) -> bool {
        self.off.is_empty()
    }

    /// Turn every kind on, or every kind off.
    pub fn set_all(&mut self, on: bool) {
        self.off = if on {
            BTreeSet::new()
        } else {
            PickKind::ALL.into_iter().collect()
        };
    }

    /// Turn on exactly `kinds` and every other kind off (a filter preset,
    /// #2383).
    pub fn set_on_kinds(&mut self, kinds: impl IntoIterator<Item = PickKind>) {
        let on: BTreeSet<PickKind> = kinds.into_iter().collect();
        self.off = PickKind::ALL
            .into_iter()
            .filter(|kind| !on.contains(kind))
            .collect();
    }

    /// The kinds that are on, in [`PickKind::ALL`] order.
    pub fn on_kinds(&self) -> impl Iterator<Item = PickKind> + '_ {
        PickKind::ALL.into_iter().filter(|kind| self.is_on(*kind))
    }
}

/// Four corners on two x and two y values: the rectangle the Fill tool draws.
fn is_upright_rectangle(outline: &[Point]) -> bool {
    let [a, b, c, d] = outline else {
        return false;
    };
    let xs = [a.x, b.x, c.x, d.x];
    let ys = [a.y, b.y, c.y, d.y];
    let two = |v: [f64; 4]| {
        let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        lo < hi && v.iter().all(|x| *x == lo || *x == hi)
    };
    // Neighbouring corners share an x or a y; opposite corners share neither.
    let shares = |p: &Point, q: &Point| p.x == q.x || p.y == q.y;
    two(xs) && two(ys) && shares(a, b) && shares(b, c) && shares(c, d) && shares(d, a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn every_kind_starts_on_and_a_toggle_flips_one() {
        let mut filter = PickFilter::default();
        assert!(filter.is_all_on());
        filter.toggle(PickKind::Pads);
        let pad = ItemRef::Pad {
            footprint: Uuid::nil(),
            pad: Uuid::nil(),
        };
        assert!(!filter.allows(pad));
        assert!(filter.allows(ItemRef::Via(Uuid::nil())));
        assert_eq!(filter.on_kinds().count(), PickKind::ALL.len() - 1);
        filter.toggle(PickKind::Pads);
        assert!(filter.is_all_on());
    }

    #[test]
    fn all_off_then_all_on() {
        let mut filter = PickFilter::default();
        filter.set_all(false);
        assert_eq!(filter.on_kinds().count(), 0);
        assert!(!filter.allows(ItemRef::Track(Uuid::nil())));
        filter.set_all(true);
        assert!(filter.is_all_on());
    }

    fn zone(fill: &str, outline: &[(f64, f64)]) -> Zone {
        Zone {
            uuid: Uuid::nil(),
            net: 0,
            net_name: String::new(),
            layer: "TopCopper".into(),
            outline: outline.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            holes: Vec::new(),
            priority: 0,
            fill_type: fill.into(),
            thermal_relief: false,
            thermal_gap: 0.0,
            thermal_width: 0.0,
            clearance: 0.0,
            min_thickness: 0.0,
            min_island_area: 0.0,
            remove_dead_islands: false,
            shelved: false,
            unfilled: false,
            corners: Default::default(),
            corner_size: 0.0,
            fill_mode: Default::default(),
            hatch_width: 0.0,
            hatch_gap: 0.0,
            unknown: Default::default(),
        }
    }

    /// #2383: a keepout, a Fill and a Solid Region are told apart; #2216: a
    /// pour is a polygon.
    #[test]
    fn drawn_areas_have_their_kind() {
        let rect = [(0.0, 0.0), (4.0, 0.0), (4.0, 2.0), (0.0, 2.0)];
        let tri = [(0.0, 0.0), (4.0, 0.0), (2.0, 3.0)];
        let bow = [(0.0, 0.0), (4.0, 2.0), (4.0, 0.0), (0.0, 2.0)];
        assert_eq!(
            PickKind::of_zone(&zone("keepout", &tri)),
            Some(PickKind::Keepouts)
        );
        assert_eq!(
            PickKind::of_zone(&zone("solid", &rect)),
            Some(PickKind::Fills)
        );
        assert_eq!(
            PickKind::of_zone(&zone("solid", &tri)),
            Some(PickKind::Regions)
        );
        assert_eq!(
            PickKind::of_zone(&zone("solid", &bow)),
            Some(PickKind::Regions)
        );
        assert_eq!(
            PickKind::of_zone(&zone("pour", &rect)),
            Some(PickKind::Polygons),
            "#2216: a pour is picked as a polygon"
        );
        assert_eq!(PickKind::of_zone(&zone("", &rect)), None);
        for kind in [
            PickKind::Keepouts,
            PickKind::Fills,
            PickKind::Regions,
            PickKind::Polygons,
            // #2457: rooms are held by the selection index.
            PickKind::Rooms,
        ] {
            assert!(kind.is_pickable(), "{kind:?}");
        }
        assert!(!PickKind::Bodies3d.is_pickable());
    }

    #[test]
    fn a_preset_turns_on_exactly_its_kinds() {
        let mut filter = PickFilter::default();
        filter.set_on_kinds([PickKind::Vias, PickKind::Keepouts]);
        assert_eq!(
            filter.on_kinds().collect::<Vec<_>>(),
            vec![PickKind::Keepouts, PickKind::Vias]
        );
    }
}
