//! Aperture macros (`%AM`): their statements read once, then built into
//! shapes for each `%AD` that names them, with its parameters as `$1`,
//! `$2`, ... Primitives 1 circle, 4 outline, 5 polygon, 7 thermal, 20 (and
//! its old alias 2) vector line, 21 centre line and 22 lower-left line are
//! built; 0 is a comment; any other code is reported as unsupported. An
//! exposure-off primitive is not cut out of the flash (docs/GERBER_VIEWER.md,
//! "What is drawn").

use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, SQRT_2, TAU};

use pcb_geom::{MAX_ABS_NM, PointNm, Shape};

use super::arcs::{chords, rotate};
use super::expr::{self, Expr, ExprError};
use crate::image::{Units, polygon_shape};
use crate::number::nm_per_unit;

/// The most corners an outline primitive may have.
const MAX_OUTLINE_CORNERS: f64 = 10_000.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Item {
    Primitive { code: u32, args: Vec<Expr> },
    Assign { var: u32, value: Expr },
}

/// One macro as defined.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Macro {
    pub name: String,
    pub items: Vec<Item>,
    /// A statement could not be read; the rest still build.
    pub bad: bool,
}

/// What building a macro made.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Built {
    pub shapes: Vec<Shape>,
    /// Primitive codes not supported, in order met.
    pub unsupported: Vec<u32>,
    /// A primitive's parameters made no shape.
    pub bad: bool,
}

/// Read an `%AM` command: the first statement is `AM<name>`. `None` when
/// it names nothing.
pub(crate) fn parse_macro(statements: &[(String, u32)]) -> Option<Macro> {
    let (head, body) = statements.split_first()?;
    let name = head.0.strip_prefix("AM")?.trim();
    if name.is_empty() {
        return None;
    }
    let mut parsed = Macro {
        name: name.to_string(),
        items: Vec::new(),
        bad: false,
    };
    for (statement, _) in body {
        match parse_item(statement) {
            Ok(Some(item)) => parsed.items.push(item),
            Ok(None) => {}
            Err(_) => parsed.bad = true,
        }
    }
    Some(parsed)
}

fn parse_item(statement: &str) -> Result<Option<Item>, ExprError> {
    if let Some(assignment) = statement.strip_prefix('$') {
        let (var, value) = assignment.split_once('=').ok_or(ExprError::Unexpected(0))?;
        let var = var
            .trim()
            .parse::<u32>()
            .map_err(|_| ExprError::Unexpected(0))?;
        return Ok(Some(Item::Assign {
            var,
            value: expr::parse(value)?,
        }));
    }
    let mut fields = statement.split(',');
    let code = fields.next().unwrap_or_default().trim();
    if code == "0" || code.starts_with("0 ") {
        return Ok(None);
    }
    let code = code.parse::<u32>().map_err(|_| ExprError::Unexpected(0))?;
    let args = fields.map(expr::parse).collect::<Result<Vec<_>, _>>()?;
    Ok(Some(Item::Primitive { code, args }))
}

impl Macro {
    /// The macro's shapes with `params` as `$1`, `$2`, ..., in `units`,
    /// about the flash point.
    pub(crate) fn build(&self, params: &[f64], units: Units) -> Built {
        let mut vars: BTreeMap<u32, f64> = (1..).zip(params.iter().copied()).collect();
        let scale = Scale(nm_per_unit(units) as f64);
        let mut built = Built::default();
        for item in &self.items {
            match item {
                Item::Assign { var, value } => {
                    let v = value.eval(&vars);
                    vars.insert(*var, v);
                }
                Item::Primitive { code, args } => {
                    let values: Vec<f64> = args.iter().map(|a| a.eval(&vars)).collect();
                    match primitive(*code, &values, scale, &mut built.shapes) {
                        Ok(()) => {}
                        Err(Fault::Unsupported) => built.unsupported.push(*code),
                        Err(Fault::Bad) => built.bad = true,
                    }
                }
            }
        }
        built
    }
}

enum Fault {
    Unsupported,
    Bad,
}

/// nm per file unit.
#[derive(Clone, Copy)]
struct Scale(f64);

