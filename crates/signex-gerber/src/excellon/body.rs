//! The drill body: tool selections, holes and `G85` slots, with modal
//! coordinates written with a decimal point or in the header's format.

use pcb_geom::{PointNm, Shape};

use super::Reader;
use super::header::split_tool;
use crate::image::{Object, ObjectAttributes, ObjectKind, Polarity, Units};
use crate::number::{coordinate_to_nm, decimal_to_nm};
use crate::problem::ProblemKind;

/// The value after `letter`, up to the next letter.
fn letter_value(text: &str, letter: char) -> Option<&str> {
    let start = text.find(letter)? + 1;
    let tail = &text[start..];
    let end = tail
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(tail.len());
    Some(&tail[..end])
}

impl Reader<'_> {
    pub(super) fn body_line(&mut self, content: &str, line: u32) {
        match content {
            "M30" | "M00" => self.ended = true,
            // Absolute mode, drill mode, a repeated header end.
            "G90" | "G05" | "%" | "M95" => {}
            "M71" => self.image.units = Some(Units::Millimetres),
            "M72" => self.image.units = Some(Units::Inches),
            _ if content.starts_with('T') => self.select_tool(content, line),
            _ if content.starts_with('X') || content.starts_with('Y') => {
                self.hole_line(content, line);
            }
            _ => self.unsupported(line, content),
        }
    }

    /// `T1` selects a tool, `T0` none; `T3C0.8` defines it first.
    fn select_tool(&mut self, content: &str, line: u32) {
        let Some((tool, size)) = split_tool(content) else {
            self.unsupported(line, content);
            return;
        };
        if size.is_some() {
            self.tool_definition(content, line);
        }
        if tool == 0 {
            self.tool = None;
            return;
        }
        self.tool = Some(tool);
        if !self.tools.contains_key(&tool) {
            self.undefined.entry(tool).or_insert(line);
        }
    }

    /// One coordinate in nm, reported when it cannot be read.
    fn coordinate(&mut self, text: &str, line: u32) -> Option<i64> {
        let units = self.units(line);
        let nm = if text.contains('.') {
            decimal_to_nm(text, units)
        } else {
            let format = self.implied_format(units, line);
            coordinate_to_nm(text, format, units)
        };
        if nm.is_none() {
            self.report(line, ProblemKind::BadCoordinate);
        }
        nm
    }

    /// `X..Y..` from `base`, an absent axis keeping its value.
    fn position(&mut self, text: &str, base: PointNm, line: u32) -> Option<PointNm> {
        let plain = text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'X' | 'Y' | '.' | '+' | '-'));
        let (x, y) = (letter_value(text, 'X'), letter_value(text, 'Y'));
        if !plain || (x.is_none() && y.is_none()) {
            self.report(line, ProblemKind::BadCoordinate);
            return None;
        }
        let x = match x {
            Some(x) => self.coordinate(x, line)?,
            None => base.x,
        };
        let y = match y {
            Some(y) => self.coordinate(y, line)?,
            None => base.y,
        };
        Some(PointNm::new(x, y))
    }

    /// A hole, or a slot `X..Y..G85X..Y..`, with the selected tool.
    fn hole_line(&mut self, content: &str, line: u32) {
        let (start, end) = match content.split_once("G85") {
            Some((start, end)) => (start, Some(end)),
            None => (content, None),
        };
        let Some(at) = self.position(start, self.point, line) else {
            return;
        };
        let slot_to = match end {
            Some(end) => match self.position(end, at, line) {
                Some(to) => Some(to),
                None => return,
            },
            None => None,
        };
        self.point = slot_to.unwrap_or(at);
        let tool = self.tool.unwrap_or(0);
        *self.holes.entry(tool).or_default() += 1;
        let diameter = match self.tools.get(&tool) {
            Some((diameter, _)) => *diameter,
            None => {
                self.undefined.entry(tool).or_insert(line);
                None
            }
        };
        let shapes = match (diameter, slot_to) {
            (Some(d), None) => vec![Shape::Disc {
                center: at,
                radius: d / 2,
            }],
            (Some(d), Some(to)) => vec![Shape::Capsule {
                a: at,
                b: to,
                radius: d / 2,
            }],
            (None, _) => Vec::new(),
        };
        self.image.objects.push(Object {
            kind: ObjectKind::Hole {
                at,
                diameter,
                tool,
                slot_to,
            },
            polarity: Polarity::Dark,
            aperture: self.tool,
            attributes: ObjectAttributes::default(),
            at: self.source.at(line),
            shapes,
        });
    }
}
