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

/// Style of a power port, read from its `lib_id`. This one classifier is
/// shared by the built-in body below and the Properties panel's Style row,
/// so the style the panel shows is always the shape that is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerPortStyle {
    Bar,
    Arrow,
    Wave,
    Circle,
    PowerGround,
    SignalGround,
    Earth,
}

impl PowerPortStyle {
    /// Every style, in the order the Properties panel lists them.
    pub const ALL: [Self; 7] = [
        Self::Bar,
        Self::Arrow,
        Self::Wave,
        Self::Circle,
        Self::PowerGround,
        Self::SignalGround,
        Self::Earth,
    ];

    /// The style a `lib_id` names. Anything unrecognised, such as
    /// `power:+5V` or `power:VCC`, is a bar.
    #[must_use]
    pub fn of(lib_id: &str) -> Self {
        let lid = lib_id.to_lowercase();
        if lid.contains("gndref") {
            Self::SignalGround
        } else if lid.contains("earth") {
            Self::Earth
        } else if lid.contains("gnd") || lid.contains("ground") {
            Self::PowerGround
        } else if lid.contains("arrow") {
            Self::Arrow
        } else if lid.contains("wave") {
            Self::Wave
        } else if lid.contains("circle") {
            Self::Circle
        } else {
            Self::Bar
        }
    }

    /// The name the Properties panel shows.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Bar => "Bar",
            Self::Arrow => "Arrow",
            Self::Wave => "Wave",
            Self::Circle => "Circle",
            Self::PowerGround => "Power Ground",
            Self::SignalGround => "Signal Ground",
            Self::Earth => "Earth",
        }
    }

    /// The `power:<tag>` keyword that selects this style.
    #[must_use]
    pub fn lib_tag(self) -> &'static str {
        match self {
            Self::Bar => "bar",
            Self::Arrow => "arrow",
            Self::Wave => "wave",
            Self::Circle => "circle",
            Self::PowerGround => "GND",
            Self::SignalGround => "GNDREF",
            Self::Earth => "Earth",
        }
    }

    /// Whether the body hangs below the connection point (the grounds)
    /// rather than rising above it.
    #[must_use]
    pub fn hangs_down(self) -> bool {
        matches!(self, Self::PowerGround | Self::SignalGround | Self::Earth)
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
    match PowerPortStyle::of(lib_id) {
        PowerPortStyle::PowerGround => {
            let mut out = vec![line((0.0, 0.0), (0.0, -top))];
            for (i, half) in [0.9525, 0.6, 0.25].into_iter().enumerate() {
                let y = -top - 0.5 * i as f64;
                out.push(line((-half, y), (half, y)));
            }
            out
        }
        PowerPortStyle::Bar => vec![
            line((0.0, 0.0), (0.0, top)),
            line((-HEAD_HALF_MM, top), (HEAD_HALF_MM, top)),
        ],
        PowerPortStyle::Arrow => vec![
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
        PowerPortStyle::Circle => vec![
            line((0.0, 0.0), (0.0, top)),
            Graphic::Circle {
                center: Point::new(0.0, top + HEAD_HALF_MM),
                radius: HEAD_HALF_MM,
                width: 0.0,
                fill: FillType::None,
            },
        ],
        PowerPortStyle::SignalGround => vec![
            line((0.0, 0.0), (0.0, -top)),
            Graphic::Polyline {
                points: vec![
                    Point::new(-0.9525, -top),
                    Point::new(0.9525, -top),
                    Point::new(0.0, -top - 1.0),
                    Point::new(-0.9525, -top),
                ],
                width: 0.0,
                fill: FillType::None,
            },
        ],
        PowerPortStyle::Earth => {
            let mut out = vec![
                line((0.0, 0.0), (0.0, -top)),
                line((-0.9525, -top), (0.9525, -top)),
            ];
            for x in [-0.9525, 0.0, 0.9525] {
                out.push(line((x, -top), (x - 0.5, -top - 0.6)));
            }
            out
        }
        PowerPortStyle::Wave => {
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
        assert_eq!(PowerPortStyle::of("power:GND"), PowerPortStyle::PowerGround);
        assert_eq!(
            PowerPortStyle::of("power:GNDREF"),
            PowerPortStyle::SignalGround
        );
        assert_eq!(PowerPortStyle::of("power:Earth"), PowerPortStyle::Earth);
        assert_eq!(PowerPortStyle::of("power:+5V"), PowerPortStyle::Bar);
        assert_eq!(PowerPortStyle::of("power:VCC"), PowerPortStyle::Bar);
        assert_eq!(PowerPortStyle::of("power:arrow"), PowerPortStyle::Arrow);
        assert_eq!(PowerPortStyle::of("power:wave"), PowerPortStyle::Wave);
        assert_eq!(PowerPortStyle::of("power:circle"), PowerPortStyle::Circle);
    }

    #[test]
    fn each_style_round_trips_through_its_lib_tag() {
        for style in PowerPortStyle::ALL {
            let lib_id = format!("power:{}", style.lib_tag());
            assert_eq!(PowerPortStyle::of(&lib_id), style, "{lib_id}");
        }
    }

    #[test]
    fn the_body_hangs_the_way_the_style_says() {
        // The Properties panel keeps a port's orientation across a style
        // change by trusting `hangs_down`, so the drawn body must agree.
        for style in PowerPortStyle::ALL {
            let lib_id = format!("power:{}", style.lib_tag());
            let ys: Vec<f64> = power_port_graphics(&lib_id)
                .iter()
                .flat_map(|g| match g {
                    Graphic::Polyline { points, .. } => points.iter().map(|p| p.y).collect(),
                    Graphic::Circle { center, .. } => vec![center.y],
                    _ => vec![],
                })
                .collect();
            let lowest = ys.iter().copied().fold(f64::INFINITY, f64::min);
            let highest = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            if style.hangs_down() {
                assert!(lowest < 0.0 && highest <= 0.0, "{lib_id} should hang down");
            } else {
                assert!(highest > 0.0 && lowest >= 0.0, "{lib_id} should rise");
            }
        }
    }

    #[test]
    fn every_style_has_a_body() {
        for style in PowerPortStyle::ALL {
            let lib_id = format!("power:{}", style.lib_tag());
            assert!(!power_port_graphics(&lib_id).is_empty(), "{lib_id}");
        }
    }
}
