//! Gerber RS-274X images with X2 attributes. By default millimetres, six
//! decimals (`%FSLAX46Y46*%`), so a coordinate is the integer nm itself; an
//! Export Job's Gerber settings (#1299) choose the units, the format, the
//! zeros, the layers and the aperture form. See
//! `docs/FABRICATION_OUTPUTS.md`, "Gerber".

use pcb_geom::PointNm;
use signex_types::output_job::{
    GerberApertures, GerberFormat, GerberSettings, GerberUnits, GerberZeros,
};

use super::frame::{div_round, millionths};
use super::layers::{CopperStack, FabLayer, file_function, file_polarity};
use super::marks::{ApertureShape, Attributes, Function, Mark, Step};

/// The first aperture number a file may define.
const FIRST_APERTURE: usize = 10;

/// The X2 `.GenerationSoftware` value every file carries. No version, so a
/// release bump does not change the bytes of an unchanged board.
pub(crate) const GENERATION_SOFTWARE: &str = "Alp Lab,Signex";

/// One image as a Gerber file, written with `settings`.
pub(crate) fn write_image(
    layer: FabLayer,
    marks: &[Mark],
    stack: &CopperStack,
    settings: &GerberSettings,
) -> String {
    let numbers = Numbers::of(settings);
    let apertures = Apertures::of(marks, settings.apertures);
    let mut out = String::new();
    header(&mut out, layer, stack, numbers);
    apertures.define(&mut out, numbers);
    out.push_str("G75*\n");
    let mut writer = Body {
        out,
        aperture: None,
        mode: None,
        with_attributes: layer.is_copper(),
        numbers,
    };
    for mark in marks {
        writer.mark(mark, &apertures);
    }
    let mut out = writer.out;
    out.push_str("M02*\n");
    out
}

fn header(out: &mut String, layer: FabLayer, stack: &CopperStack, numbers: Numbers) {
    let polarity = file_polarity(layer);
    out.push_str(&format!("%TF.GenerationSoftware,{GENERATION_SOFTWARE}*%\n"));
    out.push_str(&format!(
        "%TF.FileFunction,{}*%\n",
        file_function(layer, stack)
    ));
    out.push_str(&format!("%TF.FilePolarity,{polarity}*%\n"));
    let zeros = match numbers.zeros {
        GerberZeros::Trailing => 'T',
        GerberZeros::Leading | GerberZeros::None => 'L',
    };
    let digits = format!("{}{}", numbers.format.integer, numbers.format.decimal);
    let units = match numbers.units {
        GerberUnits::Mm => "MM",
        GerberUnits::Inch => "IN",
    };
    out.push_str(&format!("%FS{zeros}AX{digits}Y{digits}*%\n%MO{units}*%\n"));
}

/// How one file writes its numbers: a coordinate in the format's digits
/// with its zeros left out, a length in the file's unit.
#[derive(Debug, Clone, Copy, Default)]
struct Numbers {
    units: GerberUnits,
    format: GerberFormat,
    zeros: GerberZeros,
}

impl Numbers {
    fn of(settings: &GerberSettings) -> Self {
        Self {
            units: settings.units,
            format: settings.format,
            zeros: settings.zeros,
        }
    }

    /// `nm` counted in the format's last decimal digit, rounded half away
    /// from zero. Millimetres to six decimals is the nm value itself.
    fn scaled(self, nm: i64) -> i64 {
        let per_unit: i128 = match self.units {
            GerberUnits::Mm => 1_000_000,
            GerberUnits::Inch => 25_400_000,
        };
        let value = i128::from(nm) * 10i128.pow(u32::from(self.format.decimal));
        let quotient = value / per_unit;
        let rest = value % per_unit;
        let rounded = if 2 * rest.abs() >= per_unit {
            quotient + value.signum()
        } else {
            quotient
        };
        i64::try_from(rounded).unwrap_or(i64::MAX)
    }

    /// A coordinate (`X`, `Y`, `I`, `J`).
    fn coord(self, nm: i64) -> String {
        let value = self.scaled(nm);
        if self.zeros == GerberZeros::Leading {
            return value.to_string();
        }
        let sign = if value < 0 { "-" } else { "" };
        let width = usize::from(self.format.integer + self.format.decimal);
        let digits = format!("{:0width$}", value.unsigned_abs());
        match self.zeros {
            GerberZeros::Trailing => {
                let kept = digits.trim_end_matches('0');
                let kept = if kept.is_empty() { "0" } else { kept };
                format!("{sign}{kept}")
            }
            GerberZeros::Leading | GerberZeros::None => format!("{sign}{digits}"),
        }
    }

