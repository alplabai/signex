//! Imported drawing to sketch geometry (mockup row 179c). Each piece becomes
//! a sketch entity. With `join` on, ends within the one heal tolerance share
//! a point (Coincident by construction) and exactly level or plumb lines take
//! Horizontal or Vertical. Pure: the caller adds the result to a sketch.

use std::collections::HashMap;

use crate::constraint::{Constraint, ConstraintKind};
use crate::entity::{Entity, EntityKind};
use crate::heal::{HEAL_TOLERANCE_MM, heals};
use crate::id::{ConstraintId, SketchEntityId};
use crate::plane::PlaneId;

/// Two coordinates closer than this (mm) are the same: a joined end this
/// close was not a gap, and a line this level is horizontal.
const SAME_MM: f64 = 1.0e-6;

/// One imported shape, in sketch coordinates (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImportShape {
    Line {
        a: (f64, f64),
        b: (f64, f64),
    },
    /// Counter-clockwise from `start` to `end`.
    Arc {
        center: (f64, f64),
        start: (f64, f64),
        end: (f64, f64),
    },
    Circle {
        center: (f64, f64),
        radius: f64,
    },
}

/// A shape and the caller's tag for it (the layer it came from).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportPiece {
    pub shape: ImportShape,
    pub tag: usize,
}

/// What an import would make, shown before anything changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportCheck {
    pub lines: usize,
    pub arcs: usize,
    pub circles: usize,
    /// End gaps closed under the heal tolerance.
    pub healed: usize,
    /// Ends that meet nothing.
    pub open_ends: usize,
    /// Edges shorter than the tolerance, left out.
    pub dropped: usize,
    /// Horizontal and Vertical constraints added.
    pub constraints: usize,
}

/// The entities and constraints of an import.
#[derive(Debug, Clone, PartialEq)]
pub struct SketchImport {
    pub entities: Vec<Entity>,
    pub constraints: Vec<Constraint>,
    /// Each edge with the tag of its piece.
    pub edges: Vec<(SketchEntityId, usize)>,
    pub check: ImportCheck,
}

/// The end points of an import, joined within the tolerance when `join`.
struct Ends {
    join: bool,
    ids: Vec<SketchEntityId>,
    at: Vec<(f64, f64)>,
    uses: Vec<u32>,
    cells: HashMap<(i64, i64), Vec<usize>>,
    healed: usize,
}

impl Ends {
    fn new(join: bool) -> Self {
        Self {
            join,
            ids: Vec::new(),
            at: Vec::new(),
            uses: Vec::new(),
            cells: HashMap::new(),
            healed: 0,
        }
    }

    fn cell(p: (f64, f64)) -> (i64, i64) {
        (
            (p.0 / HEAL_TOLERANCE_MM).floor() as i64,
            (p.1 / HEAL_TOLERANCE_MM).floor() as i64,
        )
    }

    /// The nearest end within the tolerance, and how far it is.
    fn nearest(&self, p: (f64, f64)) -> Option<(usize, f64)> {
        let (cx, cy) = Self::cell(p);
        let mut best: Option<(usize, f64)> = None;
        for x in cx - 1..=cx + 1 {
            for y in cy - 1..=cy + 1 {
                for &i in self.cells.get(&(x, y)).into_iter().flatten() {
                    let gap = (self.at[i].0 - p.0).hypot(self.at[i].1 - p.1);
                    if heals(gap) && best.is_none_or(|(_, b)| gap < b) {
                        best = Some((i, gap));
                    }
                }
            }
        }
        best
    }

    /// The end at `p`: a joined one, else a new one.
    fn end(&mut self, p: (f64, f64)) -> usize {
        if self.join
            && let Some((i, gap)) = self.nearest(p)
        {
            self.healed += usize::from(gap > SAME_MM);
            return i;
        }
        let i = self.ids.len();
        self.ids.push(SketchEntityId::new());
        self.at.push(p);
        self.uses.push(0);
        self.cells.entry(Self::cell(p)).or_default().push(i);
        i
    }

    /// Two ends of one edge; `None` when they are one point.
    fn pair(&mut self, a: (f64, f64), b: (f64, f64)) -> Option<(usize, usize)> {
        if (a.0 - b.0).hypot(a.1 - b.1) <= SAME_MM {
            return None;
        }
        let healed = self.healed;
        let (i, j) = (self.end(a), self.end(b));
        if i == j {
            self.healed = healed;
            return None;
        }
        self.uses[i] += 1;
        self.uses[j] += 1;
        Some((i, j))
    }
}

