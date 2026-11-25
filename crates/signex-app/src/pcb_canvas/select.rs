//! Selection and hover of the board in view (#2191). Pure state: the
//! geometry is `pcb_geom`'s, the index is built once per board revision and
//! kept until that revision changes.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use pcb_geom::{
    BoardIndex, BoardTextField, ClickMode, ItemRef, PickFilter, PointNm, Selection, pick_filtered,
    pick_under_texts, turned_marquee_select,
};
use signex_renderer::pcb::{BoardObject, PcbLayerView, zone_object};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::highlight::Highlight;

#[path = "select_drag.rs"]
mod drag;
pub use drag::{DraggedItems, DraggedSelection};

#[derive(Debug, Default)]
pub struct PcbSelectionState {
    selection: Selection,
    hover: Option<ItemRef>,
    index: Option<BoardIndex>,
    index_builds: u32,
    document: Option<PathBuf>,
    /// The `DocumentState::pcb_revisions` stamp the index was built for.
    revision: u64,
    /// Lines for the Messages panel, drained by [`Self::take_report`].
    report: Vec<String>,
    /// The board view the index was built for: items on a hidden layer are
    /// not in it, so they cannot be hovered, picked or boxed (#2223).
    view: PcbLayerView,
    /// The next build only follows a view change, so what it refuses was
    /// already reported.
    quiet_rebuild: bool,
    /// The kinds a click, a box or the hover may pick (#2353). The index
    /// keeps every kind, so a selection made another way is not filtered.
    filter: PickFilter,
    /// What a press grabs, built once per index build and filter
    /// (`drag::GrabCache`), not on every paint.
    grab_cache: Option<drag::GrabCache>,
}

/// What a drag or Move is carrying: it is drawn where it is going, so
/// nothing of it is marked at its old place.
#[derive(Debug, Default)]
struct MovingItems {
    /// Footprints, vias, and the tracks and arcs following them.
    ids: BTreeSet<Uuid>,
    /// A footprint text moved on its own.
    text: Option<(Uuid, BoardTextField)>,
}

impl MovingItems {
    fn holds(&self, item: ItemRef) -> bool {
        match item {
            ItemRef::Footprint(id)
            | ItemRef::Pad { footprint: id, .. }
            | ItemRef::Via(id)
            | ItemRef::Track(id)
            | ItemRef::Arc(id) => self.ids.contains(&id),
            ItemRef::FootprintText { footprint, field } => {
                self.ids.contains(&footprint) || self.text == Some((footprint, field))
            }
            _ => false,
        }
    }
}

/// What [`PcbSelectionState::select_items`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemsSelected {
    /// Whether the selection changed.
    pub changed: bool,
    /// How many of the items were left out for sitting on a hidden layer.
    pub hidden: usize,
}

impl PcbSelectionState {
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn hover(&self) -> Option<ItemRef> {
        self.hover
    }

    /// The board the selection was last aligned with.
    pub fn document(&self) -> Option<&Path> {
        self.document.as_deref()
    }

    /// How many times the index was built; one per board change at most.
    pub fn index_builds(&self) -> u32 {
        self.index_builds
    }

    /// Align with the board in view: another document drops the selection
    /// and hover; a new `revision` of the same one drops only the index and
    /// keeps what still exists. Called before every probe, so a writer that
    /// forgets to announce a board change cannot leave a stale index.
    pub fn sync(&mut self, document: Option<&Path>, revision: u64) {
        if self.document.as_deref() != document {
            self.document = document.map(Path::to_path_buf);
            self.selection.clear();
            self.hover = None;
            self.index = None;
        }
        if self.revision != revision {
            self.revision = revision;
            self.index = None;
            self.quiet_rebuild = false;
        }
    }

    /// Follow the board view: a change of it drops the index, which is
    /// rebuilt without the items on hidden layers.
    pub fn set_view(&mut self, view: &PcbLayerView) {
        if self.view == *view {
            return;
        }
        self.view = view.clone();
        self.quiet_rebuild = self.index.take().is_some();
    }

    /// Follow the selection filter: a hovered item of a kind turned off is
    /// no longer hovered. The selection keeps what it holds.
    pub fn set_filter(&mut self, filter: &PickFilter) {
        if self.filter == *filter {
            return;
        }
        self.filter = filter.clone();
        if self.hover.is_some_and(|h| !filter.allows(h)) {
            self.hover = None;
        }
    }

    /// Items the last index build left out, once per build; empty after.
    pub fn take_report(&mut self) -> Vec<String> {
        std::mem::take(&mut self.report)
    }

    /// Build the index now, if this revision has none, so what it refuses
    /// reaches [`Self::take_report`] without a probe having to ask first.
    pub fn build_index(&mut self, board: &PcbBoard) {
        self.ensure_index(board);
    }

