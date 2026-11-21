//! Footprints on their way to a new place (#2240): a drag in progress, an
//! armed Edit ▸ Move waiting for its clicks, a paste on the cursor (#2361),
//! or a library part on the cursor (#2364). Pure state; the board is
//! untouched until a click or drop commits one batch.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use pcb_geom::{ArcEnds, BoardTextField, TrackEnds};
use signex_engine::pcb::BoardClipboard;
use signex_engine::pcb::part_place::PartToPlace;
use signex_engine::pcb::placement::{grid_for, grid_step_mm};
use signex_engine::pcb::snap::{GuideHit, SnapQuery, axis_snap, guide_snap, object_snap_point};
use signex_types::pcb::{PcbBoard, Point, Segment};
use signex_types::pcb_arc::TrackArc;
use signex_types::pcb_layer::CopperLayer;
use signex_types::pcb_options::{GridUse, GuideKind};
use uuid::Uuid;

use super::PcbSnapOptions;

/// What a paste on the cursor places, and the shapes its preview draws
/// with the copy's origin at (0, 0).
#[derive(Debug, PartialEq)]
pub struct PasteLoad {
    pub copy: BoardClipboard,
    pub ghost: Vec<pcb_geom::Shape>,
}

/// How the carry started, which decides what a click does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarryKind {
    /// A press on a selected footprint, moved past the drag threshold.
    Drag,
    /// Edit ▸ Move: the first click is the reference point (and, with
    /// nothing selected, picks the footprint), the second the drop.
    Move,
    /// Paste: the copy follows the pointer, on the grid, until a click
    /// places it.
    Paste,
    /// A library part on the cursor: each click places a copy, until Esc or
    /// a right-click.
    Place,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcbCarry {
    pub kind: CarryKind,
    /// The board the carry belongs to; another tab drops it.
    pub path: PathBuf,
    /// The undocked window it started in (`None`: the main window). Another
    /// window showing the same board does not see it.
    pub window: Option<iced::window::Id>,
    /// Empty while a Move waits to pick a footprint.
    pub footprints: BTreeSet<Uuid>,
    /// The reference point, board mm; `None` until a Move's first click.
    pub anchor: Option<Point>,
    /// The snapped offset the preview shows. For a paste, where the copy's
    /// origin lands.
    pub delta: Point,
    /// What a paste places; `None` for a drag, Move or Place.
    pub paste: Option<Arc<PasteLoad>>,
    /// The part a Place carries; its origin is at `anchor` once the pointer
    /// has been over the board.
    pub part: Option<Box<PartToPlace>>,
    /// A footprint's designator or comment a drag moves on its own (#2368);
    /// `footprints` is then empty.
    pub text: Option<(Uuid, BoardTextField)>,
    /// The vias a drag moves; `footprints` is then empty.
    pub vias: BTreeSet<Uuid>,
    /// Tracks or copper arcs end on the carried `vias`, read when the drag
    /// starts.
    pub via_tracks: bool,
    /// A drag with Ctrl held moves the footprints or vias alone: the tracks
    /// on them stay where they are (Caner, 2026-11-21).
    pub leaves_tracks: bool,
}

impl PcbCarry {
    /// A Place of `part` on the board at `path` in `window`.
    pub fn place(path: PathBuf, window: Option<iced::window::Id>, part: PartToPlace) -> Self {
        Self {
            part: Some(Box::new(part)),
            ..Self::new(CarryKind::Place, path, window, BTreeSet::new())
        }
    }

    pub fn new(
        kind: CarryKind,
        path: PathBuf,
        window: Option<iced::window::Id>,
        footprints: BTreeSet<Uuid>,
    ) -> Self {
        Self {
            kind,
            path,
            window,
            footprints,
            anchor: None,
            delta: Point::new(0.0, 0.0),
            paste: None,
            part: None,
            text: None,
            vias: BTreeSet::new(),
            via_tracks: false,
            leaves_tracks: false,
        }
    }

    /// The footprints and vias the carry takes.
    pub fn carried(&self) -> BTreeSet<Uuid> {
        self.footprints.union(&self.vias).copied().collect()
    }

