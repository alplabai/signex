//! Which kinds of board object a click or a box may pick (#2353): the PCB
//! selection filter. Pure data; the app keeps one for the board editor.

use std::collections::BTreeSet;

use crate::index::ItemRef;

/// One kind of board object the selection filter names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
        }
    }

    /// Whether the selection index holds objects of this kind, so turning
    /// it off changes what a click or box picks.
    pub fn is_pickable(self) -> bool {
        matches!(
            self,
            PickKind::Components | PickKind::Tracks | PickKind::Pads | PickKind::Vias
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

    /// The kinds that are on, in [`PickKind::ALL`] order.
    pub fn on_kinds(&self) -> impl Iterator<Item = PickKind> + '_ {
        PickKind::ALL.into_iter().filter(|kind| self.is_on(*kind))
    }
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
}