impl Scale {
    /// A length or coordinate in nm, within the geometry's reach.
    fn nm(self, value: f64) -> Result<f64, Fault> {
        let nm = value * self.0;
        (nm.is_finite() && nm.abs() <= MAX_ABS_NM as f64)
            .then_some(nm)
            .ok_or(Fault::Bad)
    }

    /// A size: not negative.
    fn size(self, value: f64) -> Result<f64, Fault> {
        self.nm(value)
            .and_then(|v| if v < 0.0 { Err(Fault::Bad) } else { Ok(v) })
    }
}

fn to_point(p: (f64, f64)) -> PointNm {
    PointNm::new(p.0.round() as i64, p.1.round() as i64)
}

/// The arguments, or `Bad` when there are fewer than `n`.
fn at_least(args: &[f64], n: usize) -> Result<&[f64], Fault> {
    if args.len() >= n && args.iter().all(|v| v.is_finite()) {
        Ok(args)
    } else {
        Err(Fault::Bad)
    }
}

/// The optional trailing rotation at `index`, 0 when absent.
fn rotation(args: &[f64], index: usize) -> f64 {
    args.get(index).copied().unwrap_or(0.0)
}

fn primitive(code: u32, args: &[f64], scale: Scale, out: &mut Vec<Shape>) -> Result<(), Fault> {
    let builder: fn(&[f64], Scale, &mut Vec<Shape>) -> Result<(), Fault> = match code {
        1 => circle,
        2 | 20 => vector_line,
        21 => centre_line,
        22 => lower_left_line,
        4 => outline,
        5 => polygon,
        7 => return thermal(at_least(args, 5)?, scale, out),
        _ => return Err(Fault::Unsupported),
    };
    // Every other primitive starts with its exposure; off is not drawn.
    if at_least(args, 1)?[0] == 0.0 {
        return Ok(());
    }
    builder(args, scale, out)
}

fn circle(args: &[f64], scale: Scale, out: &mut Vec<Shape>) -> Result<(), Fault> {
    let a = at_least(args, 4)?;
    let diameter = scale.size(a[1])?;
    let center = rotate((scale.nm(a[2])?, scale.nm(a[3])?), rotation(a, 4));
    out.push(Shape::Disc {
        center: to_point(center),
        radius: (diameter / 2.0).round() as i64,
    });
    Ok(())
}

fn vector_line(args: &[f64], scale: Scale, out: &mut Vec<Shape>) -> Result<(), Fault> {
    let a = at_least(args, 6)?;
    let half = scale.size(a[1])? / 2.0;
    let (sx, sy) = (scale.nm(a[2])?, scale.nm(a[3])?);
    let (ex, ey) = (scale.nm(a[4])?, scale.nm(a[5])?);
    let length = (ex - sx).hypot(ey - sy);
    if length == 0.0 {
        return Ok(());
    }
    let (nx, ny) = (-(ey - sy) / length * half, (ex - sx) / length * half);
    let turn = rotation(a, 6);
    let corners = [
        (sx - nx, sy - ny),
        (ex - nx, ey - ny),
        (ex + nx, ey + ny),
        (sx + nx, sy + ny),
    ]
    .map(|c| to_point(rotate(c, turn)));
    out.extend(polygon_shape(&corners));
    Ok(())
}

fn rect(w: f64, h: f64, center: (f64, f64), turn: f64, out: &mut Vec<Shape>) {
    out.push(Shape::Rect {
        center: to_point(rotate(center, turn)),
        half_w: (w / 2.0).round() as i64,
        half_h: (h / 2.0).round() as i64,
        rotation_deg: turn,
    });
}

fn centre_line(args: &[f64], scale: Scale, out: &mut Vec<Shape>) -> Result<(), Fault> {
    let a = at_least(args, 5)?;
    let (w, h) = (scale.size(a[1])?, scale.size(a[2])?);
    let center = (scale.nm(a[3])?, scale.nm(a[4])?);
    rect(w, h, center, rotation(a, 5), out);
    Ok(())
}

fn lower_left_line(args: &[f64], scale: Scale, out: &mut Vec<Shape>) -> Result<(), Fault> {
    let a = at_least(args, 5)?;
    let (w, h) = (scale.size(a[1])?, scale.size(a[2])?);
    let center = (scale.nm(a[3])? + w / 2.0, scale.nm(a[4])? + h / 2.0);
    rect(w, h, center, rotation(a, 5), out);
    Ok(())
}

