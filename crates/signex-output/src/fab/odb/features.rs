//! One ODB++ `features` file: the symbol table, the attribute names, then
//! one record per pad, line, arc and surface, in mark order. Coordinates
//! in mm, symbol sizes in micrometres, every feature positive.

use std::ops::Range;

use pcb_geom::PointNm;

use super::text::{cw_degrees, mm, um};
use crate::fab::marks::{ApertureShape, Function, Mark, Shape, Step, doubled_area};

/// The `.smd` attribute: a copper pad for surface mounting.
const SMD: &str = ".smd";

#[derive(Debug, Default)]
pub(crate) struct Features {
    symbols: Vec<String>,
    attributes: Vec<&'static str>,
    records: Vec<String>,
}

impl Features {
    /// Write `mark`; the numbers of the features it became.
    pub(crate) fn mark(&mut self, mark: &Mark) -> Range<usize> {
        let first = self.records.len();
        match mark {
            Mark::Flash {
                shape,
                function,
                at,
                ..
            } => {
                let smd = (*function == Function::SmdPad).then(|| self.attribute(SMD));
                let (standard, udeg) = Shape::of(shape);
                match shape_symbol(&standard) {
                    Some(symbol) => self.pad(*at, symbol, udeg, smd),
                    // A polygon pad has no standard symbol: its copper is
                    // a surface of its corners, with the pad's attribute.
                    None => {
                        if let ApertureShape::Outline { corners } = shape {
                            let outline: Vec<PointNm> = corners
                                .iter()
                                .map(|c| PointNm::new(at.x + c.x, at.y + c.y))
                                .collect();
                            self.filled(&outline, &[], smd);
                        }
                    }
                }
            }
            Mark::Stroke {
                width,
                start,
                steps,
                ..
            } => self.path(*start, steps, *width),
            Mark::Region { outline, holes, .. } => self.surface(outline, holes),
        }
        first..self.records.len()
    }

    /// A pad of `symbol` at `at`, turned `udeg` counter-clockwise.
    pub(crate) fn pad(&mut self, at: PointNm, symbol: String, udeg: i64, attribute: Option<usize>) {
        let symbol = self.symbol(symbol);
        let orient = if udeg.rem_euclid(360_000_000) == 0 {
            "0".to_string()
        } else {
            format!("8 {}", cw_degrees(udeg))
        };
        let attributes = attribute.map(|a| format!(";{a}")).unwrap_or_default();
        self.records.push(format!(
            "P {} {} {symbol} P 0 {orient}{attributes}",
            mm(at.x),
            mm(at.y)
        ));
    }

    /// A line from `from` to `to` drawn with a round pen of `width` nm.
    pub(crate) fn line(&mut self, from: PointNm, to: PointNm, width: i64) {
        let symbol = self.symbol(round_symbol(width));
        self.records.push(format!(
            "L {} {} {} {} {symbol} P 0",
            mm(from.x),
            mm(from.y),
            mm(to.x),
            mm(to.y)
        ));
    }

    /// A filled outline with polygon holes: the island clockwise, the
    /// holes counter-clockwise, each ring closed.
    pub(crate) fn surface(&mut self, outline: &[PointNm], holes: &[Vec<PointNm>]) {
        self.filled(outline, holes, None);
    }

    /// A surface with an optional attribute; nothing under three points.
    fn filled(&mut self, outline: &[PointNm], holes: &[Vec<PointNm>], attribute: Option<usize>) {
        if outline.len() < 3 {
            return;
        }
        let attributes = attribute.map(|a| format!(";{a}")).unwrap_or_default();
        let mut record = format!("S P 0{attributes}\n");
        ring(&mut record, outline, false);
        for hole in holes.iter().filter(|h| h.len() >= 3) {
            ring(&mut record, hole, true);
        }
        record.push_str("SE");
        self.records.push(record);
    }

    /// The file's text.
    pub(crate) fn finish(self) -> String {
        let mut out = String::from("UNITS=MM\n#\n#Feature symbol names\n#\n");
        for (i, symbol) in self.symbols.iter().enumerate() {
            out.push_str(&format!("${i} {symbol}\n"));
        }
        out.push_str("#\n#Feature attribute names\n#\n");
        for (i, attribute) in self.attributes.iter().enumerate() {
            out.push_str(&format!("@{i} {attribute}\n"));
        }
        out.push_str("#\n#Layer features\n#\n");
        for record in &self.records {
            out.push_str(record);
            out.push('\n');
        }
        out
    }

    /// A stroked path: a line or arc per step. A full circle (an arc back
    /// to its start) is two half arcs: a curve that ends where it starts
    /// names no turn.
    fn path(&mut self, start: PointNm, steps: &[Step], width: i64) {
        let mut from = start;
        for step in steps {
            match *step {
                Step::Line(to) => {
                    self.line(from, to, width);
                    from = to;
                }
                Step::Arc { to, center, ccw } => {
                    if to == from {
                        let opposite = PointNm::new(2 * center.x - from.x, 2 * center.y - from.y);
                        self.arc(from, opposite, center, ccw, width);
                        from = opposite;
                    }
                    self.arc(from, to, center, ccw, width);
                    from = to;
                }
            }
        }
    }

