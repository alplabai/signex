//! Gerber RS-274X images with X2 attributes. Format: millimetres, six
//! decimals (`%FSLAX46Y46*%`), so a coordinate is the integer nm itself.
//! See `docs/FABRICATION_OUTPUTS.md`, "Gerber".

use pcb_geom::PointNm;

use super::frame::millionths;
use super::layers::{CopperStack, FabLayer, file_function, file_polarity};
use super::marks::{ApertureShape, Attributes, Function, Mark, Step};

/// The first aperture number a file may define.
const FIRST_APERTURE: usize = 10;

/// The X2 `.GenerationSoftware` value every file carries. No version, so a
/// release bump does not change the bytes of an unchanged board.
pub(crate) const GENERATION_SOFTWARE: &str = "Alp Lab,Signex";

/// One image as a Gerber file.
pub(crate) fn write_image(layer: FabLayer, marks: &[Mark], stack: &CopperStack) -> String {
    let apertures = Apertures::of(marks);
    let mut out = String::new();
    header(&mut out, layer, stack);
    apertures.define(&mut out);
    out.push_str("G75*\n");
    let mut writer = Body {
        out,
        aperture: None,
        mode: None,
        with_attributes: layer.is_copper(),
    };
    for mark in marks {
        writer.mark(mark, &apertures);
    }
    let mut out = writer.out;
    out.push_str("M02*\n");
    out
}

fn header(out: &mut String, layer: FabLayer, stack: &CopperStack) {
    let polarity = file_polarity(layer);
    out.push_str(&format!("%TF.GenerationSoftware,{GENERATION_SOFTWARE}*%\n"));
    out.push_str(&format!(
        "%TF.FileFunction,{}*%\n",
        file_function(layer, stack)
    ));
    out.push_str(&format!("%TF.FilePolarity,{polarity}*%\n"));
    out.push_str("%FSLAX46Y46*%\n%MOMM*%\n");
}

/// The apertures a file uses, numbered from [`FIRST_APERTURE`] in the
/// order the marks first use them.
struct Apertures {
    list: Vec<(ApertureShape, Function)>,
}

impl Apertures {
    fn of(marks: &[Mark]) -> Self {
        let mut list = Vec::new();
        for mark in marks {
            if let Some(key) = aperture_of(mark)
                && !list.contains(&key)
            {
                list.push(key);
            }
        }
        Self { list }
    }

    fn code(&self, key: (ApertureShape, Function)) -> usize {
        let index = self.list.iter().position(|k| *k == key).unwrap_or(0);
        FIRST_APERTURE + index
    }

    fn define(&self, out: &mut String) {
        for (index, (shape, function)) in self.list.iter().enumerate() {
            let code = FIRST_APERTURE + index;
            if let Some(body) = macro_body(shape) {
                out.push_str(&format!("%AMM{code}*\n{body}%\n"));
            }
            out.push_str(&format!("%TA.AperFunction,{}*%\n", function.x2()));
            out.push_str(&format!("%ADD{code}{}*%\n", template(shape, code)));
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

fn template(shape: &ApertureShape, code: usize) -> String {
    let mm = millionths;
    match *shape {
        ApertureShape::Circle { diameter } => format!("C,{}", mm(diameter)),
        ApertureShape::Rect { w, h } => format!("R,{}X{}", mm(w), mm(h)),
        ApertureShape::Obround { w, h } => format!("O,{}X{}", mm(w), mm(h)),
        ApertureShape::TurnedRect { .. }
        | ApertureShape::TurnedObround { .. }
        | ApertureShape::RoundRect { .. }
        | ApertureShape::Outline { .. } => format!("M{code}"),
    }
}

/// The primitives of a macro aperture, one per line, each ending in `*`;
/// `None` for a standard aperture. Primitive 21 is a centred rectangle and
/// 1 a circle, each turned about the aperture's origin.
fn macro_body(shape: &ApertureShape) -> Option<String> {
    let mm = millionths;
    let rect = |w: i64, h: i64, a: i64| format!("21,1,{},{},0,0,{}*\n", mm(w), mm(h), mm(a));
    let disc =
        |d: i64, x: i64, y: i64, a: i64| format!("1,1,{},{},{},{}*\n", mm(d), mm(x), mm(y), mm(a));
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
                self.out.push_str(&format!("X{}Y{}D03*\n", at.x, at.y));
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

    fn stroke(&mut self, start: PointNm, steps: &[Step]) {
        self.out
            .push_str(&format!("X{}Y{}D02*\n", start.x, start.y));
        let mut at = start;
        for step in steps {
            match *step {
                Step::Line(to) => {
                    self.set_mode(Mode::Linear);
                    self.out.push_str(&format!("X{}Y{}D01*\n", to.x, to.y));
                    at = to;
                }
                Step::Arc { to, center, ccw } => {
                    self.set_mode(if ccw {
                        Mode::CounterClockwise
                    } else {
                        Mode::Clockwise
                    });
                    let (i, j) = (center.x - at.x, center.y - at.y);
                    self.out
                        .push_str(&format!("X{}Y{}I{i}J{j}D01*\n", to.x, to.y));
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
        self.out
            .push_str(&format!("X{}Y{}D02*\n", first.x, first.y));
        for p in rest.iter().chain(std::iter::once(first)) {
            self.out.push_str(&format!("X{}Y{}D01*\n", p.x, p.y));
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
        let body = macro_body(&ApertureShape::TurnedObround {
            w: 2_000_000,
            h: 1_000_000,
            udeg: 45_000_000,
        })
        .unwrap();
        assert_eq!(
            body,
            "21,1,1,1,0,0,45*\n1,1,1,0.5,0,45*\n1,1,1,-0.5,0,45*\n"
        );
    }

    #[test]
    fn a_round_rect_is_two_bars_and_four_corner_discs() {
        let body = macro_body(&ApertureShape::RoundRect {
            w: 2_000_000,
            h: 1_000_000,
            r: 250_000,
            udeg: 0,
        })
        .unwrap();
        assert_eq!(
            body,
            "21,1,2,0.5,0,0,0*\n21,1,1.5,1,0,0,0*\n\
             1,1,0.5,0.75,0.25,0*\n1,1,0.5,-0.75,0.25,0*\n\
             1,1,0.5,-0.75,-0.25,0*\n1,1,0.5,0.75,-0.25,0*\n"
        );
        assert_eq!(macro_body(&ApertureShape::Circle { diameter: 1 }), None);
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
        let body = macro_body(&ApertureShape::Outline { corners }).unwrap();
        assert_eq!(body, "4,1,3,-0.75,-0.5,0.75,-0.5,1.25,0.5,-0.75,-0.5,0*\n");
    }
}
