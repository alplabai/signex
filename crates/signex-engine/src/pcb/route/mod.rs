//! Interactive routing (#2389): each click routes a leg from the run's end
//! to the point on the active copper layer, walking around other nets'
//! copper at the rule clearance or pushing their tracks aside
//! ([`RouteMode`]). Legs gather in a [`RouteDraft`] that reaches the board
//! as ONE undo step when the route ends ([`RouteDraft::commands`]), after
//! [`draft_fits`] checked it against the board as it is then. The
//! geometry is `pcb_geom::route`; this module turns it into board items.

mod tune;

use pcb_geom::route::{
    ClearanceRule, NewCopper, NewKind, Pushed, RouteWorld, Walk, rule_legs, shove, walkaround,
};
use pcb_geom::{ItemRef, PointNm, layer_bit, mm_to_nm, via_layers, via_shape};
use signex_types::corner::CornerRule;
use signex_types::format::pcb_layer_token;
use signex_types::pcb::{PcbBoard, Point, Segment, Via};
use signex_types::pcb_layer::PcbLayer;
use uuid::Uuid;

use super::command::BoardCommand;
use super::draw::TrackStyle;
use super::items::BoardItem;

pub use tune::{Meander, TuneRefusal, tune_length};

/// Lattice points one clicked leg's search may expand.
pub const SEARCH_BUDGET: usize = 12_000;

/// Lattice points the search for the leg on the pointer may expand: the
/// preview runs while the pointer moves, so it gives up sooner.
pub const PREVIEW_BUDGET: usize = 1_500;

/// Most lattice steps across a leg: a longer leg searches a coarser
/// multiple of the grid.
const MAX_STEPS: i64 = 160;

/// How a leg treats other nets' copper in its way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RouteMode {
    /// Go around it at the clearance the rules ask.
    #[default]
    Walkaround,
    /// Push other nets' tracks aside where they can move and stay legal,
    /// and go around what cannot move.
    Shove,
}

impl RouteMode {
    /// The other mode.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Walkaround => Self::Shove,
            Self::Shove => Self::Walkaround,
        }
    }
}

/// What a leg is routed by.
pub struct RouteRules<'a> {
    /// The clearance between two copper items, as the design rule check
    /// asks it.
    pub clearance: &'a ClearanceRule<'a>,
    /// The widest clearance the rules can ask of any pair, in nm: how far
    /// round new copper the check looks.
    pub reach: i64,
    pub corner: CornerRule,
    pub mode: RouteMode,
    /// The board's snap grid: the search's lattice step.
    pub grid_mm: f64,
    /// Lattice points the search may expand ([`SEARCH_BUDGET`],
    /// [`PREVIEW_BUDGET`]).
    pub budget: usize,
}

/// Why a leg was not routed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RouteRefusal {
    #[error("no way there keeps the clearance the rules ask")]
    Blocked,
    #[error("the via does not keep the clearance the rules ask")]
    ViaBlocked,
    #[error("a point or the width is not a length the board geometry can hold")]
    Unrepresentable,
    #[error("a track the route pushed was changed or removed since")]
    BoardChanged,
}

/// One routed leg: its tracks, the via that took it to another layer, and
/// the other nets' tracks it pushed (as they will be, and as they were).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoutedLeg {
    pub tracks: Vec<Segment>,
    pub via: Option<Via>,
    pub shoved: Vec<Segment>,
    /// The pushed tracks as the board held them before the route.
    pub shoved_from: Vec<Segment>,
}

/// The legs of a route not yet on the board.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RouteDraft {
    pub tracks: Vec<Segment>,
    pub vias: Vec<Via>,
    /// Board tracks the route pushed, at their new place; one per track.
    pub shoved: Vec<Segment>,
    /// Those tracks as the board held them before the route first pushed
    /// them: a board that no longer holds one as it was has been edited
    /// under the route ([`draft_fits`]).
    pub shoved_from: Vec<Segment>,
}