    /// Whether the board is drawn without what is carried: for footprints,
    /// and for vias with tracks following them. A lone via keeps its ghost.
    pub fn previews_on_board(&self) -> bool {
        !self.footprints.is_empty() || (self.via_tracks && !self.leaves_tracks)
    }

    /// The track ends on what is carried, which move with it; none with
    /// Ctrl held.
    pub fn following(&self, board: &PcbBoard) -> Vec<TrackEnds> {
        if self.leaves_tracks {
            Vec::new()
        } else if self.vias.is_empty() {
            pcb_geom::tracks_on_pads(board, &self.footprints)
        } else {
            pcb_geom::tracks_on_vias(board, &self.vias)
        }
    }

    /// The copper arc ends on the carried vias, which move with them; none
    /// with Ctrl held.
    pub fn following_arcs(&self, board: &PcbBoard) -> Vec<ArcEnds> {
        if self.leaves_tracks {
            Vec::new()
        } else {
            pcb_geom::arcs_on_vias(board, &self.vias)
        }
    }

    /// What a via's snap leaves out: the carried vias and the tracks and
    /// arcs following them. With Ctrl they stay put, so they still count.
    pub fn snap_skip(&self, board: &PcbBoard) -> BTreeSet<Uuid> {
        let mut skip = self.vias.clone();
        skip.extend(self.following(board).iter().map(|ends| ends.track));
        skip.extend(self.following_arcs(board).iter().map(|ends| ends.arc));
        skip
    }

    /// A Move, a paste or a part on the cursor waits for clicks: nothing is
    /// grabbed and every press is a click (#2240, #2364, #2396).
    pub fn waits_for_clicks(&self) -> bool {
        self.kind != CarryKind::Drag
    }

    /// A paste of `load` on the cursor, its origin at `at`.
    pub fn paste(
        path: PathBuf,
        window: Option<iced::window::Id>,
        load: PasteLoad,
        at: Point,
    ) -> Self {
        Self {
            anchor: Some(Point::new(0.0, 0.0)),
            delta: at,
            paste: Some(Arc::new(load)),
            ..Self::new(CarryKind::Paste, path, window, BTreeSet::new())
        }
    }

    /// Whether the carry belongs to the board at `path` shown in `window`.
    pub fn is_for(&self, path: Option<&std::path::Path>, window: Option<iced::window::Id>) -> bool {
        path == Some(self.path.as_path()) && window == self.window
    }
}

/// The offset that takes the carried footprints from `from` towards `to`.
/// An object point within the snap distance of `to` takes `from` exactly
/// onto it (#2353). Otherwise, with grid snapping on, the origin of the
/// footprint nearest `from` lands on the components grid
/// (`placement::grid_for`); then a guide in reach fixes the axes it runs
/// across, and axis snapping lines the free axes up with object points
/// (#2374). `active` is the active copper layer.
pub fn carry_delta(
    board: &PcbBoard,
    footprints: &BTreeSet<Uuid>,
    from: Point,
    to: Point,
    snap: &PcbSnapOptions,
    active: CopperLayer,
) -> Point {
    let on_grid = grid_delta(board, footprints, from, to, snap);
    snapped_delta(board, footprints, from, to, on_grid, snap, active)
}

/// The offset that takes the carried `vias` from `from` towards `to`, snapped
/// as [`carry_delta`] snaps but for the centre of the via nearest `from`, so
/// it lands exactly on the snap point wherever the via was pressed. A via is
/// not a component: it takes the Non-Components grid.
pub fn via_carry_delta(
    board: &PcbBoard,
    vias: &BTreeSet<Uuid>,
    skip: &BTreeSet<Uuid>,
    from: Point,
    to: Point,
    snap: &PcbSnapOptions,
    active: CopperLayer,
) -> Point {
    let raw = Point::new(to.x - from.x, to.y - from.y);
    let distance = |p: Point| (p.x - from.x).powi(2) + (p.y - from.y).powi(2);
    let Some(centre) = board
        .vias
        .iter()
        .filter(|via| vias.contains(&via.uuid))
        .map(|via| via.position)
        .min_by(|a, b| distance(*a).total_cmp(&distance(*b)))
    else {
        return raw;
    };
    let on_grid = if snap.to_grids {
        grid_for(board, GridUse::NonComponents).snap_delta(centre, raw)
    } else {
        raw
    };
    let target = Point::new(centre.x + raw.x, centre.y + raw.y);
    snapped_delta(board, skip, centre, target, on_grid, snap, active)
}