    /// Esc: drop selection and hover. Whether either held anything.
    pub fn clear(&mut self) -> bool {
        let cleared = self.selection.clear();
        cleared | self.hover.take().is_some()
    }

    /// Hover probe at `at`. Whether the hovered item changed.
    pub fn hover_at(&mut self, board: &PcbBoard, at: Point, tolerance_mm: f64) -> bool {
        let hit = self.pick_at(board, at, tolerance_mm).map(|(item, _)| item);
        std::mem::replace(&mut self.hover, hit) != hit
    }

    /// A click at `at`. Whether the selection changed.
    pub fn click(
        &mut self,
        board: &PcbBoard,
        at: Point,
        tolerance_mm: f64,
        mode: ClickMode,
    ) -> bool {
        let hit = self.pick_at(board, at, tolerance_mm).map(|(item, _)| item);
        self.selection.apply_click(hit, mode)
    }

    /// The item a click at `at` would pick, without selecting it (the
    /// Properties net pick, #2355).
    pub fn item_at(&mut self, board: &PcbBoard, at: Point, tolerance_mm: f64) -> Option<ItemRef> {
        self.pick_at(board, at, tolerance_mm).map(|(item, _)| item)
    }

    /// [`Self::click`] for a tool that acts on parts (Move): a designator
    /// or comment is passed over for what lies under it (#2368).
    pub fn click_part(
        &mut self,
        board: &PcbBoard,
        at: Point,
        tolerance_mm: f64,
        mode: ClickMode,
    ) -> bool {
        let hit = self.part_at(board, at, tolerance_mm);
        self.selection.apply_click(hit, mode)
    }

    /// [`Self::item_at`] passing over designators and comments (Measure).
    pub fn part_at(&mut self, board: &PcbBoard, at: Point, tolerance_mm: f64) -> Option<ItemRef> {
        let p = PointNm::from_mm(at, "pointer").ok()?;
        let tol = pcb_geom::mm_to_nm(tolerance_mm, "tolerance").ok()?;
        self.ensure_index(board);
        pick_under_texts(self.index.as_ref()?, p, tol, &self.filter).map(|i| i.item)
    }

    /// A box from `from` to `to`, drawn along the screen of a view turned
    /// `view_rotation_deg` (#2316). Whether the selection changed.
    pub fn box_select(
        &mut self,
        board: &PcbBoard,
        from: Point,
        to: Point,
        mode: ClickMode,
        view_rotation_deg: f64,
    ) -> bool {
        let (Ok(from), Ok(to)) = (
            PointNm::from_mm(from, "box.from"),
            PointNm::from_mm(to, "box.to"),
        ) else {
            return false;
        };
        self.ensure_index(board);
        let Some(index) = &self.index else {
            return false;
        };
        let mut hits = turned_marquee_select(index, from, to, view_rotation_deg);
        hits.retain(|item| self.filter.allows(*item));
        self.selection.apply_marquee(&hits, mode)
    }

    /// Select exactly those of `items` the view lets be picked (a
    /// violation's items #2222, a cross selection #1312): an item on a
    /// hidden layer, or not on the board, is left out (#2223).
    pub fn select_items(&mut self, board: &PcbBoard, items: &[ItemRef]) -> ItemsSelected {
        self.ensure_index(board);
        let selectable: HashSet<ItemRef> = self
            .index
            .iter()
            .flat_map(|index| index.items().map(|i| i.item))
            .collect();
        let (kept, left_out): (Vec<ItemRef>, Vec<ItemRef>) = items
            .iter()
            .copied()
            .partition(|item| selectable.contains(item));
        let hidden = if left_out.is_empty() {
            0
        } else {
            let on_hidden = hidden_items(board, &self.view);
            left_out.iter().filter(|i| on_hidden.contains(i)).count()
        };
        ItemsSelected {
            changed: self.selection.apply_marquee(&kept, ClickMode::Replace),
            hidden,
        }
    }

    /// Hold exactly `selection`, with no view filter: Replay's recorded
    /// targets, and the selection it puts back afterwards.
    pub fn put_selection(&mut self, selection: Selection) {
        self.selection = selection;
    }

    /// Select All (#2361): every footprint, track, via and drawn area the view
    /// lets be picked. A pad comes with its footprint and is selected alone only
    /// when its footprint's body cannot be picked. Whether the selection
    /// changed.
    pub fn select_all(&mut self, board: &PcbBoard) -> bool {
        self.ensure_index(board);
        let Some(index) = &self.index else {
            return false;
        };
        let bodies: HashSet<Uuid> = index
            .items()
            .filter_map(|i| match i.item {
                ItemRef::Footprint(id) => Some(id),
                _ => None,
            })
            .collect();
        let all: Vec<ItemRef> = index
            .items()
            .map(|i| i.item)
            .filter(|item| !matches!(item, ItemRef::Pad { footprint, .. } if bodies.contains(footprint)))
            // A designator or comment comes with its footprint (#2368).
            .filter(|item| !matches!(item, ItemRef::FootprintText { .. }))
            .collect();
        self.selection.apply_marquee(&all, ClickMode::Replace)
    }