/// The sketch geometry of `pieces` on `plane`.
pub fn build_import(pieces: &[ImportPiece], plane: PlaneId, join: bool) -> SketchImport {
    let mut ends = Ends::new(join);
    let mut edges: Vec<Entity> = Vec::new();
    let mut centers: Vec<Entity> = Vec::new();
    let mut tagged = Vec::new();
    let mut constraints = Vec::new();
    let mut check = ImportCheck::default();
    let point = |(x, y): (f64, f64)| EntityKind::Point { x, y };
    for piece in pieces {
        let id = SketchEntityId::new();
        let kind = match piece.shape {
            ImportShape::Line { a, b } => {
                let Some((i, j)) = ends.pair(a, b) else {
                    check.dropped += 1;
                    continue;
                };
                check.lines += 1;
                if join {
                    constraints.extend(level_constraint(id, ends.at[i], ends.at[j]));
                }
                EntityKind::Line {
                    start: ends.ids[i],
                    end: ends.ids[j],
                }
            }
            ImportShape::Arc { center, start, end } => {
                let Some((i, j)) = ends.pair(start, end) else {
                    check.dropped += 1;
                    continue;
                };
                check.arcs += 1;
                let hub = SketchEntityId::new();
                centers.push(Entity::new(hub, plane, point(center)));
                EntityKind::Arc {
                    center: hub,
                    start: ends.ids[i],
                    end: ends.ids[j],
                    sweep_ccw: true,
                }
            }
            ImportShape::Circle { center, radius } => {
                check.circles += 1;
                let hub = SketchEntityId::new();
                centers.push(Entity::new(hub, plane, point(center)));
                EntityKind::Circle {
                    center: hub,
                    radius,
                }
            }
        };
        edges.push(Entity::new(id, plane, kind));
        tagged.push((id, piece.tag));
    }
    check.healed = ends.healed;
    check.open_ends = ends.uses.iter().filter(|&&n| n == 1).count();
    check.constraints = constraints.len();
    let used = (0..ends.ids.len()).filter(|&i| ends.uses[i] > 0);
    let mut entities: Vec<Entity> = used
        .map(|i| Entity::new(ends.ids[i], plane, point(ends.at[i])))
        .collect();
    entities.append(&mut centers);
    entities.append(&mut edges);
    SketchImport {
        entities,
        constraints,
        edges: tagged,
        check,
    }
}

/// Horizontal or Vertical for a line that is exactly level or plumb.
fn level_constraint(line: SketchEntityId, a: (f64, f64), b: (f64, f64)) -> Option<Constraint> {
    let kind = if (a.1 - b.1).abs() <= SAME_MM {
        ConstraintKind::Horizontal { line }
    } else if (a.0 - b.0).abs() <= SAME_MM {
        ConstraintKind::Vertical { line }
    } else {
        return None;
    };
    Some(Constraint {
        id: ConstraintId::new(),
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: (f64, f64), b: (f64, f64)) -> ImportPiece {
        ImportPiece {
            shape: ImportShape::Line { a, b },
            tag: 0,
        }
    }

    /// A 10 mm square whose last edge stops `gap` short of the first corner.
    fn square(gap: f64) -> Vec<ImportPiece> {
        vec![
            line((0.0, 0.0), (10.0, 0.0)),
            line((10.0, 0.0), (10.0, 10.0)),
            line((10.0, 10.0), (0.0, 10.0)),
            line((0.0, 10.0), (0.0, gap)),
        ]
    }

    #[test]
    fn a_gap_under_the_tolerance_heals_and_the_square_closes() {
        let import = build_import(&square(0.03), PlaneId::new(), true);
        assert_eq!(import.check.lines, 4);
        assert_eq!(import.check.healed, 1);
        assert_eq!(import.check.open_ends, 0);
        let points = import
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
            .count();
        assert_eq!(points, 4, "the ends share corners");
    }

    #[test]
    fn a_gap_over_the_tolerance_stays_open_and_is_counted() {
        let import = build_import(&square(0.2), PlaneId::new(), true);
        assert_eq!(import.check.healed, 0);
        assert_eq!(import.check.open_ends, 2);
    }

    #[test]
    fn level_and_plumb_lines_take_their_constraints_only_when_joining() {
        let joined = build_import(&square(0.0), PlaneId::new(), true);
        assert_eq!(joined.check.constraints, 4);
        let apart = build_import(&square(0.0), PlaneId::new(), false);
        assert!(apart.constraints.is_empty());
        assert_eq!(apart.check.open_ends, 8, "every edge keeps its own ends");
    }

    #[test]
    fn an_edge_shorter_than_the_tolerance_is_left_out_with_no_stray_point() {
        let import = build_import(&[line((0.0, 0.0), (0.01, 0.0))], PlaneId::new(), true);
        assert_eq!(import.check.dropped, 1);
        assert!(import.entities.is_empty());
    }

    #[test]
    fn each_edge_keeps_its_piece_tag() {
        let pieces = [ImportPiece {
            shape: ImportShape::Circle {
                center: (1.0, 1.0),
                radius: 0.5,
            },
            tag: 7,
        }];
        let import = build_import(&pieces, PlaneId::new(), true);
        assert_eq!(import.edges.len(), 1);
        assert_eq!(import.edges[0].1, 7);
        assert_eq!(import.check.circles, 1);
    }
}
