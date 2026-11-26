//! Aperture definitions (`%AD`): the standard circle, rectangle, obround
//! and polygon, or a macro with its parameters, built once into shapes
//! about the flash point. A hole in a standard aperture is not cut out of
//! the flash (docs/GERBER_VIEWER.md, "What is drawn").

use pcb_geom::{PointNm, Shape};

use super::macros::regular_polygon;
use super::state::{Pen, Reader, Template};
use crate::image::{ApertureDef, Units, polygon_shape};
use crate::number::decimal_to_nm;
use crate::problem::ProblemKind;

/// The lowest D code an aperture may have; D01-D03 are operations.
const FIRST_APERTURE: u32 = 10;

/// `ADD10R,1X1.2` as (10, `R`, [`1`, `1.2`]).
fn split_definition(statement: &str) -> Option<(u32, &str, Vec<&str>)> {
    let rest = statement.strip_prefix("ADD")?;
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let d_code = rest[..digits].parse().ok()?;
    let (name, params) = rest[digits..]
        .split_once(',')
        .unwrap_or((&rest[digits..], ""));
    let params = if params.trim().is_empty() {
        Vec::new()
    } else {
        params.split('X').map(str::trim).collect()
    };
    Some((d_code, name.trim(), params))
}

/// A standard aperture's shapes and pen; `None` when its parameters are
/// not sizes it can have.
fn standard(name: &str, params: &[&str], units: Units) -> Option<Template> {
    let size = |i: usize| decimal_to_nm(params.get(i)?, units).filter(|v| *v >= 0);
    // At most one optional hole after the sizes; it must still read.
    let fits = |sizes: usize| {
        (sizes..=sizes + 1).contains(&params.len())
            && (params.len() == sizes || size(sizes).is_some())
    };
    let origin = PointNm::ORIGIN;
    match name {
        "C" if fits(1) => {
            let radius = size(0)? / 2;
            Some(Template::Built {
                flash: vec![Shape::Disc {
                    center: origin,
                    radius,
                }],
                pen: Pen::Round(radius),
            })
        }
        "R" if fits(2) => {
            let (half_w, half_h) = (size(0)? / 2, size(1)? / 2);
            let flash = vec![Shape::Rect {
                center: origin,
                half_w,
                half_h,
                rotation_deg: 0.0,
            }];
            Some(Template::Built {
                flash,
                pen: Pen::Rect { half_w, half_h },
            })
        }
        "O" if fits(2) => {
            let (w, h) = (size(0)?, size(1)?);
            Some(Template::Built {
                flash: vec![obround(w, h)],
                pen: Pen::None,
            })
        }
        "P" => polygon(params, units),
        _ => None,
    }
}

/// A stadium `w` by `h` about the origin, its caps on the longer side.
fn obround(w: i64, h: i64) -> Shape {
    let reach = (w.max(h) - w.min(h)) / 2;
    let tip = if w >= h {
        PointNm::new(reach, 0)
    } else {
        PointNm::new(0, reach)
    };
    if reach == 0 {
        return Shape::Disc {
            center: PointNm::ORIGIN,
            radius: w / 2,
        };
    }
    Shape::Capsule {
        a: PointNm::ORIGIN.minus(tip),
        b: tip,
        radius: w.min(h) / 2,
    }
}

/// `P,<outer diameter>X<corners>[X<rotation>[X<hole>]]`.
fn polygon(params: &[&str], units: Units) -> Option<Template> {
    if !(2..=4).contains(&params.len()) {
        return None;
    }
    let diameter = decimal_to_nm(params[0], units).filter(|v| *v >= 0)?;
    let corners = params[1]
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|n| (3..=12).contains(n))?;
    let rotation = match params.get(2) {
        Some(text) => text.trim().parse::<f64>().ok().filter(|v| v.is_finite())?,
        None => 0.0,
    };
    if let Some(hole) = params.get(3) {
        decimal_to_nm(hole, units).filter(|v| *v >= 0)?;
    }
    let ring = regular_polygon((0.0, 0.0), diameter as f64 / 2.0, corners, rotation);
    Some(Template::Built {
        flash: polygon_shape(&ring).into_iter().collect(),
        pen: Pen::None,
    })
}