    /// The copper to paint for the selection and the hover. A selected
    /// footprint paints its body outline and all its pads.
    pub fn highlight(&mut self, board: &PcbBoard) -> Highlight {
        self.highlight_except(board, &MovingItems::default())
    }

    /// [`Self::highlight`] without what is `moving`: nothing of it stays
    /// marked at its old place (#2358).
    fn highlight_except(&mut self, board: &PcbBoard, moving: &MovingItems) -> Highlight {
        if self.selection.is_empty() && self.hover.is_none() {
            return Highlight::default();
        }
        self.ensure_index(board);
        let Some(index) = &self.index else {
            return Highlight::default();
        };
        let whole: BTreeSet<Uuid> = self
            .selection
            .items()
            .filter_map(|item| match item {
                ItemRef::Footprint(id) => Some(id),
                _ => None,
            })
            .collect();
        let mut highlight = Highlight::default();
        let is_moving = |item: ItemRef| moving.holds(item);
        for indexed in index.items().filter(|i| !is_moving(i.item)) {
            match indexed.item {
                ItemRef::Footprint(id) if whole.contains(&id) => {
                    highlight.bodies.push(indexed.shape.clone());
                }
                ItemRef::Pad { footprint, .. } if whole.contains(&footprint) => {
                    highlight.selected.push(indexed.shape.clone());
                }
                item if self.selection.contains(item) => {
                    highlight.selected.push(indexed.shape.clone())
                }
                _ => {}
            }
        }
        if let Some(hovered) = self.hover.filter(|h| !is_moving(*h))
            && let Some(found) = index.items().find(|i| i.item == hovered)
        {
            match hovered {
                ItemRef::Footprint(_) => highlight.hover_body = Some(found.shape.clone()),
                _ => highlight.hover = Some(found.shape.clone()),
            }
        }
        highlight
    }

    /// What a canvas paints and grabs for this selection: the highlight, the
    /// preview of `carry` once it is anchored, and the shapes a press grabs
    /// to drag: every footprint, selected or not (#2358). While a Move or a
    /// paste waits for its clicks, or a part is on the cursor, nothing is
    /// grabbed, so a jiggled click stays a click (#2240, #2364).
    pub fn paint(
        &mut self,
        board: &PcbBoard,
        carry: Option<&super::PcbCarry>,
    ) -> (Highlight, std::sync::Arc<[pcb_geom::Shape]>) {
        let moving = carry
            .filter(|c| c.anchor.is_some() && c.previews_on_board())
            .map_or_else(MovingItems::default, |c| {
                // Following tracks are searched only if one is marked: this
                // runs on every pointer step.
                let track_marked = self
                    .hover
                    .into_iter()
                    .chain(self.selection.items())
                    .any(|item| matches!(item, ItemRef::Track(_) | ItemRef::Arc(_)));
                MovingItems {
                    ids: if track_marked {
                        c.moving_ids(board)
                    } else {
                        c.carried()
                    },
                    text: c.text,
                }
            });
        let mut highlight = self.highlight_except(board, &moving);
        // A Move, a paste or a Place waits for clicks: nothing is grabbed.
        let waiting = carry.is_some_and(super::PcbCarry::waits_for_clicks);
        let grab = if waiting {
            Vec::new().into()
        } else {
            self.grab_shapes(board)
        };
        // Only what is not on the board yet has a ghost: a paste.
        if let Some(carry) = carry.filter(|c| c.anchor.is_some())
            && let Some(load) = &carry.paste
            && let Ok(by) = PointNm::from_mm(carry.delta, "carry")
        {
            highlight.ghost = load
                .ghost
                .iter()
                .map(|shape| shape.clone().translated(by))
                .collect();
        }
        (highlight, grab)
    }

    /// The footprints a drag pressed at `at` carries (#2358). A press on a
    /// selected footprint, or on one of its pads, carries the selection;
    /// a press on an unselected one selects that footprint alone and
    /// carries it. Off every footprint it carries nothing.
    pub fn grab_for_drag(
        &mut self,
        board: &PcbBoard,
        at: Point,
        tolerance_mm: f64,
    ) -> BTreeSet<Uuid> {
        let selected = self.selection.footprints();
        let (Ok(p), Ok(tol)) = (
            PointNm::from_mm(at, "press"),
            pcb_geom::mm_to_nm(tolerance_mm, "reach"),
        ) else {
            return BTreeSet::new();
        };
        self.ensure_index(board);
        let Some(index) = &self.index else {
            return BTreeSet::new();
        };
        // A pad outranks the body it sits on, as a click picks.
        let under: Vec<(bool, ItemRef, Uuid)> = index
            .items_at(p, tol)
            .into_iter()
            .filter_map(|i| match i.item {
                ItemRef::Pad { footprint, .. } => Some((false, i.item, footprint)),
                ItemRef::Footprint(id) => Some((true, i.item, id)),
                _ => None,
            })
            .collect();
        if under.iter().any(|(_, _, id)| selected.contains(id)) {
            return selected;
        }
        let Some(&(_, _, id)) = under.iter().min_by_key(|(body, item, _)| (*body, *item)) else {
            return BTreeSet::new();
        };
        self.selection
            .apply_click(Some(ItemRef::Footprint(id)), ClickMode::Replace);
        BTreeSet::from([id])
    }

