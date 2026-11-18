//! The colour a junction dot takes from the wires it joins.
//!
//! A dot is drawn in the colour most of the wires through it share (each wire
//! with its own drawn colour: own, else net override, else class). When no
//! single colour has the most wires, or the default colour does, the dot
//! falls back to its own rule. The canvas, the SVG page (so PDF and print)
//! and the drag ghost all read this one function. Rule and examples:
//! `docs/RENDERING_RULES.md`, "Junction colour".

use std::collections::{BTreeMap, HashMap};

use uuid::Uuid;

use super::{Point, SchematicSheet, Wire};
use crate::theme::Color;

/// The 1 um key the netlist calls "the same point".
fn key(p: &Point) -> (i64, i64) {
    ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64)
}

/// Dots by row (`y` key to `(x key, dot index)`) and by column.
struct DotLines {
    rows: BTreeMap<i64, Vec<(i64, usize)>>,
    columns: BTreeMap<i64, Vec<(i64, usize)>>,
}

impl DotLines {
    fn of(dots: &[(i64, i64)]) -> Self {
        let mut rows: BTreeMap<i64, Vec<(i64, usize)>> = BTreeMap::new();
        let mut columns: BTreeMap<i64, Vec<(i64, usize)>> = BTreeMap::new();
        for (index, &(x, y)) in dots.iter().enumerate() {
            rows.entry(y).or_default().push((x, index));
            columns.entry(x).or_default().push((y, index));
        }
        Self { rows, columns }
    }
}

/// The dots on the span `a`-`b` (ends included).
fn dots_on(a: (i64, i64), b: (i64, i64), dots: &[(i64, i64)], lines: &DotLines) -> Vec<usize> {
    let along = |line: Option<&Vec<(i64, usize)>>, lo: i64, hi: i64| -> Vec<usize> {
        line.map(|line| {
            (line.iter())
                .filter(|(at, _)| *at >= lo && *at <= hi)
                .map(|(_, index)| *index)
                .collect()
        })
        .unwrap_or_default()
    };
    if a.1 == b.1 {
        return along(lines.rows.get(&a.1), a.0.min(b.0), a.0.max(b.0));
    }
    if a.0 == b.0 {
        return along(lines.columns.get(&a.0), a.1.min(b.1), a.1.max(b.1));
    }
    // A slanted wire is rare: test every dot.
    (dots.iter().enumerate())
        .filter(|(_, p)| on_slant(**p, a, b))
        .map(|(index, _)| index)
        .collect()
}

fn on_slant(p: (i64, i64), a: (i64, i64), b: (i64, i64)) -> bool {
    let cross =
        (b.0 - a.0) as i128 * (p.1 - a.1) as i128 - (b.1 - a.1) as i128 * (p.0 - a.0) as i128;
    cross == 0
        && p.0 >= a.0.min(b.0)
        && p.0 <= a.0.max(b.0)
        && p.1 >= a.1.min(b.1)
        && p.1 <= a.1.max(b.1)
}

/// The colour each dot of `sheet` takes from its wires, by dot uuid; a dot
/// left out keeps its own rule. `wire_color` is the colour a wire is drawn
/// with, `None` for the default.
#[must_use]
pub fn junction_wire_colors(
    sheet: &SchematicSheet,
    wire_color: impl Fn(&Wire) -> Option<Color>,
) -> HashMap<Uuid, Color> {
    if sheet.junctions.is_empty() {
        return HashMap::new();
    }
    let dots: Vec<(i64, i64)> = sheet.junctions.iter().map(|j| key(&j.position)).collect();
    let lines = DotLines::of(&dots);
    // Per dot: each colour met and how many wires carry it.
    let mut votes: Vec<Vec<(Option<Color>, usize)>> = vec![Vec::new(); dots.len()];
    for wire in &sheet.wires {
        let on = dots_on(key(&wire.start), key(&wire.end), &dots, &lines);
        if on.is_empty() {
            continue;
        }
        let color = wire_color(wire);
        for index in on {
            let tally = &mut votes[index];
            match tally.iter_mut().find(|(c, _)| *c == color) {
                Some((_, n)) => *n += 1,
                None => tally.push((color, 1)),
            }
        }
    }
    let mut out = HashMap::new();
    for (junction, tally) in sheet.junctions.iter().zip(votes) {
        if let Some(color) = majority(&tally) {
            out.insert(junction.uuid, color);
        }
    }
    out
}

