//! Auto Place's score (#2941): one number per priority, in mm, lower is
//! better. A part's score at a place is read against the parts whose place
//! is settled; the board's score sums every pair once.

use pcb_geom::RectNm;

use super::input::{PartRoles, Priority, Weights};
use crate::pcb::rooms::BoxMm;

/// The score of each priority, mm.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Scores {
    /// Shared nets times the distance between the parts' centres.
    pub connection: f64,
    /// How far thermal pairs come inside the thermal gap.
    pub thermal: f64,
    /// How far noisy and sensitive pairs come inside the separation gap.
    pub separation: f64,
    /// How far connectors sit from the nearest board edge.
    pub edges: f64,
}

impl Scores {
    pub fn of(&self, priority: Priority) -> f64 {
        match priority {
            Priority::Connection => self.connection,
            Priority::Thermal => self.thermal,
            Priority::Separation => self.separation,
            Priority::Edges => self.edges,
        }
    }

    /// The weighted sum.
    pub fn total(&self, weights: &Weights) -> f64 {
        Priority::ALL
            .into_iter()
            .map(|p| weights.of(p) * self.of(p))
            .sum()
    }

    fn add(&mut self, other: Scores) {
        self.connection += other.connection;
        self.thermal += other.thermal;
        self.separation += other.separation;
        self.edges += other.edges;
    }
}

/// The gaps the thermal and separation priorities keep, mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gaps {
    pub thermal_mm: f64,
    pub separation_mm: f64,
}

/// What the score reads of one part: its nets (sorted, no net 0) and roles.
#[derive(Debug, Clone, Default)]
pub(super) struct Traits {
    pub nets: Vec<u32>,
    pub roles: PartRoles,
}

impl Traits {
    /// How many nets the two parts share.
    pub fn shared(&self, other: &Traits) -> usize {
        self.nets
            .iter()
            .filter(|n| other.nets.binary_search(n).is_ok())
            .count()
    }
}

fn mm(nm: i64) -> f64 {
    pcb_geom::nm_to_mm(nm)
}

/// The distance between the centres of two boxes, mm.
pub(super) fn centre_distance(a: &RectNm, b: &RectNm) -> f64 {
    let centre = |r: &RectNm| {
        (
            (mm(r.min.x) + mm(r.max.x)) / 2.0,
            (mm(r.min.y) + mm(r.max.y)) / 2.0,
        )
    };
    let ((ax, ay), (bx, by)) = (centre(a), centre(b));
    (ax - bx).hypot(ay - by)
}

/// The gap between two boxes edge to edge, mm; 0 when they meet.
pub(super) fn box_gap(a: &RectNm, b: &RectNm) -> f64 {
    let dx = (a.min.x - b.max.x).max(b.min.x - a.max.x).max(0);
    let dy = (a.min.y - b.max.y).max(b.min.y - a.max.y).max(0);
    mm(dx).hypot(mm(dy))
}

/// How far a box is from the nearest edge of the board's box, mm; 0 on or
/// past it.
pub(super) fn edge_distance(a: &RectNm, board: &BoxMm) -> f64 {
    [
        mm(a.min.x) - board.min.x,
        board.max.x - mm(a.max.x),
        mm(a.min.y) - board.min.y,
        board.max.y - mm(a.max.y),
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min)
    .max(0.0)
}

/// The score of a part with `traits` at `at` against one other part.
pub(super) fn pair_score(
    (traits, at): (&Traits, &RectNm),
    (other, other_at): (&Traits, &RectNm),
    gaps: &Gaps,
) -> Scores {
    let mut score = Scores::default();
    let shared = traits.shared(other);
    if shared > 0 {
        score.connection = shared as f64 * centre_distance(at, other_at);
    }
    let gap = || box_gap(at, other_at);
    if traits.roles.thermal_pair(other.roles) {
        score.thermal = (gaps.thermal_mm - gap()).max(0.0);
    }
    if traits.roles.separation_pair(other.roles) {
        score.separation = (gaps.separation_mm - gap()).max(0.0);
    }
    score
}

/// The board's score: every pair of placed parts with at least one in
/// `counted`, once, and the edge distance of each counted connector.
pub(super) fn board_scores(
    traits: &[Traits],
    boxes: &[Option<RectNm>],
    counted: &[bool],
    board: &BoxMm,
    gaps: &Gaps,
) -> Scores {
    let mut total = Scores::default();
    for (i, part) in traits.iter().enumerate() {
        let Some(at) = boxes[i].as_ref() else {
            continue;
        };
        if counted[i] && part.roles.connector {
            total.edges += edge_distance(at, board);
        }
        for (j, other) in traits.iter().enumerate().skip(i + 1) {
            if !(counted[i] || counted[j]) {
                continue;
            }
            if let Some(other_at) = boxes[j].as_ref() {
                total.add(pair_score((part, at), (other, other_at), gaps));
            }
        }
    }
    total
}

/// The parts a place is scored against: each one's traits, its box (`None`
/// for a body that does not convert) and whether its place is settled.
pub(super) struct Field<'a> {
    pub traits: &'a [Traits],
    pub boxes: &'a [Option<RectNm>],
    pub settled: &'a [bool],
    pub board: &'a BoxMm,
    pub gaps: &'a Gaps,
}

impl Field<'_> {
    /// The score of part `i` at `at` against the settled parts.
    pub fn place_score(&self, i: usize, at: &RectNm) -> Scores {
        let mut total = Scores::default();
        if self.traits[i].roles.connector {
            total.edges = edge_distance(at, self.board);
        }
        for (j, other_at) in self.boxes.iter().enumerate() {
            if j == i || !self.settled[j] {
                continue;
            }
            if let Some(other_at) = other_at {
                let pair = pair_score(
                    (&self.traits[i], at),
                    (&self.traits[j], other_at),
                    self.gaps,
                );
                total.add(pair);
            }
        }
        total
    }
}