    /// The designator or comment a press at `at` grabs to move on its own
    /// (#2368): the one a click there picks. It becomes the selection.
    pub fn grab_text_for_drag(
        &mut self,
        board: &PcbBoard,
        at: Point,
        tolerance_mm: f64,
    ) -> Option<(Uuid, BoardTextField)> {
        let (item, _) = self.pick_at(board, at, tolerance_mm)?;
        let ItemRef::FootprintText { footprint, field } = item else {
            return None;
        };
        if !self.selection.contains(item) {
            self.selection.apply_click(Some(item), ClickMode::Replace);
        }
        Some((footprint, field))
    }

    /// The vias a press at `at` grabs to drag: the via a click there picks,
    /// with the other selected vias when the selection is only vias and holds
    /// it; otherwise it becomes the selection. `None` off a via, and on a via
    /// over a footprint unless a click has already picked that via: the
    /// press then carries the footprint (#2358).
    pub fn grab_vias_for_drag(
        &mut self,
        board: &PcbBoard,
        at: Point,
        tolerance_mm: f64,
    ) -> Option<BTreeSet<Uuid>> {
        let (item, _) = self.pick_at(board, at, tolerance_mm)?;
        let ItemRef::Via(id) = item else {
            return None;
        };
        let p = PointNm::from_mm(at, "press").ok()?;
        let selected = self.selection.footprints();
        // Exactly under the press: a via merely near a part is not on it.
        let under: Vec<Uuid> = self
            .index
            .as_ref()?
            .items_at(p, 0)
            .iter()
            .filter_map(|i| match i.item {
                ItemRef::Footprint(fp) | ItemRef::Pad { footprint: fp, .. } => Some(fp),
                _ => None,
            })
            .collect();
        if under.iter().any(|fp| selected.contains(fp))
            || (!under.is_empty() && !self.selection.contains(item))
        {
            return None;
        }
        // A selected via carries the selection it belongs to, whatever else
        // is in it (#2870: the drag then widens to the rest).
        if self.selection.contains(item) {
            let vias = self.selection.items().filter_map(|item| match item {
                ItemRef::Via(via) => Some(via),
                _ => None,
            });
            return Some(vias.collect());
        }
        self.selection.apply_click(Some(item), ClickMode::Replace);
        Some(BTreeSet::from([id]))
    }

    /// The shapes of the indexed items `keep` accepts; the index must be
    /// built.
    fn shapes_of(&self, keep: impl Fn(ItemRef) -> bool) -> Vec<pcb_geom::Shape> {
        self.index
            .iter()
            .flat_map(|index| index.items())
            .filter(|i| keep(i.item))
            .map(|i| i.shape.clone())
            .collect()
    }

    /// The body and pads of each footprint `keep` accepts.
    fn shapes_of_footprints(
        &mut self,
        board: &PcbBoard,
        keep: impl Fn(Uuid) -> bool,
    ) -> Vec<pcb_geom::Shape> {
        self.ensure_index(board);
        let Some(index) = &self.index else {
            return Vec::new();
        };
        index
            .items()
            .filter(|i| match i.item {
                ItemRef::Footprint(id) | ItemRef::Pad { footprint: id, .. } => keep(id),
                _ => false,
            })
            .map(|i| i.shape.clone())
            .collect()
    }

    /// Empty the selection after its items were deleted, keeping the hover;
    /// whether it held anything.
    pub fn clear_selection(&mut self) -> bool {
        self.selection.clear()
    }

    fn pick_at(
        &mut self,
        board: &PcbBoard,
        at: Point,
        tolerance_mm: f64,
    ) -> Option<(ItemRef, pcb_geom::Shape)> {
        let p = PointNm::from_mm(at, "pointer").ok()?;
        let tol = pcb_geom::mm_to_nm(tolerance_mm, "tolerance").ok()?;
        self.ensure_index(board);
        pick_filtered(self.index.as_ref()?, p, tol, &self.filter).map(|i| (i.item, i.shape.clone()))
    }

