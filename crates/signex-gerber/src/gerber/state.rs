//! The graphics state of a Gerber read and how a read ends: the problems
//! counted while reading (uses of unknown macros and undefined apertures)
//! are written once, with their counts, when the file is done.

use std::collections::{BTreeMap, BTreeSet};

use pcb_geom::{PointNm, Shape};

use super::macros::Macro;
use crate::image::{Image, Object, ObjectAttributes, ObjectKind, Polarity, Units};
use crate::number::{CoordFormat, Omitted};
use crate::problem::ProblemKind;
use crate::source::{FileKind, Source};

/// The format assumed when a file gives none (reported): 3.6, leading
/// zeros left out.
const FALLBACK_FORMAT: CoordFormat = CoordFormat {
    integer: 3,
    decimal: 6,
    omitted: Omitted::Leading,
};

/// The most objects one image holds; block apertures flashed inside block
/// apertures could otherwise multiply without end.
pub(super) const MAX_OBJECTS: usize = 10_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Interpolation {
    Linear,
    Clockwise,
    CounterClockwise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Quadrant {
    Single,
    Multi,
}

/// The pen a draw (`D01`) is made with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pen {
    Round(i64),
    Rect {
        half_w: i64,
        half_h: i64,
    },
    /// An aperture a draw may not use.
    None,
}

/// What a D code draws.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Template {
    /// Its flash shapes about the flash point, and its pen.
    Built { flash: Vec<Shape>, pen: Pen },
    /// It names a macro the file never defines.
    UnknownMacro(String),
    /// Its definition cannot be built (reported where it is defined).
    Unbuilt,
    /// A block aperture (`%AB`): its objects about the flash point.
    Block(Vec<Object>),
}

/// A region being read (`G36` .. `G37`).
#[derive(Debug, Default)]
pub(super) struct Region {
    pub contour: Vec<PointNm>,
    /// The line the current contour started on.
    pub line: u32,
}

/// An open step and repeat (`%SR`).
#[derive(Debug, Clone, Copy)]
pub(super) struct StepRepeat {
    pub first_object: usize,
    pub nx: u32,
    pub ny: u32,
    pub step: PointNm,
    pub line: u32,
}

pub(super) struct Reader<'a> {
    pub source: Source<'a>,
    pub image: Image,
    pub format: Option<CoordFormat>,
    pub templates: BTreeMap<u32, Template>,
    pub macros: BTreeMap<String, Macro>,
    pub point: PointNm,
    pub aperture: Option<u32>,
    pub interpolation: Interpolation,
    pub quadrant: Quadrant,
    /// The last operation code, for a coordinate word without one.
    pub operation: Option<u32>,
    pub polarity: Polarity,
    pub region: Option<Region>,
    /// The X2 `.AperFunction` in force for the next `%AD`.
    pub aperture_function: Option<String>,
    pub attributes: ObjectAttributes,
    /// Objects drawn per D code.
    pub uses: BTreeMap<u32, usize>,
    /// Undefined D codes (0 for "none selected"): first line, objects.
    pub undefined: BTreeMap<u32, (u32, usize)>,
    /// Problems reported once per file, by key.
    pub reported: BTreeSet<String>,
    pub step: Option<StepRepeat>,
    /// Open block apertures: D code, definition line, first object.
    pub blocks: Vec<(u32, u32, usize)>,
    pub ended: bool,
}

impl<'a> Reader<'a> {
    pub(super) fn new(file: &'a str, text: &'a str) -> Self {
        Reader {
            source: Source::new(file, text),
            image: Image::empty(file, FileKind::Gerber),
            format: None,
            templates: BTreeMap::new(),
            macros: BTreeMap::new(),
            point: PointNm::ORIGIN,
            aperture: None,
            interpolation: Interpolation::Linear,
            // A file that never says is read as G75, as every current
            // writer means it (single-quadrant G74 is deprecated).
            quadrant: Quadrant::Multi,
            operation: None,
            polarity: Polarity::Dark,
            region: None,
            aperture_function: None,
            attributes: ObjectAttributes::default(),
            uses: BTreeMap::new(),
            undefined: BTreeMap::new(),
            reported: BTreeSet::new(),
            step: None,
            blocks: Vec::new(),
            ended: false,
        }
    }

