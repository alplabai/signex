//! Built-in body of a power port that carries no library graphics.
//!
//! The Active Bar places a power port as a bare `Symbol` (`is_power`, a
//! `power:<style>` `lib_id`) without adding a library entry to the sheet, so
//! there is nothing for `lower_symbol` to lower. [`power_port_graphics`] gives
//! such a symbol its body in library space (Y-up, connection point at the
//! origin) so the canvas, hit-test, placement ghost and SVG export all draw
//! and measure the same shape.

use super::{FillType, Graphic, Point};

/// Length of the stem from the connection point to the symbol head, mm.
const STEM_MM: f64 = 1.27;
/// Half width of the head, mm.
const HEAD_HALF_MM: f64 = 0.635;

/// Head shapes a power port can take, chosen from the `lib_id` style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PowerPortShape {
    Ground,
    Bar,
    Arrow,
    Circle,
    Wave,
}

fn shape_of(lib_id: &str) -> PowerPortShape {
    let style = lib_id.rsplit(':').next().unwrap_or(lib_id).to_lowercase();
    match style.as_str() {
        "arrow" => PowerPortShape::Arrow,
        "wave" => PowerPortShape::Wave,
        "circle" | "vcc" => PowerPortShape::Circle,
        "bar" => PowerPortShape::Bar,
        s if s.contains("gnd") || s.contains("ground") || s == "earth" => PowerPortShape::Ground,
        _ => PowerPortShape::Bar,
    }
}

fn line(a: (f64, f64), b: (f64, f64)) -> Graphic {
    Graphic::Polyline {
        points: vec![Point::new(a.0, a.1), Point::new(b.0, b.1)],
        width: 0.0,
        fill: FillType::None,
    }
}

/// Library-space graphics of the power port style named by `lib_id`.
#[must_use]
pub fn power_port_graphics(lib_id: &str) -> Vec<Graphic> {
    let top = STEM_MM;
    match shape_of(lib_id) {
        PowerPortShape::Ground => {
            let mut out = vec![line((0.0, 0.0), (0.0, -top))];
            for (i, half) in [0.9525, 0.6, 0.25].into_iter().enumerate() {
                let y = -top - 0.5 * i as f64;
                out.push(line((-half, y), (half, y)));
            }
            out
        }
        PowerPortShape::Bar => vec![
            line((0.0, 0.0), (0.0, top)),
            line((-HEAD_HALF_MM, top), (HEAD_HALF_MM, top)),
        ],
        PowerPortShape::Arrow => vec![
            line((0.0, 0.0), (0.0, top)),
            Graphic::Polyline {
                points: vec![
                    Point::new(-HEAD_HALF_MM, top),
                    Point::new(HEAD_HALF_MM, top),
                    Point::new(0.0, top + 1.0),
                    Point::new(-HEAD_HALF_MM, top),
                ],
                width: 0.0,
                fill: FillType::Outline,
            },
        ],
        PowerPortShape::Circle => vec![
            line((0.0, 0.0), (0.0, top)),
            Graphic::Circle {
                center: Point::new(0.0, top + HEAD_HALF_MM),
                radius: HEAD_HALF_MM,
                width: 0.0,
                fill: FillType::None,
            },
        ],
        PowerPortShape::Wave => {
            let y = top + 0.4;
            vec![
                line((0.0, 0.0), (0.0, top)),
                Graphic::Polyline {
                    points: (0..=8)
                        .map(|i| {
                            let t = i as f64 / 8.0;
                            Point::new(
                                (t - 0.5) * 2.0 * HEAD_HALF_MM,
                                y + 0.3 * (t * std::f64::consts::TAU).sin(),
                            )
                        })
                        .collect(),
                    width: 0.0,
                    fill: FillType::None,
                },
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_comes_from_the_lib_id() {
        assert_eq!(shape_of("power:GND"), PowerPortShape::Ground);
        assert_eq!(shape_of("power:+5V"), PowerPortShape::Bar);
        assert_eq!(shape_of("power:VCC"), PowerPortShape::Circle);
        assert_eq!(shape_of("power:arrow"), PowerPortShape::Arrow);
        assert_eq!(shape_of("power:wave"), PowerPortShape::Wave);
        assert_eq!(shape_of("power:circle"), PowerPortShape::Circle);
        assert_eq!(shape_of("power:bar"), PowerPortShape::Bar);
    }

    #[test]
    fn every_style_has_a_body() {
        for id in [
            "power:GND",
            "power:bar",
            "power:arrow",
            "power:circle",
            "power:wave",
        ] {
            assert!(!power_port_graphics(id).is_empty(), "{id}");
        }
    }
}