    fn ensure_index(&mut self, board: &PcbBoard) {
        if self.index.is_some() {
            return;
        }
        let index = BoardIndex::build_selectable(board);
        if let Some(line) = rejected_line(&index).filter(|_| !self.quiet_rebuild) {
            self.report.push(line);
        }
        self.quiet_rebuild = false;
        let index = self.without_hidden(index, board);
        self.selection.retain_in(&index);
        if self
            .hover
            .is_some_and(|h| !index.items().any(|i| i.item == h))
        {
            self.hover = None;
        }
        self.index_builds += 1;
        self.index = Some(index);
    }
}

impl PcbSelectionState {
    /// `index` without the items on a layer the view hides.
    fn without_hidden(&self, index: BoardIndex, board: &PcbBoard) -> BoardIndex {
        if !self.view.has_hidden() && !self.view.hides_objects() {
            return index;
        }
        let hidden = hidden_items(board, &self.view);
        index.without(|item| hidden.contains(&item))
    }
}

/// The pads, tracks, vias, footprint bodies and drawn areas of `board` that
/// sit only on hidden layers, or whose kind the View Options hide (#2363).
/// A footprint body follows the side the footprint is on.
fn hidden_items(board: &PcbBoard, view: &PcbLayerView) -> HashSet<ItemRef> {
    let mut hidden = HashSet::new();
    for footprint in &board.footprints {
        if !view.shows_any(std::slice::from_ref(&footprint.layer)) {
            hidden.insert(ItemRef::Footprint(footprint.uuid));
        }
        for pad in &footprint.pads {
            if !view.shows_object(BoardObject::Pads) || !view.shows_pad(footprint, pad) {
                hidden.insert(ItemRef::Pad {
                    footprint: footprint.uuid,
                    pad: pad.uuid,
                });
            }
        }
        for text in pcb_geom::footprint_texts(footprint) {
            if !view.is_visible(text.layer) {
                hidden.insert(ItemRef::FootprintText {
                    footprint: footprint.uuid,
                    field: text.field,
                });
            }
        }
    }
    let tracks = view.shows_object(BoardObject::Tracks);
    for segment in &board.segments {
        if !tracks || !view.shows_any(std::slice::from_ref(&segment.layer)) {
            hidden.insert(ItemRef::Track(segment.uuid));
        }
    }
    for via in &board.vias {
        if !view.shows_object(BoardObject::Vias) || !view.shows_via(&via.layers) {
            hidden.insert(ItemRef::Via(via.uuid));
        }
    }
    for zone in &board.zones {
        let kind = pcb_geom::PickKind::of_zone(zone);
        let off = !view.shows_object(zone_object(zone))
            || !view.shows_any(std::slice::from_ref(&zone.layer));
        if let Some(kind) = kind.filter(|_| off) {
            hidden.insert(ItemRef::Zone {
                zone: zone.uuid,
                kind,
            });
        }
    }
    hidden_free_items(board, view, &mut hidden);
    for graphic in &board.graphics {
        let Some(kind) = graphic.kind() else {
            continue;
        };
        // A text frame is a text: hidden with Texts.
        let text_off = kind == signex_types::pcb_graphic::BoardGraphicKind::TextFrame
            && !view.shows_object(BoardObject::Texts);
        if text_off || !view.shows_any(std::slice::from_ref(&graphic.layer)) {
            hidden.insert(ItemRef::Graphic {
                graphic: graphic.uuid,
                kind: pcb_geom::PickKind::of_graphic(kind),
            });
        }
    }
    hidden
}

/// The strings, free arcs, free pads and dimensions on layers `view` hides
/// (#2373): not picked, as they are not drawn.
fn hidden_free_items(board: &PcbBoard, view: &PcbLayerView, hidden: &mut HashSet<ItemRef>) {
    let off = |layer: &String, kind| {
        !view.shows_object(kind) || !view.shows_any(std::slice::from_ref(layer))
    };
    for text in board
        .texts
        .iter()
        .filter(|t| off(&t.layer, BoardObject::Texts))
    {
        hidden.insert(ItemRef::Text(text.uuid));
    }
    for arc in board
        .free
        .arcs
        .iter()
        .filter(|a| off(&a.layer, BoardObject::Arcs))
    {
        hidden.insert(ItemRef::Arc(arc.uuid));
    }
    for dimension in board
        .free
        .dimensions
        .iter()
        .filter(|d| off(&d.layer, BoardObject::Dimensions))
    {
        hidden.insert(ItemRef::Dimension(dimension.uuid));
    }
    let frame = signex_types::pcb_free::free_pad_frame();
    let pads = view.shows_object(BoardObject::Pads);
    for pad in board
        .free
        .pads
        .iter()
        .filter(|p| !pads || !view.shows_pad(&frame, p))
    {
        hidden.insert(ItemRef::FreePad(pad.uuid));
    }
}

