//! The soak (#2482): a seeded driver applies random route, shove, move,
//! delete, undo and redo actions to one board through the engine, and
//! checks the board after every action. A failure names its seed and
//! action index; the same seed and count replay it exactly.

use std::collections::HashSet;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use pcb_geom::ratsnest;
use signex_drc::DrcRules;
use signex_engine::pcb::route::RouteMode;
use signex_engine::pcb::{BoardCommand, BoardEngine, BoardItem, BoardItemId};
use signex_types::format::parse_pcb_layer;
use signex_types::net_class::NetClass;
use signex_types::pcb::{PcbBoard, Point, Segment};
use signex_types::pcb_layer::PcbLayer;
use uuid::Uuid;

use crate::fixtures::Fixture;
use crate::rng::Rng;
use crate::session::{Routed, plan, route, rules_of};

/// The high bits of the ids the soak gives new tracks: never a fixture's.
pub const SOAK_IDS: u128 = 0x2482 << 112;

/// One soak action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Route,
    Shove,
    MoveTrack,
    MoveFootprint,
    Delete,
    Undo,
    Redo,
}

impl Action {
    /// Weighted: routing most, undo and redo often enough to walk deep
    /// into the history.
    fn draw(rng: &mut Rng) -> Self {
        match rng.below(100) {
            0..=29 => Action::Route,
            30..=49 => Action::Shove,
            50..=57 => Action::MoveTrack,
            58..=65 => Action::MoveFootprint,
            66..=75 => Action::Delete,
            76..=89 => Action::Undo,
            _ => Action::Redo,
        }
    }
}

/// A broken invariant, with what replays it.
#[derive(Debug)]
pub struct Failure {
    pub board: String,
    pub seed: u64,
    pub index: usize,
    pub action: Action,
    pub message: String,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "routing soak on {}: seed {:#x}, action {} ({:?}): {}\n  replay: \
             SIGNEX_SOAK_BOARD={} SIGNEX_SOAK_SEED={:#x} SIGNEX_SOAK_ACTIONS={} \
             cargo test -p signex-engine --test routing soak_long -- --ignored --nocapture",
            self.board,
            self.seed,
            self.index,
            self.action,
            self.message,
            self.board,
            self.seed,
            self.index + 1,
        )
    }
}

/// What a run did, for its report line.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Tally {
    pub landed: usize,
    pub refused: usize,
    pub edits: usize,
    pub undos: usize,
    pub redos: usize,
    /// Routed tracks laid over a same-net track already there (the same
    /// layer, collinear, sharing a stretch): a router defect, #BUG, counted
    /// until it is fixed instead of failing the soak.
    pub overlapped_tracks: usize,
}

/// The engine, the history the soak expects of it, and what the board held
/// at load (pre-existing oddities are not the soak's findings).
struct Soak<'a> {
    engine: BoardEngine,
    rules: DrcRules,
    classes: &'a [NetClass],
    rng: Rng,
    next_id: u128,
    /// The board before each step the engine can undo, oldest first.
    undo: Vec<String>,
    /// The board before each undo the engine can redo, newest last.
    redo: Vec<String>,
    initial_zero_length: usize,
    tally: Tally,
    /// What the last action did, for a failure message.
    last: String,
}

/// Run `actions` seeded actions on `fixture`.
pub fn run(fixture: &Fixture, seed: u64, actions: usize) -> Result<(Tally, PcbBoard), Failure> {
    let mut soak = Soak {
        engine: BoardEngine::new(fixture.board.clone(), 1),
        rules: rules_of(&fixture.board, &fixture.classes),
        classes: &fixture.classes,
        rng: Rng::new(seed),
        next_id: 0,
        undo: Vec::new(),
        redo: Vec::new(),
        initial_zero_length: zero_length(&fixture.board),
        tally: Tally::default(),
        last: String::new(),
    };
    for index in 0..actions {
        let action = Action::draw(&mut soak.rng);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            soak.act(action)?;
            soak.check()
        }));
        let message = match outcome {
            Ok(Ok(())) => continue,
            Ok(Err(message)) => message,
            Err(panic) => format!("panicked: {}", panic_text(&*panic)),
        };
        let message = format!("{message}\n  last action: {}", soak.last);
        return Err(Failure {
            board: fixture.name.clone(),
            seed,
            index,
            action,
            message,
        });
    }
    Ok((soak.tally, soak.engine.into_board()))
}

fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_string()))
        .unwrap_or_else(|| "a non-text panic".into())
}

fn snapshot(board: &PcbBoard) -> String {
    serde_json::to_string(board).expect("a board serializes")
}