    /// An aperture or macro length as a decimal in the file's unit.
    fn length(self, nm: i64) -> String {
        match self.units {
            GerberUnits::Mm => millionths(nm),
            // Millionths of an inch: 25.4 nm each.
            GerberUnits::Inch => millionths(div_round(nm.saturating_mul(10), 254)),
        }
    }
}

/// The apertures a file uses, numbered from [`FIRST_APERTURE`] in the
/// order the marks first use them.
struct Apertures {
    list: Vec<(ApertureShape, Function)>,
    form: GerberApertures,
}

impl Apertures {
    fn of(marks: &[Mark], form: GerberApertures) -> Self {
        let mut list = Vec::new();
        for mark in marks {
            if let Some(key) = aperture_of(mark)
                && !list.contains(&key)
            {
                list.push(key);
            }
        }
        Self { list, form }
    }

    fn code(&self, key: (ApertureShape, Function)) -> usize {
        let index = self.list.iter().position(|k| *k == key).unwrap_or(0);
        FIRST_APERTURE + index
    }

    fn define(&self, out: &mut String, numbers: Numbers) {
        for (index, (shape, function)) in self.list.iter().enumerate() {
            let code = FIRST_APERTURE + index;
            let body = match self.form {
                GerberApertures::Standard => macro_body(shape, numbers),
                GerberApertures::Macros => Some(any_macro_body(shape, numbers)),
            };
            if let Some(body) = &body {
                out.push_str(&format!("%AMM{code}*\n{body}%\n"));
            }
            out.push_str(&format!("%TA.AperFunction,{}*%\n", function.x2()));
            let template = match body {
                Some(_) => format!("M{code}"),
                None => standard_template(shape, numbers),
            };
            out.push_str(&format!("%ADD{code}{template}*%\n"));
        }
        if !self.list.is_empty() {
            out.push_str("%TD*%\n");
        }
    }
}

fn aperture_of(mark: &Mark) -> Option<(ApertureShape, Function)> {
    match mark {
        Mark::Flash {
            shape, function, ..
        } => Some((shape.clone(), *function)),
        Mark::Stroke {
            width, function, ..
        } => Some((ApertureShape::Circle { diameter: *width }, *function)),
        Mark::Region { .. } => None,
    }
}

/// The template of a standard aperture; only called for a shape
/// [`macro_body`] gives no body.
fn standard_template(shape: &ApertureShape, numbers: Numbers) -> String {
    let mm = |value| numbers.length(value);
    match *shape {
        ApertureShape::Circle { diameter } => format!("C,{}", mm(diameter)),
        ApertureShape::Rect { w, h } => format!("R,{}X{}", mm(w), mm(h)),
        ApertureShape::Obround { w, h } => format!("O,{}X{}", mm(w), mm(h)),
        ApertureShape::TurnedRect { .. }
        | ApertureShape::TurnedObround { .. }
        | ApertureShape::RoundRect { .. }
        | ApertureShape::Outline { .. } => String::new(),
    }
}

/// A macro body for any shape, the standard ones included: the circle
/// primitive for a circle, an unturned rectangle or stadium otherwise.
fn any_macro_body(shape: &ApertureShape, numbers: Numbers) -> String {
    let standard = match *shape {
        ApertureShape::Circle { diameter } => {
            let mm = |value| numbers.length(value);
            return format!("1,1,{},0,0,0*\n", mm(diameter));
        }
        ApertureShape::Rect { w, h } => ApertureShape::TurnedRect { w, h, udeg: 0 },
        ApertureShape::Obround { w, h } => ApertureShape::TurnedObround { w, h, udeg: 0 },
        _ => shape.clone(),
    };
    macro_body(&standard, numbers).unwrap_or_default()
}