/// `on_grid` refined by the object, guide and axis snaps at `to`, leaving
/// the carried `skip` items out of them.
fn snapped_delta(
    board: &PcbBoard,
    skip: &BTreeSet<Uuid>,
    from: Point,
    to: Point,
    on_grid: Point,
    snap: &PcbSnapOptions,
    active: CopperLayer,
) -> Point {
    let radius_mm = snap.distance.factor() * grid_step_mm(board);
    let query = snap.layer_mask(active).map(|layers| SnapQuery {
        objects: &snap.objects,
        layers,
        skip,
        radius_mm,
    });
    if let Some(query) = &query
        && let Some(target) = object_snap_point(board, to, query)
    {
        return Point::new(target.x - from.x, target.y - from.y);
    }
    let mut delta = on_grid;
    let (mut free_x, mut free_y) = (true, true);
    if snap.to_guides
        && let Some(hit) = guide_snap(&board.options.guides, to, radius_mm)
    {
        let onto = |p: Point| Point::new(p.x - from.x, p.y - from.y);
        match hit {
            GuideHit::Line {
                kind: GuideKind::Vertical,
                point,
            } => {
                delta.x = point.x - from.x;
                free_x = false;
            }
            GuideHit::Line {
                kind: GuideKind::Horizontal,
                point,
            } => {
                delta.y = point.y - from.y;
                free_y = false;
            }
            GuideHit::Line { point, .. } | GuideHit::Crossing(point) => return onto(point),
        }
    }
    if snap.to_axes
        && let Some(query) = &query
    {
        let (x, y) = axis_snap(board, to, query, snap.axis_range_mm);
        if let Some(x) = x.filter(|_| free_x) {
            delta.x = x - from.x;
        }
        if let Some(y) = y.filter(|_| free_y) {
            delta.y = y - from.y;
        }
    }
    delta
}

/// The carry's offset on the components grid, or as dragged with grid
/// snapping off.
fn grid_delta(
    board: &PcbBoard,
    footprints: &BTreeSet<Uuid>,
    from: Point,
    to: Point,
    snap: &PcbSnapOptions,
) -> Point {
    let raw = Point::new(to.x - from.x, to.y - from.y);
    if !snap.to_grids {
        return raw;
    }
    let distance = |p: Point| (p.x - from.x).powi(2) + (p.y - from.y).powi(2);
    let anchor = board
        .footprints
        .iter()
        .filter(|fp| footprints.contains(&fp.uuid))
        .map(|fp| fp.position)
        .min_by(|a, b| distance(*a).total_cmp(&distance(*b)));
    match anchor {
        Some(anchor) => grid_for(board, GridUse::Components).snap_delta(anchor, raw),
        None => raw,
    }
}

/// Where a placement tool's point at `to` lands (#2373): an object point
/// within the snap distance, else the nearest point of the board's grid when
/// grid snapping is on, else `to` itself. `active` is the active copper
/// layer.
pub fn snap_point(
    board: &PcbBoard,
    to: Point,
    snap: &PcbSnapOptions,
    active: CopperLayer,
) -> Point {
    if let Some(layers) = snap.layer_mask(active) {
        let none = BTreeSet::new();
        let query = SnapQuery {
            objects: &snap.objects,
            layers,
            skip: &none,
            radius_mm: snap.distance.factor() * grid_step_mm(board),
        };
        if let Some(target) = object_snap_point(board, to, &query) {
            return target;
        }
    }
    if !snap.to_grids {
        return to;
    }
    // Tool points are not components: the Non-Components grid (#2374).
    grid_for(board, GridUse::NonComponents).snap_delta(Point::new(0.0, 0.0), to)
}