impl RouteDraft {
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty() && self.vias.is_empty() && self.shoved.is_empty()
    }

    /// Add a leg; a track pushed again keeps only its latest place, and
    /// the place it had before the first push.
    pub fn add(&mut self, leg: RoutedLeg) {
        self.vias.extend(leg.via);
        self.tracks.extend(leg.tracks);
        for moved in leg.shoved {
            match self.shoved.iter_mut().find(|held| held.uuid == moved.uuid) {
                Some(held) => *held = moved,
                None => self.shoved.push(moved),
            }
        }
        for was in leg.shoved_from {
            if !self.shoved_from.iter().any(|held| held.uuid == was.uuid) {
                self.shoved_from.push(was);
            }
        }
    }

    /// `board` as it will be once the draft lands: what the next leg is
    /// routed against.
    pub fn applied_to(&self, board: &PcbBoard) -> PcbBoard {
        let mut out = board.clone();
        for moved in &self.shoved {
            if let Some(track) = out.segments.iter_mut().find(|t| t.uuid == moved.uuid) {
                *track = moved.clone();
            }
        }
        out.segments.extend(self.tracks.iter().cloned());
        out.vias.extend(self.vias.iter().cloned());
        out
    }

    /// The draft as one undo step: its vias and tracks added, the pushed
    /// tracks replaced. Empty for an empty draft.
    pub fn commands(&self) -> Vec<BoardCommand> {
        let added: Vec<BoardItem> = self
            .vias
            .iter()
            .cloned()
            .map(BoardItem::Via)
            .chain(self.tracks.iter().cloned().map(BoardItem::Segment))
            .collect();
        let mut commands = Vec::new();
        if !added.is_empty() {
            commands.push(BoardCommand::AddItems { items: added });
        }
        if !self.shoved.is_empty() {
            let items = self
                .shoved
                .iter()
                .cloned()
                .map(BoardItem::Segment)
                .collect();
            commands.push(BoardCommand::ReplaceItems { items });
        }
        commands
    }
}

/// A leg to route: from the run's end to the clicked point on `style`'s
/// layer, with `via` at `from` when the leg changes layer.
#[derive(Debug, Clone, PartialEq)]
pub struct LegRequest {
    pub from: Point,
    pub to: Point,
    pub style: TrackStyle,
    pub via: Option<Via>,
}

/// Route `leg` against `board` with `draft` applied. `ids` names each new
/// track.
pub fn route_leg(
    board: &PcbBoard,
    draft: &RouteDraft,
    leg: LegRequest,
    rules: &RouteRules,
    mut ids: impl FnMut() -> Uuid,
) -> Result<RoutedLeg, RouteRefusal> {
    let LegRequest {
        from,
        to,
        style,
        via,
    } = leg;
    let working = draft.applied_to(board);
    let world = RouteWorld::with_reach(&working, rules.clearance, rules.reach);
    let (a, b) = (nm(from)?, nm(to)?);
    let width =
        mm_to_nm(style.width_mm, "track width").map_err(|_| RouteRefusal::Unrepresentable)?;
    let new_via = match &via {
        Some(via) => {
            let shape = via_shape(via).map_err(|_| RouteRefusal::Unrepresentable)?;
            let new = NewCopper {
                kind: NewKind::Via,
                net: style.net,
                layers: via_layers(via),
                shape,
            };
            if !world.is_clear(&new) {
                return Err(RouteRefusal::ViaBlocked);
            }
            Some(new)
        }
        None => None,
    };
    let walk = Walk {
        net: style.net,
        layers: 1 << layer_bit(style.layer),
        radius: width.abs() / 2,
        rule: rules.corner,
        step: lattice_step(rules.grid_mm, a, b),
        budget: rules.budget,
    };
    let pushing = match rules.mode {
        RouteMode::Shove => pushed_way(&world, &walk, new_via, a, b),
        RouteMode::Walkaround => None,
    };
    let (path, pushed) = match pushing {
        Some(found) => found,
        None => (
            walkaround(&world, a, b, &walk).ok_or(RouteRefusal::Blocked)?,
            Vec::new(),
        ),
    };
    let layer = pcb_layer_token(PcbLayer::Copper(style.layer));
    // The leg's ends are the clicked points as given, not their nm image.
    let point = |p: PointNm| match p {
        p if p == a => from,
        p if p == b => to,
        p => p.to_mm(),
    };
    let tracks = path
        .windows(2)
        .map(|leg| Segment {
            uuid: ids(),
            start: point(leg[0]),
            end: point(leg[1]),
            width: style.width_mm,
            layer: layer.clone(),
            net: style.net,
            unknown_columns: Default::default(),
        })
        .collect();
    let shoved: Vec<Segment> = pushed
        .into_iter()
        .filter_map(|p| moved_track(&working, p))
        .collect();
    let shoved_from = shoved
        .iter()
        .filter_map(|moved| board.segments.iter().find(|s| s.uuid == moved.uuid))
        .cloned()
        .collect();
    Ok(RoutedLeg {
        tracks,
        via,
        shoved,
        shoved_from,
    })
}