impl Soak<'_> {
    fn act(&mut self, action: Action) -> Result<(), String> {
        self.last = format!("{action:?}");
        match action {
            Action::Route => self.route(RouteMode::Walkaround),
            Action::Shove => self.route(RouteMode::Shove),
            Action::MoveTrack => self.move_track(),
            Action::MoveFootprint => self.move_footprint(),
            Action::Delete => self.delete(),
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
        }
    }

    fn route(&mut self, mode: RouteMode) -> Result<(), String> {
        let legs = 1 + usize::from(self.rng.chance(30));
        let Some(plan) = plan(self.engine.board(), &mut self.rng, mode, legs) else {
            return Ok(());
        };
        let before = snapshot(self.engine.board());
        let revision = self.engine.revision();
        let next_id = &mut self.next_id;
        let mut ids = || {
            *next_id += 1;
            Uuid::from_u128(SOAK_IDS | *next_id)
        };
        let routed = route(&mut self.engine, &self.rules, self.classes, &plan, &mut ids)?;
        self.last = match &routed {
            Routed::Landed(draft) => format!("{plan:?} landed {draft:?}"),
            Routed::Refused => format!("{plan:?} refused"),
        };
        match routed {
            Routed::Landed(draft) => {
                self.tally.landed += 1;
                let board = self.engine.board();
                self.tally.overlapped_tracks += draft
                    .tracks
                    .iter()
                    .filter(|t| board.segments.iter().any(|s| overlaps(t, s)))
                    .count();
                self.stepped(before);
            }
            Routed::Refused => {
                self.tally.refused += 1;
                if self.engine.revision() != revision || snapshot(self.engine.board()) != before {
                    return Err("a refused route changed the board".into());
                }
            }
        }
        Ok(())
    }

    fn move_track(&mut self) -> Result<(), String> {
        let tracks: Vec<Segment> = self
            .engine
            .board()
            .segments
            .iter()
            .filter(|s| !s.locked)
            .cloned()
            .collect();
        let Some(track) = self.rng.pick(&tracks).cloned() else {
            return Ok(());
        };
        let (dx, dy) = (self.rng.offset_mm(1.0, 0.05), self.rng.offset_mm(1.0, 0.05));
        let shift = |p: Point| Point::new(p.x + dx, p.y + dy);
        let moved = Segment {
            start: shift(track.start),
            end: shift(track.end),
            ..track
        };
        self.edit(BoardCommand::ReplaceItems {
            items: vec![BoardItem::Segment(moved)],
        })
    }

    fn move_footprint(&mut self) -> Result<(), String> {
        let footprints: Vec<(Uuid, Point)> = self
            .engine
            .board()
            .footprints
            .iter()
            .filter(|f| !f.locked)
            .map(|f| (f.uuid, f.position))
            .collect();
        let Some(&(footprint, at)) = self.rng.pick(&footprints) else {
            return Ok(());
        };
        let to = Point::new(
            at.x + self.rng.offset_mm(2.0, 0.05),
            at.y + self.rng.offset_mm(2.0, 0.05),
        );
        self.edit(BoardCommand::MoveFootprint { footprint, to })
    }

    fn delete(&mut self) -> Result<(), String> {
        let board = self.engine.board();
        let items: Vec<BoardItemId> = board
            .segments
            .iter()
            .filter(|s| !s.locked)
            .map(|s| BoardItemId::Segment(s.uuid))
            .chain(
                board
                    .vias
                    .iter()
                    .filter(|v| !v.locked)
                    .map(|v| BoardItemId::Via(v.uuid)),
            )
            .collect();
        let Some(&item) = self.rng.pick(&items) else {
            return Ok(());
        };
        self.edit(BoardCommand::DeleteItems { items: vec![item] })
    }

    fn edit(&mut self, command: BoardCommand) -> Result<(), String> {
        self.last = format!("{command:?}");
        let before = snapshot(self.engine.board());
        self.engine
            .execute(command.clone())
            .map_err(|e| format!("the engine refused {command:?}: {e}"))?;
        self.tally.edits += 1;
        self.stepped(before);
        Ok(())
    }

    /// A step landed on a board that read `before`.
    fn stepped(&mut self, before: String) {
        self.undo.push(before);
        self.redo.clear();
        // The engine keeps its newest `undo_depth` steps.
        let excess = self.undo.len().saturating_sub(self.engine.undo_depth());
        self.undo.drain(..excess);
    }

    fn undo(&mut self) -> Result<(), String> {
        let now = snapshot(self.engine.board());
        let undone = self.engine.undo().map_err(|e| format!("undo: {e}"))?;
        match (undone, self.undo.pop()) {
            (None, None) => Ok(()),
            (Some(_), Some(earlier)) => {
                self.tally.undos += 1;
                self.redo.push(now);
                if snapshot(self.engine.board()) == earlier {
                    Ok(())
                } else {
                    Err("undo did not give the board back as it was".into())
                }
            }
            (undone, _) => Err(format!(
                "undo stepped: {}, but the soak expected the opposite",
                undone.is_some()
            )),
        }
    }

    fn redo(&mut self) -> Result<(), String> {
        let now = snapshot(self.engine.board());
        let redone = self.engine.redo().map_err(|e| format!("redo: {e}"))?;
        match (redone, self.redo.pop()) {
            (None, None) => Ok(()),
            (Some(_), Some(later)) => {
                self.tally.redos += 1;
                self.undo.push(now);
                if snapshot(self.engine.board()) == later {
                    Ok(())
                } else {
                    Err("redo did not give the board back as it was".into())
                }
            }
            (redone, _) => Err(format!(
                "redo stepped: {}, but the soak expected the opposite",
                redone.is_some()
            )),
        }
    }

    /// The board invariants after an action.
    fn check(&self) -> Result<(), String> {
        let board = self.engine.board();
        if self.engine.undo_len() != self.undo.len()
            || self.engine.can_redo() == self.redo.is_empty()
        {
            return Err(format!(
                "history out of step: engine undo {} / redo {}, expected {} / {}",
                self.engine.undo_len(),
                self.engine.can_redo(),
                self.undo.len(),
                !self.redo.is_empty()
            ));
        }
        unique_ids(board)?;
        let nets: HashSet<u32> = board.nets.iter().map(|n| n.number).collect();
        let known = |net: u32| net == 0 || nets.is_empty() || nets.contains(&net);
        for s in &board.segments {
            let finite = [s.start.x, s.start.y, s.end.x, s.end.y, s.width]
                .iter()
                .all(|v| v.is_finite());
            if !finite || s.width <= 0.0 {
                return Err(format!("a track has no real size: {s:?}"));
            }
            if !matches!(parse_pcb_layer(&s.layer), Some(PcbLayer::Copper(_))) {
                return Err(format!("a track is off copper: {s:?}"));
            }
            if !known(s.net) {
                return Err(format!("a track names net {} the board lacks", s.net));
            }
        }
        if let Some(v) = board.vias.iter().find(|v| !known(v.net)) {
            return Err(format!("a via names net {} the board lacks", v.net));
        }
        if zero_length(board) > self.initial_zero_length {
            return Err("a zero-length track appeared".into());
        }
        if let Some(line) = ratsnest(board).iter().find(|l| !known(l.net)) {
            return Err(format!(
                "the ratsnest names net {} the board lacks",
                line.net
            ));
        }
        Ok(())
    }
}