fn outline(args: &[f64], scale: Scale, out: &mut Vec<Shape>) -> Result<(), Fault> {
    let a = at_least(args, 2)?;
    let n = a[1];
    if n.fract() != 0.0 || !(1.0..=MAX_OUTLINE_CORNERS).contains(&n) {
        return Err(Fault::Bad);
    }
    // `n` corners and the first again, then the rotation.
    let coords = 2 * (n as usize + 1);
    let a = at_least(args, 2 + coords)?;
    let turn = rotation(a, 2 + coords);
    let corners = a[2..2 + coords]
        .chunks_exact(2)
        .map(|xy| Ok(to_point(rotate((scale.nm(xy[0])?, scale.nm(xy[1])?), turn))))
        .collect::<Result<Vec<_>, Fault>>()?;
    out.push(polygon_shape(&corners).ok_or(Fault::Bad)?);
    Ok(())
}

/// The corners of a regular polygon of `n` corners on a circle of `radius`
/// about `center`, the first on the +x axis, all turned `turn` degrees
/// about the origin.
pub(crate) fn regular_polygon(center: (f64, f64), radius: f64, n: u32, turn: f64) -> Vec<PointNm> {
    (0..n)
        .map(|k| {
            let a = TAU * f64::from(k) / f64::from(n);
            let corner = (center.0 + radius * a.cos(), center.1 + radius * a.sin());
            to_point(rotate(corner, turn))
        })
        .collect()
}

fn polygon(args: &[f64], scale: Scale, out: &mut Vec<Shape>) -> Result<(), Fault> {
    let a = at_least(args, 5)?;
    let n = a[1];
    if n.fract() != 0.0 || !(3.0..=12.0).contains(&n) {
        return Err(Fault::Bad);
    }
    let center = (scale.nm(a[2])?, scale.nm(a[3])?);
    let radius = scale.size(a[4])? / 2.0;
    let corners = regular_polygon(center, radius, n as u32, rotation(a, 5));
    out.push(polygon_shape(&corners).ok_or(Fault::Bad)?);
    Ok(())
}

/// Four quarter rings with a gap along each axis, turned about the origin.
fn thermal(a: &[f64], scale: Scale, out: &mut Vec<Shape>) -> Result<(), Fault> {
    let center = (scale.nm(a[0])?, scale.nm(a[1])?);
    let outer = scale.size(a[2])? / 2.0;
    let inner = scale.size(a[3])? / 2.0;
    let gap = scale.size(a[4])? / 2.0;
    if outer <= inner || gap >= outer / SQRT_2 {
        return Err(Fault::Bad);
    }
    let piece = thermal_piece(outer, inner, gap);
    let turn = rotation(a, 5);
    for quarter in 0..4 {
        let corners: Vec<PointNm> = piece
            .iter()
            .map(|&p| {
                let (x, y) = rotate(p, 90.0 * f64::from(quarter));
                to_point(rotate((x + center.0, y + center.1), turn))
            })
            .collect();
        out.extend(polygon_shape(&corners));
    }
    Ok(())
}

/// The first quarter of a thermal about its centre: the outer arc, then
/// the inner arc back (or the gaps' corner when the gaps meet inside it).
fn thermal_piece(outer: f64, inner: f64, gap: f64) -> Vec<(f64, f64)> {
    let arc = |radius: f64, from: f64, to: f64| {
        let n = chords(radius, to - from);
        (0..=n).map(move |k| {
            let a = from + (to - from) * k as f64 / n as f64;
            (radius * a.cos(), radius * a.sin())
        })
    };
    let start = (gap / outer).asin();
    let mut piece: Vec<(f64, f64)> = arc(outer, start, FRAC_PI_2 - start).collect();
    if inner > 0.0 && gap < inner / SQRT_2 {
        let start = (gap / inner).asin();
        piece.extend(arc(inner, FRAC_PI_2 - start, start));
    } else {
        piece.push((gap, gap));
    }
    piece
}

#[cfg(test)]
mod tests {
    use super::*;