/// `board` without the `carried` footprints and vias and the `following`
/// tracks, the ones with an end on them: what a drag preview leaves in place.
pub fn board_without(
    board: &PcbBoard,
    carried: &BTreeSet<Uuid>,
    following: &[TrackEnds],
) -> PcbBoard {
    let following: BTreeSet<Uuid> = following.iter().map(|ends| ends.track).collect();
    let mut rest = PcbBoard {
        footprints: Vec::new(),
        segments: Vec::new(),
        vias: Vec::new(),
        ..board.clone()
    };
    rest.footprints = board
        .footprints
        .iter()
        .filter(|fp| !carried.contains(&fp.uuid))
        .cloned()
        .collect();
    rest.vias = board
        .vias
        .iter()
        .filter(|via| !carried.contains(&via.uuid))
        .cloned()
        .collect();
    rest.segments = board
        .segments
        .iter()
        .filter(|segment| !following.contains(&segment.uuid))
        .cloned()
        .collect();
    rest
}

/// The arcs of `ends` on `board` as the move by `delta` leaves them; an arc
/// no longer where `ends` found it is left out.
pub fn followed_arcs(board: &PcbBoard, ends: &[ArcEnds], delta: Point) -> Vec<TrackArc> {
    ends.iter()
        .filter_map(|ends| {
            let arc = board.free.arcs.get(ends.index)?;
            (arc.uuid == ends.arc).then(|| ends.follow(arc, delta))
        })
        .collect()
}

/// `board` with each of `arcs` put over the arc with its uuid.
pub fn with_arcs_replaced(mut board: PcbBoard, arcs: &[TrackArc]) -> PcbBoard {
    for arc in arcs {
        if let Some(slot) = board.free.arcs.iter_mut().find(|a| a.uuid == arc.uuid) {
            *slot = arc.clone();
        }
    }
    board
}

/// The track of `ends` on `board` with its ends on the moved pads shifted
/// by `delta`; `None` when `board` no longer holds that track where `ends`
/// found it (`following` is read once per carry; the board may change).
fn followed(board: &PcbBoard, ends: &TrackEnds, delta: Point) -> Option<Segment> {
    let segment = board
        .segments
        .get(ends.index)
        .filter(|segment| segment.uuid == ends.track)?;
    Some(ends.follow(segment, delta))
}

/// `board` as a move of the `carried` footprints and vias by `delta` leaves
/// it: they and the `following` track ends on them moved (#2379), as the
/// drop's commands (`placement::move_commands`, `move_vias`) move them. The
/// drag preview's ratsnest is read off it.
pub fn board_after_move(
    board: &PcbBoard,
    carried: &BTreeSet<Uuid>,
    following: &[TrackEnds],
    delta: Point,
) -> PcbBoard {
    let mut moved = board.clone();
    for fp in moved
        .footprints
        .iter_mut()
        .filter(|fp| carried.contains(&fp.uuid))
    {
        fp.position = Point::new(fp.position.x + delta.x, fp.position.y + delta.y);
    }
    for via in moved
        .vias
        .iter_mut()
        .filter(|via| carried.contains(&via.uuid))
    {
        via.position = Point::new(via.position.x + delta.x, via.position.y + delta.y);
    }
    for ends in following {
        if let Some(segment) = followed(board, ends, delta) {
            moved.segments[ends.index] = segment;
        }
    }
    moved
}

