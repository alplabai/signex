//! Selection and hover of the board in view (#2191). Pure state: the
//! geometry is `pcb_geom`'s, the index is built once per board revision and
//! kept until that revision changes.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use pcb_geom::{BoardIndex, ClickMode, ItemRef, PointNm, Selection, marquee_select, pick};
use signex_renderer::pcb::PcbLayerView;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::highlight::Highlight;

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

    /// A box from `from` to `to`. Whether the selection changed.
    pub fn box_select(
        &mut self,
        board: &PcbBoard,
        from: Point,
        to: Point,
        mode: ClickMode,
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
        let hits = marquee_select(index, from, to);
        self.selection.apply_marquee(&hits, mode)
    }

    /// Select exactly `items` (a violation's items, #2222): whatever is not
    /// on the board is dropped on the next index build, like any selection.
    /// Whether the selection changed.
    pub fn select_items(&mut self, items: &[ItemRef]) -> bool {
        self.selection.apply_marquee(items, ClickMode::Replace)
    }

    /// The copper to paint for the selection and the hover. A selected
    /// footprint paints its body outline and all its pads.
    pub fn highlight(&mut self, board: &PcbBoard) -> Highlight {
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
        for indexed in index.items() {
            match indexed.item {
                ItemRef::Footprint(id) if whole.contains(&id) => {
                    highlight.bodies.push(indexed.shape);
                }
                ItemRef::Pad { footprint, .. } if whole.contains(&footprint) => {
                    highlight.selected.push(indexed.shape);
                }
                item if self.selection.contains(item) => highlight.selected.push(indexed.shape),
                _ => {}
            }
        }
        if let Some(hovered) = self.hover
            && let Some(found) = index.items().find(|i| i.item == hovered)
        {
            match hovered {
                ItemRef::Footprint(_) => highlight.hover_body = Some(found.shape),
                _ => highlight.hover = Some(found.shape),
            }
        }
        highlight
    }

    /// What a canvas paints and grabs for this selection: the highlight, the
    /// preview of `carry` once it is anchored, and the shapes a press grabs
    /// to drag the selected footprints. While a Move waits for its clicks
    /// nothing is grabbed, so a jiggled click stays a click (#2240).
    pub fn paint(
        &mut self,
        board: &PcbBoard,
        carry: Option<&super::PcbCarry>,
    ) -> (Highlight, Vec<pcb_geom::Shape>) {
        let mut highlight = self.highlight(board);
        let moving = carry.is_some_and(|c| c.kind == super::CarryKind::Move);
        let grab = if moving {
            Vec::new()
        } else {
            let selected = self.selection.footprints();
            self.footprint_shapes(board, &selected)
        };
        if let Some(carry) = carry.filter(|c| c.anchor.is_some())
            && let Ok(by) = PointNm::from_mm(carry.delta, "carry")
        {
            highlight.ghost = self
                .footprint_shapes(board, &carry.footprints)
                .into_iter()
                .map(|shape| shape.translated(by))
                .collect();
        }
        (highlight, grab)
    }

    /// The body and pads of each footprint in `footprints`: what a press
    /// grabs to drag them, and what the drag preview paints.
    pub fn footprint_shapes(
        &mut self,
        board: &PcbBoard,
        footprints: &BTreeSet<Uuid>,
    ) -> Vec<pcb_geom::Shape> {
        if footprints.is_empty() {
            return Vec::new();
        }
        self.ensure_index(board);
        let Some(index) = &self.index else {
            return Vec::new();
        };
        index
            .items()
            .filter(|i| match i.item {
                ItemRef::Footprint(id) | ItemRef::Pad { footprint: id, .. } => {
                    footprints.contains(&id)
                }
                ItemRef::Track(_) | ItemRef::Via(_) => false,
            })
            .map(|i| i.shape)
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
        pick(self.index.as_ref()?, p, tol).map(|i| (i.item, i.shape))
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
        if !self.view.has_hidden() {
            return index;
        }
        let hidden = hidden_items(board, &self.view);
        BoardIndex::from_items(
            index
                .items()
                .filter(|indexed| !hidden.contains(&indexed.item))
                .cloned()
                .collect(),
        )
    }
}

/// The pads, tracks, vias and footprint bodies of `board` that sit only on
/// hidden layers. A footprint body follows the side the footprint is on.
fn hidden_items(board: &PcbBoard, view: &PcbLayerView) -> HashSet<ItemRef> {
    let mut hidden = HashSet::new();
    for footprint in &board.footprints {
        if !view.shows_any(std::slice::from_ref(&footprint.layer)) {
            hidden.insert(ItemRef::Footprint(footprint.uuid));
        }
        for pad in &footprint.pads {
            if !view.shows_pad(footprint, pad) {
                hidden.insert(ItemRef::Pad {
                    footprint: footprint.uuid,
                    pad: pad.uuid,
                });
            }
        }
    }
    for segment in &board.segments {
        if !view.shows_any(std::slice::from_ref(&segment.layer)) {
            hidden.insert(ItemRef::Track(segment.uuid));
        }
    }
    for via in &board.vias {
        if !view.shows_any(&via.layers) {
            hidden.insert(ItemRef::Via(via.uuid));
        }
    }
    hidden
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb::{Via, ViaType};
    use uuid::Uuid;

    fn board() -> PcbBoard {
        let via = Via {
            uuid: Uuid::from_u128(4),
            position: Point::new(1.0, 1.0),
            diameter: 0.6,
            drill: 0.3,
            layers: Vec::new(),
            net: 1,
            via_type: ViaType::Through,
            unknown_columns: Default::default(),
        };
        PcbBoard {
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
            ClickMode::Replace
        ));
        // The top track is still there.
        assert!(state.click(&board, Point::new(2.0, 0.0), 0.1, ClickMode::Replace));

        view.show_all();
        state.set_view(&view);
        assert!(state.click(&board, on_bottom, 0.1, ClickMode::Replace));
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

    #[test]
    fn a_clean_board_reports_nothing() {
        let mut state = PcbSelectionState::default();
        state.sync(None, 1);
        state.hover_at(&board(), Point::new(1.0, 1.0), 0.1);
        assert!(state.take_report().is_empty());
    }
}
