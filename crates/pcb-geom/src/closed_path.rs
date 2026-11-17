//! A closed path from loose lines (#2409): the corners of the one loop a
//! set of line pieces makes, end to end. Two ends join when they are the
//! same nm point. The board outline and a keepout drawn from selected
//! lines are built from it.

use signex_types::pcb::Point;

use crate::units::PointNm;

/// Why the lines do not make one closed loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRefusal {
    /// Fewer than three lines, or a line whose end the geometry cannot hold.
    TooFew,
    /// An end meets no other line, or more than one: the path is open or
    /// branches there.
    Open(PointNm),
    /// The lines close into more than one loop.
    SeveralLoops,
}

impl std::fmt::Display for PathRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFew => write!(f, "it takes three or more lines to close a shape"),
            Self::Open(at) => {
                let p = at.to_mm();
                write!(f, "the lines do not close at {:.4},{:.4} mm", p.x, p.y)
            }
            Self::SeveralLoops => write!(f, "the lines make more than one closed shape"),
        }
    }
}

/// The corners of the loop `lines` make, in order around it, starting at
/// the first line's start; the closing corner is not repeated. A
/// zero-length line is passed over.
pub fn closed_path(lines: &[(Point, Point)]) -> Result<Vec<Point>, PathRefusal> {
    let mut edges = Vec::with_capacity(lines.len());
    for &(a, b) in lines {
        let (Ok(p), Ok(q)) = (PointNm::from_mm(a, "line"), PointNm::from_mm(b, "line")) else {
            return Err(PathRefusal::TooFew);
        };
        if p != q {
            edges.push(((p, a), (q, b)));
        }
    }
    if edges.len() < 3 {
        return Err(PathRefusal::TooFew);
    }
    for &((p, _), (q, _)) in &edges {
        for end in [p, q] {
            let meeting = edges
                .iter()
                .filter(|((a, _), (b, _))| *a == end || *b == end)
                .count();
            if meeting != 2 {
                return Err(PathRefusal::Open(end));
            }
        }
    }
    let mut used = vec![false; edges.len()];
    used[0] = true;
    let ((start, start_mm), (mut at, mut at_mm)) = edges[0];
    let mut corners = vec![start_mm];
    while at != start {
        corners.push(at_mm);
        let next = (0..edges.len()).find(|&i| {
            let ((a, _), (b, _)) = edges[i];
            !used[i] && (a == at || b == at)
        });
        let Some(i) = next else {
            return Err(PathRefusal::Open(at));
        };
        used[i] = true;
        let ((a, a_mm), (b, b_mm)) = edges[i];
        (at, at_mm) = if a == at { (b, b_mm) } else { (a, a_mm) };
    }
    if used.iter().any(|u| !u) {
        return Err(PathRefusal::SeveralLoops);
    }
    Ok(corners)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: (f64, f64), b: (f64, f64)) -> (Point, Point) {
        (Point::new(a.0, a.1), Point::new(b.0, b.1))
    }

    #[test]
    fn four_lines_in_any_order_and_direction_close_a_rectangle() {
        let lines = [
            line((0.0, 0.0), (10.0, 0.0)),
            line((10.0, 5.0), (0.0, 5.0)),
            line((10.0, 5.0), (10.0, 0.0)),
            line((0.0, 5.0), (0.0, 0.0)),
        ];
        let corners = closed_path(&lines).expect("closed");
        assert_eq!(
            corners,
            [
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 5.0),
                Point::new(0.0, 5.0),
            ]
        );
    }

    #[test]
    fn an_open_path_is_refused_where_it_opens() {
        let lines = [
            line((0.0, 0.0), (10.0, 0.0)),
            line((10.0, 0.0), (10.0, 5.0)),
            line((10.0, 5.0), (0.0, 5.0)),
        ];
        assert_eq!(
            closed_path(&lines),
            Err(PathRefusal::Open(PointNm::new(0, 0)))
        );
    }

    #[test]
    fn two_loops_and_too_few_lines_are_refused() {
        let square = |o: f64| {
            [
                line((o, 0.0), (o + 1.0, 0.0)),
                line((o + 1.0, 0.0), (o + 1.0, 1.0)),
                line((o + 1.0, 1.0), (o, 1.0)),
                line((o, 1.0), (o, 0.0)),
            ]
        };
        let mut two = square(0.0).to_vec();
        two.extend(square(5.0));
        assert_eq!(closed_path(&two), Err(PathRefusal::SeveralLoops));
        assert_eq!(closed_path(&square(0.0)[..2]), Err(PathRefusal::TooFew));
    }
}