/// The colour strictly more wires carry than any other, unless that is the
/// default.
fn majority(tally: &[(Option<Color>, usize)]) -> Option<Color> {
    let most = tally.iter().map(|(_, n)| *n).max()?;
    let mut leaders = tally.iter().filter(|(_, n)| *n == most);
    let (color, _) = leaders.next()?;
    if leaders.next().is_some() {
        return None;
    }
    *color
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::Junction;

    const GREEN: Color = Color {
        r: 0,
        g: 200,
        b: 0,
        a: 255,
    };
    const RED: Color = Color {
        r: 200,
        g: 0,
        b: 0,
        a: 255,
    };

    fn wire(a: (f64, f64), b: (f64, f64), color: Option<Color>) -> Wire {
        Wire {
            uuid: Uuid::now_v7(),
            start: Point::new(a.0, a.1),
            end: Point::new(b.0, b.1),
            color,
            ..Wire::default()
        }
    }

    fn sheet(wires: Vec<Wire>, dot: (f64, f64)) -> (SchematicSheet, Uuid) {
        let junction = Junction {
            uuid: Uuid::now_v7(),
            position: Point::new(dot.0, dot.1),
            ..Junction::default()
        };
        let id = junction.uuid;
        let sheet = SchematicSheet {
            wires,
            junctions: vec![junction],
            ..SchematicSheet::default()
        };
        (sheet, id)
    }

    fn own(wire: &Wire) -> Option<Color> {
        wire.color
    }

    #[test]
    fn a_dot_on_coloured_wires_takes_their_colour() {
        // A T: two wires end on the dot, a third passes through it.
        let (s, dot) = sheet(
            vec![
                wire((0.0, 0.0), (10.0, 0.0), Some(GREEN)),
                wire((10.0, 0.0), (20.0, 0.0), Some(GREEN)),
                wire((10.0, -10.0), (10.0, 10.0), Some(GREEN)),
            ],
            (10.0, 0.0),
        );
        assert_eq!(junction_wire_colors(&s, own).get(&dot), Some(&GREEN));
    }

    #[test]
    fn the_colour_most_wires_share_wins_and_a_tie_or_the_default_decides_nothing() {
        let cases = [
            (Some(GREEN), Some(GREEN), None, Some(GREEN)),
            (Some(GREEN), Some(RED), None, None),
            (None, None, Some(GREEN), None),
            (Some(GREEN), Some(RED), Some(RED), Some(RED)),
        ];
        for (a, b, c, want) in cases {
            let (s, dot) = sheet(
                vec![
                    wire((0.0, 0.0), (10.0, 0.0), a),
                    wire((10.0, 0.0), (20.0, 0.0), b),
                    wire((10.0, 0.0), (10.0, 10.0), c),
                ],
                (10.0, 0.0),
            );
            assert_eq!(
                junction_wire_colors(&s, own).get(&dot).copied(),
                want,
                "{a:?} {b:?} {c:?}"
            );
        }
    }

    #[test]
    fn a_wire_that_misses_the_dot_does_not_vote() {
        let (s, dot) = sheet(
            vec![
                wire((0.0, 0.0), (10.0, 0.0), Some(GREEN)),
                wire((0.0, 5.0), (20.0, 5.0), Some(RED)),
                wire((3.0, 3.0), (7.0, 7.0), Some(RED)),
            ],
            (10.0, 0.0),
        );
        assert_eq!(junction_wire_colors(&s, own).get(&dot), Some(&GREEN));
    }

    #[test]
    fn a_slanted_wire_through_the_dot_votes() {
        let (s, dot) = sheet(
            vec![wire((0.0, 0.0), (10.0, 10.0), Some(RED))],
            (5.0, 5.0),
        );
        assert_eq!(junction_wire_colors(&s, own).get(&dot), Some(&RED));
    }
}