/// Only the `carried` footprints and vias of `board`, moved by `delta`, on
/// its layers, with the `following` tracks on them (#2379): what a drag
/// preview draws where they are going. No other copper of the board's own.
pub fn carried_board(
    board: &PcbBoard,
    carried: &BTreeSet<Uuid>,
    following: &[TrackEnds],
    delta: Point,
) -> PcbBoard {
    let moved = board
        .footprints
        .iter()
        .filter(|fp| carried.contains(&fp.uuid))
        .map(|fp| {
            let mut fp = fp.clone();
            fp.position = Point::new(fp.position.x + delta.x, fp.position.y + delta.y);
            fp
        })
        .collect();
    let vias = board
        .vias
        .iter()
        .filter(|via| carried.contains(&via.uuid))
        .map(|via| {
            let mut via = via.clone();
            via.position = Point::new(via.position.x + delta.x, via.position.y + delta.y);
            via
        })
        .collect();
    let segments = following
        .iter()
        .filter_map(|ends| followed(board, ends, delta))
        .collect();
    PcbBoard {
        uuid: board.uuid,
        version: board.version,
        generator: String::new(),
        thickness: board.thickness,
        outline: Vec::new(),
        layers: board.layers.clone(),
        setup: board.setup.clone(),
        nets: board.nets.clone(),
        footprints: moved,
        segments,
        vias,
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        unknown: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_origin_lands_on_the_grid() {
        let board: PcbBoard = serde_json::from_value(serde_json::json!({
            "uuid": Uuid::nil(),
            "setup": {"grid_size": 0.5},
            "footprints": [
                {"uuid": Uuid::from_u128(1), "position": {"x": 1.1, "y": 1.0}},
                {"uuid": Uuid::from_u128(2), "position": {"x": 9.0, "y": 9.0}}
            ]
        }))
        .unwrap();
        let both: BTreeSet<Uuid> = [Uuid::from_u128(1), Uuid::from_u128(2)].into();
        let delta = carry_delta(
            &board,
            &both,
            Point::new(1.0, 1.0),
            Point::new(2.0, 1.3),
            &PcbSnapOptions::default(),
            CopperLayer::Top,
        );
        // U1 at 1.1 + 1.0 = 2.1 snaps to 2.0; y 1.0 + 0.3 snaps to 1.5.
        assert!(
            (delta.x - 0.9).abs() < 1e-9 && (delta.y - 0.5).abs() < 1e-9,
            "{delta:?}"
        );
    }

    /// U1 at (1, 1) on a 0.5 mm board grid, with a vertical guide at x = 5
    /// and one extra 0.2 mm components grid from (0.1, 0.1).
    fn guided_board() -> PcbBoard {
        serde_json::from_value(serde_json::json!({
            "uuid": Uuid::nil(),
            "setup": {"grid_size": 0.5},
            "footprints": [{"uuid": Uuid::from_u128(1), "position": {"x": 1.0, "y": 1.0}}],
            "options": {
                "guides": [{"uuid": Uuid::from_u128(7), "kind": "vertical",
                            "at": {"x": 5.0, "y": 0.0}, "enabled": true,
                            "color": {"r": 0, "g": 0, "b": 0, "a": 255}}],
                "grids": [{"uuid": Uuid::from_u128(8), "name": "Fine", "step_mm": 0.2,
                           "origin": {"x": 0.1, "y": 0.1},
                           "color": {"r": 0, "g": 0, "b": 0, "a": 255},
                           "components": true, "non_components": false}]
            }
        }))
        .unwrap()
    }

    fn carry(snap: &PcbSnapOptions, to: Point) -> Point {
        carry_delta(
            &guided_board(),
            &[Uuid::from_u128(1)].into(),
            Point::new(1.0, 1.0),
            to,
            snap,
            CopperLayer::Top,
        )
    }

    #[test]
    fn a_guide_fixes_x_and_the_components_grid_rounds_y() {
        let delta = carry(&PcbSnapOptions::default(), Point::new(5.1, 2.33));
        // x onto the guide; y: 1.0 + 1.33 on the 0.2 grid from 0.1 is 2.3.
        assert!(
            (delta.x - 4.0).abs() < 1e-9 && (delta.y - 1.3).abs() < 1e-9,
            "{delta:?}"
        );
        let off = PcbSnapOptions {
            to_guides: false,
            ..PcbSnapOptions::default()
        };
        let delta = carry(&off, Point::new(5.1, 2.33));
        assert!((delta.x - 4.1).abs() < 1e-9, "{delta:?}");
    }
}
