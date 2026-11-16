//! Click and marquee selection on a board index (#2191). Pure: it reads a
//! [`BoardIndex`] and a set of [`ItemRef`]s; the app owns the state.

use std::collections::BTreeSet;

use crate::index::{BoardIndex, IndexedItem, ItemRef, RectSelect};
use crate::pick_filter::PickFilter;
use crate::rect::RectNm;
use crate::units::PointNm;

/// How a click or marquee combines with the selection already held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickMode {
    /// Replace the selection (no modifier).
    Replace,
    /// Add to it (Shift).
    Add,
    /// Flip each hit in or out of it (Ctrl).
    Toggle,
}

/// The selected items, in a fixed order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    items: BTreeSet<ItemRef>,
}

impl Selection {
    pub fn items(&self) -> impl Iterator<Item = ItemRef> + '_ {
        self.items.iter().copied()
    }

    pub fn contains(&self, item: ItemRef) -> bool {
        self.items.contains(&item)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Empty the selection; whether it held anything.
    pub fn clear(&mut self) -> bool {
        let had = !self.items.is_empty();
        self.items.clear();
        had
    }

    /// Apply the item a click landed on (`None`: empty board space). An
    /// empty click replaces with nothing; with Shift or Ctrl it changes
    /// nothing. Whether the selection changed.
    pub fn apply_click(&mut self, hit: Option<ItemRef>, mode: ClickMode) -> bool {
        self.apply(hit.as_slice(), mode)
    }

    /// Apply the items a marquee caught. Whether the selection changed.
    pub fn apply_marquee(&mut self, hits: &[ItemRef], mode: ClickMode) -> bool {
        self.apply(hits, mode)
    }

    /// Drop every item the index no longer holds. Whether any went.
    pub fn retain_in(&mut self, index: &BoardIndex) -> bool {
        let live: BTreeSet<ItemRef> = index.items().map(|i| i.item).collect();
        let before = self.items.len();
        self.items.retain(|item| live.contains(item));
        self.items.len() != before
    }

    fn apply(&mut self, hits: &[ItemRef], mode: ClickMode) -> bool {
        let before = self.items.clone();
        match mode {
            ClickMode::Replace => self.items = hits.iter().copied().collect(),
            ClickMode::Add => self.items.extend(hits.iter().copied()),
            ClickMode::Toggle => {
                for &hit in hits {
                    if !self.items.remove(&hit) {
                        self.items.insert(hit);
                    }
                }
            }
        }
        self.items != before
    }
}

impl Selection {
    /// The footprints a placement edit acts on: each selected footprint and
    /// the footprint of each selected pad, since a pad does not move alone.
    pub fn footprints(&self) -> BTreeSet<uuid::Uuid> {
        self.items
            .iter()
            .filter_map(|item| match *item {
                ItemRef::Footprint(id) | ItemRef::Pad { footprint: id, .. } => Some(id),
                ItemRef::Track(_)
                | ItemRef::Via(_)
                | ItemRef::Zone { .. }
                | ItemRef::Graphic { .. } => None,
            })
            .collect()
    }
}

/// The item under `p`: a via over a pad over a track over a board drawing
/// over a footprint body over a drawn area, then the lowest
/// [`ItemRef`], so the answer does not depend on how the tree was built.
pub fn pick(index: &BoardIndex, p: PointNm, tolerance: i64) -> Option<&IndexedItem> {
    pick_filtered(index, p, tolerance, &PickFilter::default())
}

/// [`pick`] among the items `filter` allows: an item of a kind turned off
/// is passed over for the one under it (#2353).
pub fn pick_filtered<'a>(
    index: &'a BoardIndex,
    p: PointNm,
    tolerance: i64,
    filter: &PickFilter,
) -> Option<&'a IndexedItem> {
    index
        .items_at(p, tolerance)
        .into_iter()
        .filter(|i| filter.allows(i.item))
        .min_by_key(|i| (rank(i.item), i.item))
}

fn rank(item: ItemRef) -> u8 {
    match item {
        ItemRef::Via(_) => 0,
        ItemRef::Pad { .. } => 1,
        ItemRef::Track(_) => 2,
        // A drawing's thin stroke over a part's body takes the click (#2424).
        ItemRef::Graphic { .. } => 3,
        // The body is under its own pads: a click on copper takes the copper.
        ItemRef::Footprint(_) => 4,
        // A drawn area is under everything on it (#2383).
        ItemRef::Zone { .. } => 5,
    }
}

/// A marquee dragged left to right selects what lies wholly inside; right
/// to left (or straight up or down), what it touches.
pub fn marquee_mode(from: PointNm, to: PointNm) -> RectSelect {
    if to.x > from.x {
        RectSelect::Inside
    } else {
        RectSelect::Touching
    }
}

/// The items a marquee from `from` to `to` selects.
pub fn marquee_select(index: &BoardIndex, from: PointNm, to: PointNm) -> Vec<ItemRef> {
    index.hit_rect(&RectNm::new(from, to), marquee_mode(from, to))
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