    fn arc(&mut self, from: PointNm, to: PointNm, center: PointNm, ccw: bool, width: i64) {
        let symbol = self.symbol(round_symbol(width));
        let clockwise = if ccw { "N" } else { "Y" };
        self.records.push(format!(
            "A {} {} {} {} {} {} {symbol} P 0 {clockwise}",
            mm(from.x),
            mm(from.y),
            mm(to.x),
            mm(to.y),
            mm(center.x),
            mm(center.y)
        ));
    }

    /// The number of `symbol`, adding it.
    fn symbol(&mut self, symbol: String) -> usize {
        match self.symbols.iter().position(|s| *s == symbol) {
            Some(i) => i,
            None => {
                self.symbols.push(symbol);
                self.symbols.len() - 1
            }
        }
    }

    fn attribute(&mut self, name: &'static str) -> usize {
        match self.attributes.iter().position(|a| *a == name) {
            Some(i) => i,
            None => {
                self.attributes.push(name);
                self.attributes.len() - 1
            }
        }
    }
}

/// The standard symbol of an unturned shape, sizes in micrometres.
/// `None` for a polygon, which no standard symbol draws.
fn shape_symbol(shape: &Shape) -> Option<String> {
    Some(match *shape {
        Shape::Circle { diameter } => round_symbol(diameter),
        Shape::Rect { w, h } => format!("rect{}x{}", um(w), um(h)),
        Shape::Oval { w, h } => format!("oval{}x{}", um(w), um(h)),
        Shape::RoundRect { w, h, r } => format!("rect{}x{}xr{}", um(w), um(h), um(r)),
        Shape::Outline { .. } => return None,
    })
}

/// The round symbol of `diameter` nm.
pub(crate) fn round_symbol(diameter: i64) -> String {
    format!("r{}", um(diameter))
}

/// One closed ring of a surface: `OB` at the first point, `OS` to each
/// next and back to the first, `OE`. `hole` picks the winding.
pub(super) fn ring(record: &mut String, points: &[PointNm], hole: bool) {
    let mut points = points.to_vec();
    if points.first() == points.last() {
        points.pop();
    }
    let ccw = doubled_area(&points) > 0;
    if ccw != hole {
        points.reverse();
    }
    let Some(first) = points.first().copied() else {
        return;
    };
    let kind = if hole { "H" } else { "I" };
    record.push_str(&format!("OB {} {} {kind}\n", mm(first.x), mm(first.y)));
    for p in points.iter().skip(1).chain(std::iter::once(&first)) {
        record.push_str(&format!("OS {} {}\n", mm(p.x), mm(p.y)));
    }
    record.push_str("OE\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fab::marks::{ApertureShape, Attributes};

    #[test]
    fn a_turned_smd_pad_names_its_upright_symbol_and_turns_clockwise() {
        let mut features = Features::default();
        let pad = Mark::Flash {
            shape: ApertureShape::TurnedRect {
                w: 1_000_000,
                h: 500_000,
                udeg: 30_000_000,
            },
            function: Function::SmdPad,
            at: PointNm::new(2_000_000, -1_500_000),
            attributes: Attributes::default(),
        };
        assert_eq!(features.mark(&pad), 0..1);
        let text = features.finish();
        assert!(text.contains("$0 rect1000x500\n"), "{text}");
        assert!(text.contains("@0 .smd\n"), "{text}");
        assert!(text.contains("P 2 -1.5 0 P 0 8 330;0\n"), "{text}");
    }

    #[test]
    fn a_polygon_pad_is_one_surface_with_the_pads_attribute() {
        let mut features = Features::default();
        let pad = Mark::Flash {
            shape: ApertureShape::Outline {
                corners: vec![
                    PointNm::new(-500_000, -500_000),
                    PointNm::new(500_000, -500_000),
                    PointNm::new(500_000, 500_000),
                    PointNm::new(-500_000, 500_000),
                ],
            },
            function: Function::SmdPad,
            at: PointNm::new(2_000_000, 0),
            attributes: Attributes::default(),
        };
        assert_eq!(features.mark(&pad), 0..1);
        let text = features.finish();
        assert!(!text.contains("$0"), "no symbol: {text}");
        assert!(text.contains("@0 .smd\n"), "{text}");
        assert!(
            text.contains(
                "S P 0;0\nOB 1.5 0.5 I\nOS 2.5 0.5\nOS 2.5 -0.5\nOS 1.5 -0.5\nOS 1.5 0.5\nOE\nSE\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn a_full_circle_is_two_half_arcs_and_a_surface_winds_island_clockwise() {
        let mut features = Features::default();
        let circle = Mark::circle(PointNm::new(0, 0), 1_000_000, 100_000, Function::Graphic);
        assert_eq!(features.mark(&circle), 0..2);
        let square = Mark::Region {
            outline: vec![
                PointNm::new(0, 0),
                PointNm::new(1_000_000, 0),
                PointNm::new(1_000_000, 1_000_000),
                PointNm::new(0, 1_000_000),
            ],
            holes: Vec::new(),
            attributes: Attributes::default(),
        };
        assert_eq!(features.mark(&square), 2..3);
        let text = features.finish();
        assert!(text.contains("A 1 0 -1 0 0 0 0 P 0 N\n"), "{text}");
        assert!(text.contains("A -1 0 1 0 0 0 0 P 0 N\n"), "{text}");
        assert!(
            text.contains("S P 0\nOB 0 1 I\nOS 1 1\nOS 1 0\nOS 0 0\nOS 0 1\nOE\nSE\n"),
            "{text}"
        );
    }
}
