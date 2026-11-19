//! Excellon drill files, the common subset: an `M48` header with units,
//! zero mode, number format and tools, closed by `%` or `M95`; a body of
//! tool selections, holes and `G85` slots, ended by `M30`. X2 attributes
//! ride in comments (`; #@! TF.FileFunction,Plated,1,2,PTH`), as Signex
//! writes them. Routing (`G00`-`G03`, `M15`-`M17`) and incremental mode are
//! reported as unsupported. A tool with no size keeps its holes, with no
//! diameter and no shape, for the viewer to mark.

mod body;
mod header;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use pcb_geom::PointNm;

use crate::image::{Image, Units};
use crate::number::{CoordFormat, Omitted};
use crate::problem::ProblemKind;
use crate::source::{FileKind, Source};

/// The X2 attribute mark in a drill comment.
const X2_MARK: &str = "#@!";

/// Where a reader is in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    /// Before `M48`.
    Start,
    Header,
    Body,
}

struct Reader<'a> {
    source: Source<'a>,
    image: Image,
    part: Part,
    /// Which zeros an implied-decimal coordinate leaves out, when given.
    omitted: Option<Omitted>,
    /// Integer and decimal digits, when the header gives them.
    digits: Option<(u32, u32)>,
    /// Each tool's diameter (`None`: no size) and definition line.
    tools: BTreeMap<u32, (Option<i64>, u32)>,
    tool: Option<u32>,
    point: PointNm,
    /// The X2 `.AperFunction` for the next tool.
    function: Option<String>,
    /// Holes per tool, and per undefined tool its first selection line.
    holes: BTreeMap<u32, usize>,
    undefined: BTreeMap<u32, u32>,
    reported: BTreeSet<&'static str>,
    ended: bool,
}

/// Read the Excellon file `file` from its text.
pub fn read_excellon(file: &str, text: &str) -> Image {
    let mut reader = Reader {
        source: Source::new(file, text),
        image: Image::empty(file, FileKind::Excellon),
        part: Part::Start,
        omitted: None,
        digits: None,
        tools: BTreeMap::new(),
        tool: None,
        point: PointNm::ORIGIN,
        function: None,
        holes: BTreeMap::new(),
        undefined: BTreeMap::new(),
        reported: BTreeSet::new(),
        ended: false,
    };
    for (index, raw) in text.lines().enumerate() {
        let line = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let content = raw.trim();
        if content.is_empty() {
            continue;
        }
        if let Some(comment) = content.strip_prefix(';') {
            reader.comment(comment, line);
            continue;
        }
        match reader.part {
            Part::Start if content == "M48" => reader.part = Part::Header,
            Part::Start => {
                reader.report_once(
                    "no header",
                    line,
                    ProblemKind::BadExcellonHeader {
                        text: content.to_string(),
                    },
                );
                reader.part = Part::Body;
                reader.body_line(content, line);
            }
            Part::Header => reader.header_line(content, line),
            Part::Body => reader.body_line(content, line),
        }
    }
    reader.finish()
}

impl Reader<'_> {
    fn report(&mut self, line: u32, kind: ProblemKind) {
        let problem = self.source.problem(line, kind);
        self.image.problems.push(problem);
    }

    fn report_once(&mut self, key: &'static str, line: u32, kind: ProblemKind) {
        if self.reported.insert(key) {
            self.report(line, kind);
        }
    }

    fn unsupported(&mut self, line: u32, command: &str) {
        self.report(
            line,
            ProblemKind::UnsupportedCommand {
                command: command.to_string(),
            },
        );
    }

    /// The units in force; inches, the format's old default, reported once
    /// when the header names none.
    fn units(&mut self, line: u32) -> Units {
        if let Some(units) = self.image.units {
            return units;
        }
        self.report_once("units", line, ProblemKind::MissingUnits);
        Units::Inches
    }

    /// The format of a coordinate without a decimal point: the header's
    /// digits or the usual 3.3 metric and 2.4 inch. When the header gives
    /// no zero mode the reading is ambiguous: reported once, and read with
    /// leading zeros left out.
    fn implied_format(&mut self, units: Units, line: u32) -> CoordFormat {
        let (integer, decimal) = self.digits.unwrap_or(match units {
            Units::Millimetres => (3, 3),
            Units::Inches => (2, 4),
        });
        let omitted = match self.omitted {
            Some(omitted) => omitted,
            None => {
                self.report_once("format", line, ProblemKind::MissingFormat);
                Omitted::Leading
            }
        };
        CoordFormat {
            integer,
            decimal,
            omitted,
        }
    }

    /// A comment: X2 attributes after `#@!`, anything else is a note.
    fn comment(&mut self, comment: &str, line: u32) {
        let Some(attribute) = comment.trim().strip_prefix(X2_MARK) else {
            return;
        };
        let attribute = attribute.trim();
        let Some((kind, name, value)) = crate::gerber::split_attribute(attribute) else {
            return;
        };
        match kind {
            "TF" => {
                if !crate::gerber::file_attribute(&mut self.image, name, value) {
                    self.unsupported(line, attribute);
                }
            }
            "TA" if name == ".AperFunction" => self.function = Some(value.to_string()),
            _ => {}
        }
    }

    fn finish(mut self) -> Image {
        let last = self.source.last_line();
        if !self.ended {
            self.report(last, ProblemKind::MissingEndOfFile);
        }
        let without_size: Vec<(u32, u32)> = self
            .tools
            .iter()
            .filter(|(_, (size, _))| size.is_none())
            .map(|(tool, (_, line))| (*tool, *line))
            .collect();
        for (tool, line) in without_size {
            let holes = self.holes.get(&tool).copied().unwrap_or(0);
            self.report(line, ProblemKind::ToolWithoutSize { tool, holes });
        }
        for (tool, line) in std::mem::take(&mut self.undefined) {
            let holes = self.holes.get(&tool).copied().unwrap_or(0);
            self.report(line, ProblemKind::UndefinedTool { tool, holes });
        }
        if self.image.function.is_none() {
            self.report(1, ProblemKind::NoFileFunction);
        }
        self.image.problems.sort_by_key(|p| p.at.line);
        self.image
    }
}