/// Every track, via, footprint and pad id once.
fn unique_ids(board: &PcbBoard) -> Result<(), String> {
    let mut seen = HashSet::new();
    let ids = board
        .segments
        .iter()
        .map(|s| s.uuid)
        .chain(board.vias.iter().map(|v| v.uuid))
        .chain(board.footprints.iter().map(|f| f.uuid))
        .chain(
            board
                .footprints
                .iter()
                .flat_map(|f| f.pads.iter().map(|p| p.uuid)),
        );
    for id in ids {
        if !seen.insert(id) {
            return Err(format!("id {id} is on the board twice"));
        }
    }
    Ok(())
}

fn zero_length(board: &PcbBoard) -> usize {
    board.segments.iter().filter(|s| s.start == s.end).count()
}

/// What "on the line" and "a shared stretch" mean for [`overlaps`], in mm.
const OVERLAP_MM: f64 = 1e-3;

/// Whether `track` lies over `other`, a different track on the same net and
/// layer: both on one line and sharing a stretch longer than
/// [`OVERLAP_MM`]. Meeting end to end is not an overlap.
pub fn overlaps(track: &Segment, other: &Segment) -> bool {
    if track.uuid == other.uuid || track.net != other.net || track.layer != other.layer {
        return false;
    }
    let (dx, dy) = (track.end.x - track.start.x, track.end.y - track.start.y);
    let length = dx.hypot(dy);
    if length <= OVERLAP_MM {
        return false;
    }
    // Signed distance from the track's line, and distance along it.
    let off = |p: Point| (dx * (p.y - track.start.y) - dy * (p.x - track.start.x)) / length;
    let along = |p: Point| (dx * (p.x - track.start.x) + dy * (p.y - track.start.y)) / length;
    if off(other.start).abs() > OVERLAP_MM || off(other.end).abs() > OVERLAP_MM {
        return false;
    }
    let (a, b) = (along(other.start), along(other.end));
    let shared = a.max(b).min(length) - a.min(b).max(0.0);
    shared > OVERLAP_MM
}