    pub(super) fn report(&mut self, line: u32, kind: ProblemKind) {
        let problem = self.source.problem(line, kind);
        self.image.problems.push(problem);
    }

    /// Report `kind` the first time `key` is met.
    pub(super) fn report_once(&mut self, key: &str, line: u32, kind: ProblemKind) {
        if self.reported.insert(key.to_string()) {
            self.report(line, kind);
        }
    }

    pub(super) fn unsupported(&mut self, line: u32, command: &str) {
        self.report(
            line,
            ProblemKind::UnsupportedCommand {
                command: command.to_string(),
            },
        );
    }

    /// The units in force; millimetres, reported once, when none are set.
    pub(super) fn units(&mut self, line: u32) -> Units {
        if let Some(units) = self.image.units {
            return units;
        }
        self.report_once("units", line, ProblemKind::MissingUnits);
        Units::Millimetres
    }

    /// The coordinate format; [`FALLBACK_FORMAT`], reported once, when none
    /// is set.
    pub(super) fn format(&mut self, line: u32) -> CoordFormat {
        if let Some(format) = self.format {
            return format;
        }
        self.report_once("format", line, ProblemKind::MissingFormat);
        FALLBACK_FORMAT
    }

    /// Add an object drawn with the selected aperture (`None` for a region).
    pub(super) fn push(
        &mut self,
        kind: ObjectKind,
        aperture: Option<u32>,
        shapes: Vec<Shape>,
        line: u32,
    ) {
        let object = Object {
            kind,
            polarity: self.polarity,
            aperture,
            attributes: self.attributes.clone(),
            at: self.source.at(line),
            shapes,
        };
        self.image.objects.push(object);
    }

    /// Count one object drawn with D code `d_code` (0: none selected).
    pub(super) fn count_use(&mut self, d_code: u32, line: u32) {
        *self.uses.entry(d_code).or_default() += 1;
        if !self.templates.contains_key(&d_code) {
            self.undefined.entry(d_code).or_insert((line, 0)).1 += 1;
        }
    }

    /// The problems known only at the end, then every problem by line.
    pub(super) fn finish(mut self) -> Image {
        let last = self.source.last_line();
        // A region the file never ends; a contour not back at its start is
        // reported as it is closed, so a closed one is reported here.
        let open = self.region.as_ref().map(|r| {
            r.contour.len() < 2 || r.contour.first() == r.contour.last()
        });
        if let Some(closed) = open {
            if closed {
                self.report(last, ProblemKind::UnclosedRegion);
            }
            self.end_region(last);
        }
        if self.step.is_some() {
            self.close_step();
        }
        for (d_code, line, _) in std::mem::take(&mut self.blocks) {
            self.report(line, ProblemKind::BadApertureDefinition { d_code });
        }
        if !self.ended {
            self.report_once("end", last, ProblemKind::MissingEndOfFile);
        }
        self.report_counted();
        if self.image.function.is_none() {
            self.report(1, ProblemKind::NoFileFunction);
        }
        self.image.problems.sort_by_key(|p| p.at.line);
        self.image
    }

    fn report_counted(&mut self) {
        let unknown: Vec<(u32, String)> = self
            .templates
            .iter()
            .filter_map(|(d, t)| match t {
                Template::UnknownMacro(name) => Some((*d, name.clone())),
                _ => None,
            })
            .collect();
        for (d_code, name) in unknown {
            let line = self.image.apertures.get(&d_code).map_or(1, |a| a.at.line);
            let flashes = self.uses.get(&d_code).copied().unwrap_or(0);
            self.report(line, ProblemKind::UnknownApertureMacro { name, flashes });
        }
        for (d_code, (line, uses)) in std::mem::take(&mut self.undefined) {
            self.report(line, ProblemKind::UndefinedAperture { d_code, uses });
        }
    }
}
