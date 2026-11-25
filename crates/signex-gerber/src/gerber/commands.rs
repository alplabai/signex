//! Word commands: G codes, aperture selection and the three operations
//! (`D01` draw, `D02` move, `D03` flash), with modal coordinates, arcs and
//! region contours. `G54` (a select prefix) and `G70`/`G71` (inch and mm,
//! deprecated forms of `%MO`) are accepted; `G55` and `M01` change nothing.

use pcb_geom::{PointNm, Shape};

use super::arcs::{arc_points, capsule_chain, single_quadrant_center};
use super::state::{Interpolation, MAX_OBJECTS, Pen, Quadrant, Reader, Region, Template};
use crate::image::{ObjectKind, Units, polygon_shape};
use crate::number::coordinate_to_nm;
use crate::problem::ProblemKind;

/// The letters of one word command and their values.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Word<'t> {
    pub g: Option<u32>,
    pub d: Option<u32>,
    pub m: Option<u32>,
    pub x: Option<&'t str>,
    pub y: Option<&'t str>,
    pub i: Option<&'t str>,
    pub j: Option<&'t str>,
}

impl Word<'_> {
    fn has_coordinates(&self) -> bool {
        self.x.is_some() || self.y.is_some() || self.i.is_some() || self.j.is_some()
    }
}

/// Split a word into its letters; `None` for an unknown or repeated letter
/// or a letter without a value.
pub(super) fn split_word(text: &str) -> Option<Word<'_>> {
    let mut word = Word::default();
    let mut rest = text;
    while let Some(letter) = rest.chars().next() {
        let after = &rest[letter.len_utf8()..];
        let len = after
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, '+' | '-' | '.')))
            .unwrap_or(after.len());
        let (value, tail) = after.split_at(len);
        if value.is_empty() {
            return None;
        }
        let code = || value.parse::<u32>().ok();
        let filled = match letter {
            'G' => word.g.replace(code()?).is_none(),
            'D' => word.d.replace(code()?).is_none(),
            'M' => word.m.replace(code()?).is_none(),
            'X' => word.x.replace(value).is_none(),
            'Y' => word.y.replace(value).is_none(),
            'I' => word.i.replace(value).is_none(),
            'J' => word.j.replace(value).is_none(),
            _ => false,
        };
        if !filled {
            return None;
        }
        rest = tail;
    }
    Some(word)
}