/// One line naming the items the index could not take, if any.
fn rejected_line(index: &BoardIndex) -> Option<String> {
    const NAMED: usize = 3;
    let rejected = index.rejected();
    if rejected.is_empty() {
        return None;
    }
    let named: Vec<String> = rejected
        .iter()
        .take(NAMED)
        .map(|r| format!("{} ({})", describe(r.item), r.error))
        .collect();
    let more = rejected.len().saturating_sub(NAMED);
    let tail = if more > 0 {
        format!(" and {more} more")
    } else {
        String::new()
    };
    Some(format!(
        "PCB: {} board item(s) cannot be hovered or selected: {}{tail}",
        rejected.len(),
        named.join("; ")
    ))
}

fn describe(item: ItemRef) -> String {
    match item {
        ItemRef::Pad { footprint, pad } => format!("pad {pad} of footprint {footprint}"),
        ItemRef::Track(id) => format!("track {id}"),
        ItemRef::Via(id) => format!("via {id}"),
        ItemRef::Footprint(id) => format!("footprint {id}"),
        ItemRef::Zone { zone, .. } => format!("area {zone}"),
        ItemRef::Text(id) => format!("string {id}"),
        ItemRef::Arc(id) => format!("arc {id}"),
        ItemRef::FreePad(id) => format!("free pad {id}"),
        ItemRef::Dimension(id) => format!("dimension {id}"),
        ItemRef::Graphic { graphic, .. } => format!("drawing {graphic}"),
        ItemRef::FootprintText { footprint, field } => {
            format!("{} of footprint {footprint}", field.id())
        }
        ItemRef::Room(id) => format!("room {id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb::{Via, ViaType};
    use uuid::Uuid;

    fn board() -> PcbBoard {
        let via = Via {
            attributes: Default::default(),
            uuid: Uuid::from_u128(4),
            position: Point::new(1.0, 1.0),
            diameter: 0.6,
            drill: 0.3,
            layers: Vec::new(),
            net: 1,
            locked: false,
            layer_sizes: None,
            via_type: ViaType::Through,
            unknown_columns: Default::default(),
        };
        PcbBoard {
            sketch: None,
            stackup: None,
            rooms: Vec::new(),
            options: Default::default(),
            uuid: Uuid::nil(),
            version: 1,
            generator: String::new(),
            thickness: 1.6,
            outline: Vec::new(),
            layers: Vec::new(),
            setup: None,
            nets: Vec::new(),
            footprints: Vec::new(),
            segments: Vec::new(),
            vias: vec![via],
            zones: Vec::new(),
            graphics: Vec::new(),
            texts: Vec::new(),
            free: Default::default(),
            unknown: Default::default(),
        }
    }

    #[test]
    fn the_index_is_built_once_until_the_board_changes() {
        let board = board();
        let mut state = PcbSelectionState::default();
        let doc = Path::new("a.snxpcb");
        state.sync(Some(doc), 1);
        assert!(state.click(&board, Point::new(1.0, 1.0), 0.1, ClickMode::Replace));
        state.hover_at(&board, Point::new(5.0, 5.0), 0.1);
        state.box_select(
            &board,
            Point::new(0.0, 0.0),
            Point::new(2.0, 2.0),
            ClickMode::Add,
            0.0,
        );
        assert_eq!(state.index_builds(), 1);

        // Same document, new revision: the index goes, the selection stays.
        state.sync(Some(doc), 2);
        assert_eq!(state.selection().len(), 1);
        state.hover_at(&board, Point::new(1.0, 1.0), 0.1);
        assert_eq!(state.index_builds(), 2);

        // Another document: the selection goes too.
        state.sync(Some(Path::new("b.snxpcb")), 3);
        assert!(state.selection().is_empty());
        assert_eq!(state.hover(), None);
    }

    #[test]
    fn a_selected_item_gone_from_the_board_is_dropped() {
        let mut state = PcbSelectionState::default();
        state.sync(None, 1);
        state.click(&board(), Point::new(1.0, 1.0), 0.1, ClickMode::Replace);
        let mut emptied = board();
        emptied.vias.clear();
        state.sync(None, 2);
        state.hover_at(&emptied, Point::new(9.0, 9.0), 0.1);
        assert!(state.selection().is_empty());
    }

    fn two_layer_board() -> PcbBoard {
        let segment = |n: u128, y: f64, layer: &str| signex_types::pcb::Segment {
            locked: false,
            attributes: Default::default(),
            uuid: Uuid::from_u128(n),
            start: Point::new(0.0, y),
            end: Point::new(5.0, y),
            width: 0.25,
            layer: layer.to_owned(),
            net: 1,
            unknown_columns: Default::default(),
        };
        let mut board = board();
        board.vias.clear();
        board.segments = vec![
            segment(10, 0.0, "TopCopper"),
            segment(11, 3.0, "BottomCopper"),
        ];
        board
    }

    /// #2223: a track on a hidden layer is not hovered, picked or boxed, and
    /// is selectable again once the layer is shown.
    #[test]
    fn items_on_a_hidden_layer_cannot_be_hit() {
        use signex_types::pcb_layer::{CopperLayer, PcbLayer};
        let board = two_layer_board();
        let on_bottom = Point::new(2.0, 3.0);
        let mut state = PcbSelectionState::default();
        state.sync(None, 1);
        assert!(state.click(&board, on_bottom, 0.1, ClickMode::Replace));

        let mut view = PcbLayerView::default();
        view.toggle_visibility(PcbLayer::Copper(CopperLayer::Bottom));
        state.set_view(&view);
        assert!(!state.hover_at(&board, on_bottom, 0.1), "no hover");
        assert!(state.selection().is_empty(), "hiding drops the selection");
        assert!(!state.click(&board, on_bottom, 0.1, ClickMode::Replace));
        assert!(!state.box_select(
            &board,
            Point::new(-1.0, 2.0),
            Point::new(6.0, 4.0),
            ClickMode::Replace,
            0.0
        ));
        // The top track is still there.
        assert!(state.click(&board, Point::new(2.0, 0.0), 0.1, ClickMode::Replace));

        view.show_all();
        state.set_view(&view);
        assert!(state.click(&board, on_bottom, 0.1, ClickMode::Replace));
    }

    /// #2363: a kind the View Options hide is not picked, on any layer, and
    /// can be picked again once shown.
    #[test]
    fn a_hidden_object_kind_cannot_be_hit() {
        let board = two_layer_board();
        let on_top = Point::new(2.0, 0.0);
        let mut state = PcbSelectionState::default();
        state.sync(None, 1);
        let mut view = PcbLayerView::default();
        view.set_object_visible(BoardObject::Tracks, false);
        state.set_view(&view);
        assert!(!state.click(&board, on_top, 0.1, ClickMode::Replace));
        assert!(state.selection().is_empty());

        view.set_object_visible(BoardObject::Tracks, true);
        state.set_view(&view);
        assert!(state.click(&board, on_top, 0.1, ClickMode::Replace));
    }

    /// #2363: hiding Texts hides a text frame, drawing and all; a plain
    /// drawing on the same layer stays pickable.
    #[test]
    fn hiding_texts_hides_a_text_frame_but_not_a_drawing() {
        let drawing = |n: u128, kind: &str| signex_types::pcb::BoardGraphic {
            uuid: Uuid::from_u128(n),
            graphic_type: kind.to_owned(),
            layer: "TopSilkscreen".to_owned(),
            width: 0.1,
            start: Some(Point::new(0.0, 0.0)),
            end: Some(Point::new(4.0, 2.0)),
            center: None,
            radius: 0.0,
            points: Vec::new(),
            text: "NOTE".to_owned(),
            font_size: 1.0,
        };
        let mut board = board();
        board.graphics = vec![drawing(20, "text_frame"), drawing(21, "line")];
        let mut view = PcbLayerView::default();
        view.set_object_visible(BoardObject::Texts, false);
        let hidden = hidden_items(&board, &view);
        let ids: Vec<Uuid> = hidden
            .iter()
            .filter_map(|item| match item {
                ItemRef::Graphic { graphic, .. } => Some(*graphic),
                _ => None,
            })
            .collect();
        assert_eq!(ids, [Uuid::from_u128(20)]);
    }

    /// Review finding 4 / S1 on #2271: selecting a list of items (a DRC pick,
    /// a cross selection) leaves out those on a hidden layer and counts them.
    #[test]
    fn select_items_leaves_out_and_counts_items_on_a_hidden_layer() {
        use signex_types::pcb_layer::{CopperLayer, PcbLayer};
        let board = two_layer_board();
        let top = ItemRef::Track(Uuid::from_u128(10));
        let bottom = ItemRef::Track(Uuid::from_u128(11));
        let gone = ItemRef::Track(Uuid::from_u128(99));
        let mut state = PcbSelectionState::default();
        state.sync(None, 1);
        let mut view = PcbLayerView::default();
        view.toggle_visibility(PcbLayer::Copper(CopperLayer::Bottom));
        state.set_view(&view);

        let done = state.select_items(&board, &[bottom]);
        assert_eq!(
            done,
            ItemsSelected {
                changed: false,
                hidden: 1
            }
        );
        assert!(state.selection().is_empty());

        let done = state.select_items(&board, &[top, bottom, gone]);
        assert_eq!(
            done,
            ItemsSelected {
                changed: true,
                hidden: 1
            }
        );
        assert_eq!(state.selection().items().collect::<Vec<_>>(), vec![top]);

        view.show_all();
        state.set_view(&view);
        let done = state.select_items(&board, &[top, bottom]);
        assert_eq!(
            done,
            ItemsSelected {
                changed: true,
                hidden: 0
            }
        );
        assert_eq!(state.selection().len(), 2);
    }

    /// A view change rebuilds the index but does not report a refused item twice.
    #[test]
    fn a_view_change_does_not_repeat_the_refused_item_report() {
        use signex_types::pcb_layer::{CopperLayer, PcbLayer};
        let mut broken = two_layer_board();
        broken.vias = board().vias;
        broken.vias[0].position = Point::new(f64::NAN, 0.0);
        let mut state = PcbSelectionState::default();
        state.sync(None, 1);
        state.build_index(&broken);
        assert_eq!(state.take_report().len(), 1);
        let mut view = PcbLayerView::default();
        view.toggle_visibility(PcbLayer::Copper(CopperLayer::Bottom));
        state.set_view(&view);
        state.build_index(&broken);
        assert!(state.take_report().is_empty());
    }

    #[test]
    fn a_non_finite_pointer_selects_nothing() {
        let mut state = PcbSelectionState::default();
        assert!(!state.click(&board(), Point::new(f64::NAN, 0.0), 0.1, ClickMode::Replace));
    }

    #[test]
    fn a_board_edit_without_a_notice_still_drops_the_index() {
        let mut state = PcbSelectionState::default();
        state.sync(None, 1);
        state.hover_at(&board(), Point::new(1.0, 1.0), 0.1);
        state.sync(None, 1);
        state.hover_at(&board(), Point::new(1.0, 1.0), 0.1);
        assert_eq!(state.index_builds(), 1);
        state.sync(None, 2);
        state.hover_at(&board(), Point::new(1.0, 1.0), 0.1);
        assert_eq!(state.index_builds(), 2);
    }

    #[test]
    fn an_item_the_index_rejects_is_reported_once_per_build() {
        let mut broken = board();
        broken.vias[0].position = Point::new(f64::NAN, 0.0);
        let mut state = PcbSelectionState::default();
        state.sync(None, 1);
        state.hover_at(&broken, Point::new(1.0, 1.0), 0.1);
        state.hover_at(&broken, Point::new(2.0, 2.0), 0.1);
        let report = state.take_report();
        assert_eq!(report.len(), 1, "{report:?}");
        assert!(report[0].contains("via"), "{report:?}");
        assert!(state.take_report().is_empty());

        state.sync(None, 2);
        state.hover_at(&broken, Point::new(1.0, 1.0), 0.1);
        assert_eq!(state.take_report().len(), 1);
    }

    #[test]
    fn building_the_index_reports_a_refused_item_with_no_probe() {
        let mut broken = board();
        broken.vias[0].position = Point::new(f64::NAN, 0.0);
        let mut state = PcbSelectionState::default();
        state.sync(None, 1);
        state.build_index(&broken);
        state.build_index(&broken);
        assert_eq!(state.take_report().len(), 1);
        assert_eq!(state.index_builds(), 1);
    }

    /// #899: a click inside a pour's hole selects nothing; a click on the
    /// copper around the hole selects the pour.
    #[test]
    fn a_click_inside_a_zone_hole_does_not_select_the_zone() {
        let square = |x: f64, y: f64, side: f64| {
            vec![
                Point::new(x, y),
                Point::new(x + side, y),
                Point::new(x + side, y + side),
                Point::new(x, y + side),
            ]
        };
        let mut board = board();
        board.zones = vec![signex_types::pcb::Zone {
            uuid: Uuid::from_u128(20),
            net: 0,
            net_name: String::new(),
            layer: "TopCopper".into(),
            outline: square(10.0, 10.0, 10.0),
            holes: vec![square(14.0, 14.0, 2.0)],
            priority: 0,
            fill_type: "pour".into(),
            thermal_relief: false,
            thermal_gap: 0.0,
            thermal_width: 0.0,
            clearance: 0.0,
            min_thickness: 0.0,
            min_island_area: 0.0,
            remove_dead_islands: false,
            shelved: false,
            unknown: Default::default(),
        }];
        let mut state = PcbSelectionState::default();
        state.sync(None, 1);
        let in_hole = Point::new(15.0, 15.0);
        assert!(!state.click(&board, in_hole, 0.1, ClickMode::Replace));
        assert!(state.selection().is_empty());
        assert_eq!(state.item_at(&board, in_hole, 0.1), None);
        assert!(state.click(&board, Point::new(12.0, 12.0), 0.1, ClickMode::Replace));
        assert!(matches!(
            state.selection().items().collect::<Vec<_>>()[..],
            [ItemRef::Zone { zone, .. }] if zone == Uuid::from_u128(20)
        ));
        // A click in the hole is a click on empty board: it clears.
        assert!(state.click(&board, in_hole, 0.1, ClickMode::Replace));
        assert!(state.selection().is_empty());
    }

    #[test]
    fn a_clean_board_reports_nothing() {
        let mut state = PcbSelectionState::default();
        state.sync(None, 1);
        state.hover_at(&board(), Point::new(1.0, 1.0), 0.1);
        assert!(state.take_report().is_empty());
    }
}