impl Reader<'_> {
    pub(super) fn define_aperture(&mut self, statement: &str, line: u32) {
        let Some((d_code, name, params)) = split_definition(statement) else {
            self.unsupported(line, statement);
            return;
        };
        // A D code defined twice keeps its first definition.
        if self.templates.contains_key(&d_code) {
            self.report(line, ProblemKind::BadApertureDefinition { d_code });
            return;
        }
        let units = self.units(line);
        let template = if d_code < FIRST_APERTURE {
            None
        } else if matches!(name, "C" | "R" | "O" | "P") {
            standard(name, &params, units)
        } else {
            Some(self.macro_template(name, &params, units, line))
        };
        let template = template.unwrap_or_else(|| {
            self.report(line, ProblemKind::BadApertureDefinition { d_code });
            Template::Unbuilt
        });
        self.image.apertures.insert(
            d_code,
            ApertureDef {
                d_code,
                definition: format!("%{statement}*%"),
                function: self.aperture_function.clone(),
                at: self.source.at(line),
            },
        );
        self.templates.insert(d_code, template);
    }

    /// A macro aperture. A macro the file never defines is counted and
    /// reported when the file ends; one whose primitives cannot all be
    /// built is reported here and keeps the shapes that could.
    fn macro_template(&mut self, name: &str, params: &[&str], units: Units, line: u32) -> Template {
        let Some(defined) = self.macros.get(name) else {
            return Template::UnknownMacro(name.to_string());
        };
        let values: Option<Vec<f64>> = params
            .iter()
            .map(|p| p.parse::<f64>().ok().filter(|v| v.is_finite()))
            .collect();
        let Some(values) = values else {
            self.report(
                line,
                ProblemKind::BadMacroParameters {
                    name: name.to_string(),
                },
            );
            return Template::Unbuilt;
        };
        let built = defined.build(&values, units);
        // A statement the macro could not read is reported at its `%AM`;
        // a macro of unsupported primitives only is reported as such.
        let bad = built.bad || (built.shapes.is_empty() && built.unsupported.is_empty());
        for code in &built.unsupported {
            self.unsupported(line, &format!("AM primitive {code}"));
        }
        if bad {
            self.report(
                line,
                ProblemKind::BadMacroParameters {
                    name: name.to_string(),
                },
            );
        }
        if built.shapes.is_empty() {
            return Template::Unbuilt;
        }
        Template::Built {
            flash: built.shapes,
            pen: Pen::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MM: Units = Units::Millimetres;

    fn flash(template: Option<Template>) -> Vec<Shape> {
        match template {
            Some(Template::Built { flash, .. }) => flash,
            other => panic!("not built: {other:?}"),
        }
    }

    #[test]
    fn definitions_split_into_code_name_and_parameters() {
        assert_eq!(
            split_definition("ADD10R,1X1.2"),
            Some((10, "R", vec!["1", "1.2"]))
        );
        assert_eq!(split_definition("ADD12M12"), Some((12, "M12", vec![])));
        assert_eq!(
            split_definition("ADD21THERMAL80,2.3X1.5X0.3"),
            Some((21, "THERMAL80", vec!["2.3", "1.5", "0.3"]))
        );
        assert_eq!(split_definition("ADDR,1"), None);
        assert_eq!(split_definition("AMX"), None);
    }

    #[test]
    fn standard_apertures_are_their_shapes() {
        assert_eq!(
            flash(standard("C", &["1.6"], MM)),
            vec![Shape::Disc {
                center: PointNm::ORIGIN,
                radius: 800_000
            }]
        );
        assert_eq!(
            flash(standard("R", &["1", "1.2"], MM)),
            vec![Shape::Rect {
                center: PointNm::ORIGIN,
                half_w: 500_000,
                half_h: 600_000,
                rotation_deg: 0.0
            }]
        );
        assert_eq!(
            flash(standard("O", &["2", "1"], MM)),
            vec![Shape::Capsule {
                a: PointNm::new(-500_000, 0),
                b: PointNm::new(500_000, 0),
                radius: 500_000
            }]
        );
        let hexagon = flash(standard("P", &["2", "6", "30"], MM));
        let Shape::Polygon { ring, .. } = &hexagon[0] else {
            panic!("a polygon");
        };
        assert_eq!(ring.len(), 6);
        // With a hole the shape is the same; the hole is not cut out.
        assert!(standard("C", &["1", "0.5"], MM).is_some());
    }

    #[test]
    fn bad_standard_parameters_build_nothing() {
        assert!(standard("C", &[], MM).is_none());
        assert!(standard("C", &["-1"], MM).is_none());
        assert!(standard("R", &["1"], MM).is_none());
        assert!(standard("R", &["1", "x"], MM).is_none());
        assert!(standard("C", &["1", "2", "3"], MM).is_none());
        assert!(standard("P", &["1", "2"], MM).is_none());
        assert!(standard("P", &["1", "13"], MM).is_none());
    }
}