/// The primitives of a macro aperture, one per line, each ending in `*`;
/// `None` for a standard aperture. Primitive 21 is a centred rectangle and
/// 1 a circle, each turned about the aperture's origin. Lengths are in the
/// file's unit, angles in degrees.
fn macro_body(shape: &ApertureShape, numbers: Numbers) -> Option<String> {
    let mm = |value| numbers.length(value);
    let rect =
        |w: i64, h: i64, a: i64| format!("21,1,{},{},0,0,{}*\n", mm(w), mm(h), millionths(a));
    let disc = |d: i64, x: i64, y: i64, a: i64| {
        format!("1,1,{},{},{},{}*\n", mm(d), mm(x), mm(y), millionths(a))
    };
    match *shape {
        ApertureShape::TurnedRect { w, h, udeg } => Some(rect(w, h, udeg)),
        ApertureShape::TurnedObround { w, h, udeg } => {
            let short = w.min(h);
            let reach = (w.max(h) - short) / 2;
            // The bar spans between the two end-cap centres.
            let ((dx, dy), (bar_w, bar_h)) = if w >= h {
                ((reach, 0), (2 * reach, short))
            } else {
                ((0, reach), (short, 2 * reach))
            };
            Some(format!(
                "{}{}{}",
                rect(bar_w, bar_h, udeg),
                disc(short, dx, dy, udeg),
                disc(short, -dx, -dy, udeg)
            ))
        }
        ApertureShape::RoundRect { w, h, r, udeg } => {
            let (x, y) = (w / 2 - r, h / 2 - r);
            Some(format!(
                "{}{}{}{}{}{}",
                rect(w, h - 2 * r, udeg),
                rect(w - 2 * r, h, udeg),
                disc(2 * r, x, y, udeg),
                disc(2 * r, -x, y, udeg),
                disc(2 * r, -x, -y, udeg),
                disc(2 * r, x, -y, udeg)
            ))
        }
        // Primitive 4, an outline: the corner count, then every corner and
        // the first again to close it.
        ApertureShape::Outline { ref corners } => {
            let first = corners.first()?;
            let points: Vec<String> = corners
                .iter()
                .chain(std::iter::once(first))
                .map(|p| format!("{},{}", mm(p.x), mm(p.y)))
                .collect();
            Some(format!("4,1,{},{},0*\n", corners.len(), points.join(",")))
        }
        ApertureShape::Circle { .. }
        | ApertureShape::Rect { .. }
        | ApertureShape::Obround { .. } => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Linear,
    Clockwise,
    CounterClockwise,
}

struct Body {
    out: String,
    aperture: Option<usize>,
    mode: Option<Mode>,
    with_attributes: bool,
    numbers: Numbers,
}

impl Body {
    fn mark(&mut self, mark: &Mark, apertures: &Apertures) {
        let attributes = match mark {
            Mark::Flash { attributes, .. }
            | Mark::Stroke { attributes, .. }
            | Mark::Region { attributes, .. } => attributes,
        };
        let attributed = self.with_attributes && !attributes.is_empty();
        if attributed {
            self.attributes(attributes);
        }
        if let Some(key) = aperture_of(mark) {
            self.select(apertures.code(key));
        }
        match mark {
            Mark::Flash { at, .. } => {
                let at = self.xy(*at);
                self.out.push_str(&format!("{at}D03*\n"));
            }
            Mark::Stroke { start, steps, .. } => self.stroke(*start, steps),
            Mark::Region { outline, holes, .. } => {
                self.contour(outline);
                if !holes.is_empty() {
                    self.out.push_str("%LPC*%\n");
                    holes.iter().for_each(|hole| self.contour(hole));
                    self.out.push_str("%LPD*%\n");
                }
            }
        }
        if attributed {
            self.out.push_str("%TD*%\n");
        }
    }

    fn attributes(&mut self, attributes: &Attributes) {
        if let Some(net) = &attributes.net {
            self.out.push_str(&format!("%TO.N,{}*%\n", escape(net)));
        }
        if let Some((designator, pin)) = &attributes.pin {
            let designator = escape(designator);
            self.out
                .push_str(&format!("%TO.P,{designator},{}*%\n", escape(pin)));
            self.out.push_str(&format!("%TO.C,{designator}*%\n"));
        }
    }

    fn select(&mut self, code: usize) {
        if self.aperture != Some(code) {
            self.out.push_str(&format!("D{code}*\n"));
            self.aperture = Some(code);
        }
    }

    fn set_mode(&mut self, mode: Mode) {
        if self.mode != Some(mode) {
            self.out.push_str(match mode {
                Mode::Linear => "G01*\n",
                Mode::Clockwise => "G02*\n",
                Mode::CounterClockwise => "G03*\n",
            });
            self.mode = Some(mode);
        }
    }

    /// `X<x>Y<y>` in the file's numbers.
    fn xy(&self, p: PointNm) -> String {
        let n = self.numbers;
        format!("X{}Y{}", n.coord(p.x), n.coord(p.y))
    }

    fn stroke(&mut self, start: PointNm, steps: &[Step]) {
        let from = self.xy(start);
        self.out.push_str(&format!("{from}D02*\n"));
        let mut at = start;
        for step in steps {
            match *step {
                Step::Line(to) => {
                    self.set_mode(Mode::Linear);
                    let to_xy = self.xy(to);
                    self.out.push_str(&format!("{to_xy}D01*\n"));
                    at = to;
                }
                Step::Arc { to, center, ccw } => {
                    self.set_mode(if ccw {
                        Mode::CounterClockwise
                    } else {
                        Mode::Clockwise
                    });
                    let n = self.numbers;
                    let (i, j) = (n.coord(center.x - at.x), n.coord(center.y - at.y));
                    let to_xy = self.xy(to);
                    self.out.push_str(&format!("{to_xy}I{i}J{j}D01*\n"));
                    at = to;
                }
            }
        }
    }

    fn contour(&mut self, ring: &[PointNm]) {
        let Some((first, rest)) = ring.split_first() else {
            return;
        };
        self.out.push_str("G36*\n");
        self.set_mode(Mode::Linear);
        let from = self.xy(*first);
        self.out.push_str(&format!("{from}D02*\n"));
        for p in rest.iter().chain(std::iter::once(first)) {
            let at = self.xy(*p);
            self.out.push_str(&format!("{at}D01*\n"));
        }
        self.out.push_str("G37*\n");
    }
}

/// An attribute value with the characters a Gerber field cannot hold
/// (`,` `*` `%` `\`, control and non-ASCII characters) written as
/// `\uXXXX` UTF-16 escapes.
pub(crate) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        let plain = c.is_ascii() && !c.is_ascii_control() && !matches!(c, ',' | '*' | '%' | '\\');
        if plain {
            out.push(c);
        } else {
            let mut units = [0u16; 2];
            for unit in c.encode_utf16(&mut units) {
                out.push_str(&format!("\\u{unit:04X}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_values_escape_the_reserved_characters() {
        assert_eq!(escape("GND"), "GND");
        assert_eq!(escape("A,B*C%D\\"), "A\\u002CB\\u002AC\\u0025D\\u005C");
        assert_eq!(escape("Ω"), "\\u03A9");
    }

    #[test]
    fn a_turned_obround_is_a_rectangle_between_two_discs() {
        let body = macro_body(
            &ApertureShape::TurnedObround {
                w: 2_000_000,
                h: 1_000_000,
                udeg: 45_000_000,
            },
            Numbers::default(),
        )
        .unwrap();
        assert_eq!(
            body,
            "21,1,1,1,0,0,45*\n1,1,1,0.5,0,45*\n1,1,1,-0.5,0,45*\n"
        );
    }

    #[test]
    fn a_round_rect_is_two_bars_and_four_corner_discs() {
        let body = macro_body(
            &ApertureShape::RoundRect {
                w: 2_000_000,
                h: 1_000_000,
                r: 250_000,
                udeg: 0,
            },
            Numbers::default(),
        )
        .unwrap();
        assert_eq!(
            body,
            "21,1,2,0.5,0,0,0*\n21,1,1.5,1,0,0,0*\n\
             1,1,0.5,0.75,0.25,0*\n1,1,0.5,-0.75,0.25,0*\n\
             1,1,0.5,-0.75,-0.25,0*\n1,1,0.5,0.75,-0.25,0*\n"
        );
        assert_eq!(
            macro_body(&ApertureShape::Circle { diameter: 1 }, Numbers::default()),
            None
        );
    }

    #[test]
    fn a_polygon_pad_is_one_closed_outline() {
        let corners = [
            (-750_000, -500_000),
            (750_000, -500_000),
            (1_250_000, 500_000),
        ]
        .map(|(x, y)| PointNm::new(x, y))
        .to_vec();
        let body = macro_body(&ApertureShape::Outline { corners }, Numbers::default()).unwrap();
        assert_eq!(body, "4,1,3,-0.75,-0.5,0.75,-0.5,1.25,0.5,-0.75,-0.5,0*\n");
    }
}