/// Whether `draft` still lands legally on `board`, which may have changed
/// since the draft was routed (an undo, an edit from another window or
/// panel): every track it pushed is on the board as it was before the
/// push, and its tracks, vias and pushed tracks keep the clearance from
/// other nets' copper and stay out of keepouts that forbid them.
pub fn draft_fits(
    board: &PcbBoard,
    draft: &RouteDraft,
    clearance: &ClearanceRule,
    reach: i64,
) -> Result<(), RouteRefusal> {
    let on_board = |uuid| board.segments.iter().find(|s| s.uuid == uuid);
    let unchanged = draft
        .shoved_from
        .iter()
        .all(|was| on_board(was.uuid) == Some(was));
    let present = draft
        .shoved
        .iter()
        .all(|moved| on_board(moved.uuid).is_some());
    if !unchanged || !present {
        return Err(RouteRefusal::BoardChanged);
    }
    let working = draft.applied_to(board);
    let world = RouteWorld::with_reach(&working, clearance, reach);
    let crowded = draft
        .tracks
        .iter()
        .chain(&draft.shoved)
        .map(|s| ItemRef::Track(s.uuid))
        .chain(draft.vias.iter().map(|v| ItemRef::Via(v.uuid)))
        .any(|id| world.is_crowded(id));
    if crowded {
        Err(RouteRefusal::Blocked)
    } else {
        Ok(())
    }
}

fn nm(p: Point) -> Result<PointNm, RouteRefusal> {
    PointNm::from_mm(p, "route point").map_err(|_| RouteRefusal::Unrepresentable)
}

/// The rule's legs straight to `b` (then the flipped rule's), with the
/// tracks pushed out of their way, when pushing clears it. A pushed track
/// keeps clear of `via` too.
fn pushed_way(
    world: &RouteWorld,
    walk: &Walk,
    via: Option<NewCopper>,
    a: PointNm,
    b: PointNm,
) -> Option<(Vec<PointNm>, Vec<Pushed>)> {
    [walk.rule, walk.rule.flipped()]
        .into_iter()
        .find_map(|rule| {
            let mut path = vec![a];
            path.extend(rule_legs(rule, a, b)?);
            let legs: Vec<NewCopper> = path
                .windows(2)
                .map(|leg| NewCopper::track(leg[0], leg[1], walk.radius, walk.net, walk.layers))
                .chain(via)
                .collect();
            shove(world, &legs).map(|pushed| (path, pushed))
        })
}

/// A pushed track at its new ends; an end that did not move keeps the
/// value it had.
fn moved_track(board: &PcbBoard, pushed: Pushed) -> Option<Segment> {
    let track = board.segments.iter().find(|s| s.uuid == pushed.track)?;
    let keep = |was: Point, now: PointNm| {
        if PointNm::from_mm(was, "track end").ok() == Some(now) {
            was
        } else {
            now.to_mm()
        }
    };
    Some(Segment {
        start: keep(track.start, pushed.start),
        end: keep(track.end, pushed.end),
        ..track.clone()
    })
}

/// The grid in nm, or a multiple of it so a leg spans at most
/// [`MAX_STEPS`] steps.
fn lattice_step(grid_mm: f64, a: PointNm, b: PointNm) -> i64 {
    let grid = mm_to_nm(grid_mm, "grid")
        .ok()
        .filter(|g| *g > 0)
        .unwrap_or(100_000)
        .max(10_000);
    let span = (b.x - a.x).abs().max((b.y - a.y).abs());
    grid * (span / grid / MAX_STEPS + 1)
}

#[cfg(test)]
mod fit_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tune_tests;