/// Whether a word is a `G04` comment (also written `G4`).
fn is_comment(text: &str) -> bool {
    let Some(rest) = text.strip_prefix('G') else {
        return false;
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<u32>() == Ok(4)
}

impl Reader<'_> {
    pub(super) fn word(&mut self, text: &str, line: u32) {
        if is_comment(text) {
            return;
        }
        let Some(word) = split_word(text) else {
            self.unsupported(line, text);
            return;
        };
        if let Some(m) = word.m {
            match m {
                0 | 2 => self.ended = true,
                1 => {}
                _ => self.unsupported(line, text),
            }
            return;
        }
        if let Some(g) = word.g
            && !self.g_code(g, line)
        {
            self.unsupported(line, text);
            return;
        }
        match word.d {
            Some(d) if d >= 10 && !word.has_coordinates() => self.select(d, line),
            Some(op @ 1..=3) => self.operate(op, &word, line),
            Some(_) => self.unsupported(line, text),
            None if word.has_coordinates() => match self.operation {
                // A coordinate word without an operation repeats the last.
                Some(op) => self.operate(op, &word, line),
                None => self.unsupported(line, text),
            },
            None => {}
        }
    }

    /// Apply a G code; false when it is not supported.
    fn g_code(&mut self, g: u32, line: u32) -> bool {
        match g {
            1 => self.interpolation = Interpolation::Linear,
            2 => self.interpolation = Interpolation::Clockwise,
            3 => self.interpolation = Interpolation::CounterClockwise,
            36 => self.start_region(line),
            37 => self.end_region(line),
            74 => self.quadrant = Quadrant::Single,
            75 => self.quadrant = Quadrant::Multi,
            70 => self.image.units = Some(Units::Inches),
            71 => self.image.units = Some(Units::Millimetres),
            54 | 55 | 90 => {}
            _ => return false,
        }
        true
    }

    fn select(&mut self, d_code: u32, line: u32) {
        self.aperture = Some(d_code);
        if !self.templates.contains_key(&d_code) {
            self.undefined.entry(d_code).or_insert((line, 0));
        }
    }

    fn operate(&mut self, op: u32, word: &Word, line: u32) {
        self.operation = Some(op);
        let Some(target) = self.target(word, line) else {
            return;
        };
        match op {
            1 => self.draw(target, word, line),
            2 => self.move_to(target, line),
            _ => self.flash(target, line),
        }
    }

    /// A coordinate in nm, reported when it cannot be read.
    fn coordinate(&mut self, text: &str, line: u32) -> Option<i64> {
        let (format, units) = (self.format(line), self.units(line));
        let nm = coordinate_to_nm(text, format, units);
        if nm.is_none() {
            self.report(line, ProblemKind::BadCoordinate);
        }
        nm
    }

    /// Where the word goes: X and Y keep their last value when absent.
    fn target(&mut self, word: &Word, line: u32) -> Option<PointNm> {
        let x = match word.x {
            Some(x) => self.coordinate(x, line)?,
            None => self.point.x,
        };
        let y = match word.y {
            Some(y) => self.coordinate(y, line)?,
            None => self.point.y,
        };
        Some(PointNm::new(x, y))
    }

    /// The arc centre offset; an absent I or J is 0.
    fn offset(&mut self, word: &Word, line: u32) -> Option<PointNm> {
        let i = match word.i {
            Some(i) => self.coordinate(i, line)?,
            None => 0,
        };
        let j = match word.j {
            Some(j) => self.coordinate(j, line)?,
            None => 0,
        };
        Some(PointNm::new(i, j))
    }

    fn move_to(&mut self, target: PointNm, line: u32) {
        if let Some(region) = &mut self.region {
            let finished = std::mem::take(&mut region.contour);
            let started = region.line;
            region.line = line;
            self.contour_done(finished, started, line);
        }
        self.point = target;
    }

    fn draw(&mut self, target: PointNm, word: &Word, line: u32) {
        let from = self.point;
        let Some(arc) = self.arc_of(from, target, word, line) else {
            return;
        };
        self.point = target;
        let points = match arc {
            Some((center, ccw)) => {
                arc_points(from, target, center, ccw, self.quadrant == Quadrant::Multi)
            }
            None => vec![target],
        };
        if let Some(region) = &mut self.region {
            if region.contour.is_empty() {
                region.contour.push(from);
            }
            region.contour.extend(points);
            return;
        }
        let kind = match arc {
            Some((center, ccw)) => ObjectKind::Arc {
                from,
                to: target,
                center,
                ccw,
            },
            None => ObjectKind::Line { from, to: target },
        };
        let shapes = self.stroke_shapes(from, &points, arc.is_none(), line);
        let d_code = self.aperture.unwrap_or(0);
        self.count_use(d_code, line);
        self.push(kind, self.aperture, shapes, line);
    }

    /// The centre and sense of a circular draw, `None` inside for a line;
    /// `None` outside when an offset cannot be read.
    fn arc_of(
        &mut self,
        from: PointNm,
        to: PointNm,
        word: &Word,
        line: u32,
    ) -> Option<Option<(PointNm, bool)>> {
        let ccw = match self.interpolation {
            Interpolation::Linear => return Some(None),
            Interpolation::Clockwise => false,
            Interpolation::CounterClockwise => true,
        };
        let offset = self.offset(word, line)?;
        let center = match self.quadrant {
            Quadrant::Multi => from.plus(offset),
            Quadrant::Single => single_quadrant_center(from, to, offset, ccw),
        };
        Some(Some((center, ccw)))
    }

    /// The selected pen drawn from `from` through `points`. A rectangle
    /// pen is drawn along a straight line only; any other aperture cannot
    /// draw, which is reported once per D code and drawn as a marker.
    fn stroke_shapes(
        &mut self,
        from: PointNm,
        points: &[PointNm],
        straight: bool,
        line: u32,
    ) -> Vec<Shape> {
        let d_code = self.aperture.unwrap_or(0);
        let pen = match self.templates.get(&d_code) {
            Some(Template::Built { pen, .. }) => *pen,
            // Undefined and unknown apertures are counted and reported.
            Some(Template::UnknownMacro(_) | Template::Unbuilt) | None => return Vec::new(),
            Some(Template::Block(_)) => Pen::None,
        };
        match (pen, points.last()) {
            (Pen::Round(radius), _) => capsule_chain(from, points, radius),
            (Pen::Rect { half_w, half_h }, Some(&to)) if straight => {
                rect_sweep(from, to, half_w, half_h).into_iter().collect()
            }
            _ => {
                self.report_once(
                    &format!("draw D{d_code}"),
                    line,
                    ProblemKind::UnsupportedCommand {
                        command: format!("D01 with aperture D{d_code}"),
                    },
                );
                Vec::new()
            }
        }
    }

    fn flash(&mut self, at: PointNm, line: u32) {
        self.point = at;
        if self.region.is_some() {
            self.unsupported(line, "D03 in a region");
            return;
        }
        let d_code = self.aperture.unwrap_or(0);
        self.count_use(d_code, line);
        let shapes = match self.templates.get(&d_code) {
            Some(Template::Built { flash, .. }) => {
                flash.iter().map(|s| s.clone().translated(at)).collect()
            }
            Some(Template::Block(objects)) => {
                // Blocks flashed inside blocks multiply; a file cannot make
                // more objects than a drawing can hold.
                let room = MAX_OBJECTS.saturating_sub(self.image.objects.len());
                if objects.len() > room {
                    self.report_once(
                        "objects",
                        line,
                        ProblemKind::UnsupportedCommand {
                            command: format!("D03 with block D{d_code} past {MAX_OBJECTS} objects"),
                        },
                    );
                    return;
                }
                let copies: Vec<_> = objects.iter().map(|o| o.translated(at)).collect();
                self.flash_block(copies, line);
                return;
            }
            Some(Template::UnknownMacro(_) | Template::Unbuilt) | None => Vec::new(),
        };
        self.push(ObjectKind::Flash { at }, self.aperture, shapes, line);
    }

    /// A block aperture's objects placed at the flash, at the flash's line;
    /// a clear flash inverts each one's polarity.
    fn flash_block(&mut self, copies: Vec<crate::image::Object>, line: u32) {
        let invert = self.polarity == crate::image::Polarity::Clear;
        for mut object in copies {
            if invert {
                object.polarity = object.polarity.inverted();
            }
            object.at = self.source.at(line);
            self.image.objects.push(object);
        }
    }

    fn start_region(&mut self, line: u32) {
        if self.region.is_none() {
            self.region = Some(Region {
                contour: Vec::new(),
                line,
            });
        }
    }

    pub(super) fn end_region(&mut self, line: u32) {
        let Some(region) = self.region.take() else {
            self.unsupported(line, "G37 without G36");
            return;
        };
        self.contour_done(region.contour, region.line, line);
    }

    /// One contour as a region object; one not back at its start is
    /// reported and closed.
    fn contour_done(&mut self, contour: Vec<PointNm>, started: u32, line: u32) {
        if contour.len() < 2 {
            return;
        }
        if contour.first() != contour.last() {
            self.report(line, ProblemKind::UnclosedRegion);
        }
        let shapes: Vec<Shape> = polygon_shape(&contour).into_iter().collect();
        let mut outline = contour;
        if outline.len() > 1 && outline.first() == outline.last() {
            outline.pop();
        }
        self.push(ObjectKind::Region { outline }, None, shapes, started);
    }
}