    fn statements(lines: &[&str]) -> Vec<(String, u32)> {
        (1..).zip(lines).map(|(n, s)| (s.to_string(), n)).collect()
    }

    fn build(lines: &[&str], params: &[f64]) -> Built {
        parse_macro(&statements(lines))
            .expect("a macro")
            .build(params, Units::Millimetres)
    }

    #[test]
    fn the_exporters_turned_obround_builds_a_bar_and_two_discs() {
        let built = build(
            &[
                "AMM12",
                "21,1,1,1,0,0,90",
                "1,1,1,0.5,0,90",
                "1,1,1,-0.5,0,90",
            ],
            &[],
        );
        assert!(!built.bad && built.unsupported.is_empty());
        assert_eq!(built.shapes.len(), 3);
        let Shape::Rect {
            center,
            half_w,
            half_h,
            rotation_deg,
        } = built.shapes[0]
        else {
            panic!("a rect");
        };
        assert_eq!(
            (center, half_w, half_h),
            (PointNm::ORIGIN, 500_000, 500_000)
        );
        assert_eq!(rotation_deg, 90.0);
        // The disc at (0.5, 0) turned a quarter lands on (0, 0.5).
        assert_eq!(
            built.shapes[1],
            Shape::Disc {
                center: PointNm::new(0, 500_000),
                radius: 500_000
            }
        );
    }

    #[test]
    fn variables_comments_and_exposure_off() {
        let built = build(
            &[
                "AMDONUT",
                "0 a ring as two circles",
                "$3=$1x2",
                "1,1,$3,0,0",
                "1,0,$2,0,0",
            ],
            &[0.5, 0.3],
        );
        assert_eq!(
            built.shapes,
            vec![Shape::Disc {
                center: PointNm::ORIGIN,
                radius: 500_000
            }]
        );
    }

    #[test]
    fn an_outline_is_one_polygon() {
        let built = build(
            &["AMM13", "4,1,3,-0.75,-0.5,0.75,-0.5,1.25,0.5,-0.75,-0.5,0"],
            &[],
        );
        let Shape::Polygon { ring, radius } = &built.shapes[0] else {
            panic!("a polygon");
        };
        assert_eq!(*radius, 0);
        assert_eq!(
            &**ring,
            &[
                PointNm::new(-750_000, -500_000),
                PointNm::new(750_000, -500_000),
                PointNm::new(1_250_000, 500_000)
            ]
        );
    }

    #[test]
    fn a_thermal_is_four_pieces_inside_its_ring() {
        let built = build(&["AMTH", "7,0,0,2.3,1.5,0.3,45"], &[]);
        assert_eq!(built.shapes.len(), 4);
        for shape in &built.shapes {
            let Shape::Polygon { ring, .. } = shape else {
                panic!("a polygon");
            };
            for p in ring.iter() {
                let r = (p.x as f64).hypot(p.y as f64);
                assert!((749_000.0..=1_151_000.0).contains(&r), "{p:?} at {r}");
            }
        }
    }

    #[test]
    fn unsupported_and_bad_primitives_are_reported_and_the_rest_built() {
        let built = build(
            &[
                "AMMIX",
                "6,0,0,5,0.5,0.5,2,0.1,6,0",
                "5,1,13,0,0,1,0",
                "1,1,1,0,0",
            ],
            &[],
        );
        assert_eq!(built.unsupported, vec![6]);
        assert!(built.bad);
        assert_eq!(built.shapes.len(), 1);
        let broken = parse_macro(&statements(&["AMX", "1,1,(", "1,1,1,0,0"])).expect("a macro");
        assert!(broken.bad);
        assert_eq!(broken.items.len(), 1);
        assert_eq!(parse_macro(&statements(&["AM"])), None);
    }

    #[test]
    fn a_regular_polygon_starts_on_the_x_axis() {
        let corners = regular_polygon((0.0, 0.0), 1_000.0, 4, 0.0);
        assert_eq!(corners[0], PointNm::new(1_000, 0));
        assert_eq!(corners[1], PointNm::new(0, 1_000));
        let turned = regular_polygon((0.0, 0.0), 1_000.0, 4, 90.0);
        assert_eq!(turned[0], PointNm::new(0, 1_000));
    }
}