/// A rectangle pen swept along a line: the hull of the rectangle at both
/// ends.
fn rect_sweep(from: PointNm, to: PointNm, half_w: i64, half_h: i64) -> Option<Shape> {
    let corners = |c: PointNm| {
        [(-1, -1), (1, -1), (1, 1), (-1, 1)]
            .map(|(sx, sy)| PointNm::new(c.x + sx * half_w, c.y + sy * half_h))
    };
    let mut points: Vec<PointNm> = corners(from).into_iter().chain(corners(to)).collect();
    points.sort_unstable();
    points.dedup();
    polygon_shape(&convex_hull(points))
}

/// The convex hull of sorted, distinct points, counter-clockwise.
fn convex_hull(points: Vec<PointNm>) -> Vec<PointNm> {
    if points.len() < 3 {
        return points;
    }
    let turn = |o: PointNm, a: PointNm, b: PointNm| a.minus(o).cross(b.minus(o));
    let mut hull: Vec<PointNm> = Vec::with_capacity(points.len() * 2);
    for pass in [points.clone(), points.into_iter().rev().collect()] {
        let floor = hull.len();
        for p in pass {
            while hull.len() >= floor + 2
                && turn(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_split_into_their_letters() {
        let word = split_word("G01X-100Y+200I5J-6D01").expect("a word");
        assert_eq!(word.g, Some(1));
        assert_eq!(word.x, Some("-100"));
        assert_eq!(word.y, Some("+200"));
        assert_eq!(word.i, Some("5"));
        assert_eq!(word.j, Some("-6"));
        assert_eq!(word.d, Some(1));
        assert_eq!(split_word("D10").map(|w| w.d), Some(Some(10)));
        assert_eq!(split_word("X1X2D01"), None);
        assert_eq!(split_word("Q5"), None);
        assert_eq!(split_word("XD01"), None);
        assert_eq!(split_word("G-1"), None);
    }

    #[test]
    fn comments_are_g04_or_g4() {
        assert!(is_comment("G04 hello"));
        assert!(is_comment("G4 hello"));
        assert!(is_comment("G04"));
        assert!(!is_comment("G01"));
        assert!(!is_comment("G40"));
    }

    #[test]
    fn a_rectangle_swept_along_a_line_is_its_hull() {
        let shape = rect_sweep(PointNm::new(0, 0), PointNm::new(10, 0), 1, 2).expect("a hull");
        let Shape::Polygon { ring, .. } = shape else {
            panic!("a polygon");
        };
        assert_eq!(
            &*ring,
            &[
                PointNm::new(-1, -2),
                PointNm::new(11, -2),
                PointNm::new(11, 2),
                PointNm::new(-1, 2)
            ]
        );
    }
}
