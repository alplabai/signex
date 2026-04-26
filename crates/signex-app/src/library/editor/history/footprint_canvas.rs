//! Footprint diff canvas — pads instead of pins.
//!
//! Mirrors `symbol_canvas.rs` but extracts `(pad N …)` nodes and only
//! supports added / removed (no "moved" — `FootprintDiff` doesn't
//! track moved pads in Phase 1; if/when that changes, the additional
//! arrow logic is a copy-paste from `symbol_canvas.rs`).

use std::collections::BTreeSet;

use iced::widget::canvas::{self, Cache, Geometry, Path, Stroke, Text};
use iced::{Color, Point, Rectangle, Theme};
use standard_parser::sexpr::{self, SExpr};

use super::symbol_canvas::Side;

/// One pad extracted from a `(footprint …)` body.
#[derive(Clone, Debug, PartialEq)]
pub struct Pad {
    pub number: String,
    pub pos: [f64; 2],
}

/// Canvas program rendering one side of the footprint diff.
pub struct FootprintDiffCanvas {
    pub side: Side,
    pub pads: Vec<Pad>,
    pub added: BTreeSet<String>,
    pub removed: BTreeSet<String>,
    pub cache: Cache,
}

impl FootprintDiffCanvas {
    pub fn new(side: Side, pads: Vec<Pad>, added: Vec<String>, removed: Vec<String>) -> Self {
        Self {
            side,
            pads,
            added: added.into_iter().collect(),
            removed: removed.into_iter().collect(),
            cache: Cache::new(),
        }
    }

    fn bounds(&self) -> (f64, f64, f64, f64) {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        for pad in &self.pads {
            min_x = min_x.min(pad.pos[0]);
            min_y = min_y.min(pad.pos[1]);
            max_x = max_x.max(pad.pos[0]);
            max_y = max_y.max(pad.pos[1]);
        }
        if min_x > max_x {
            (-3.0, -3.0, 3.0, 3.0)
        } else {
            let pad = ((max_x - min_x).max(max_y - min_y)) * 0.2 + 1.0;
            (min_x - pad, min_y - pad, max_x + pad, max_y + pad)
        }
    }
}

impl canvas::Program<(), Theme> for FootprintDiffCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let geom = self.cache.draw(renderer, bounds.size(), |frame| {
            let (bx0, by0, bx1, by1) = self.bounds();
            let bw = bx1 - bx0;
            let bh = by1 - by0;
            if bw <= 0.0 || bh <= 0.0 {
                return;
            }

            let scale_x = bounds.width as f64 / bw;
            let scale_y = bounds.height as f64 / bh;
            let scale = scale_x.min(scale_y) * 0.85;

            let cx = bounds.width / 2.0;
            let cy = bounds.height / 2.0;
            let mid_x = (bx0 + bx1) / 2.0;
            let mid_y = (by0 + by1) / 2.0;

            let tx = |x: f64, y: f64| -> Point {
                Point::new(
                    cx + ((x - mid_x) * scale) as f32,
                    cy + ((y - mid_y) * scale) as f32,
                )
            };

            let added_color = Color::from_rgb(0.20, 0.78, 0.34);
            let removed_color = Color::from_rgb(0.93, 0.25, 0.25);
            let neutral_color = Color::from_rgb(0.78, 0.78, 0.83);
            let label_color = Color::from_rgb(0.55, 0.58, 0.65);

            for pad in &self.pads {
                let p = tx(pad.pos[0], pad.pos[1]);
                let is_added = self.added.contains(&pad.number) && self.side == Side::Next;
                let is_removed = self.removed.contains(&pad.number) && self.side == Side::Prev;
                let color = if is_added {
                    added_color
                } else if is_removed {
                    removed_color
                } else {
                    neutral_color
                };

                let r = 4.5_f32;
                if is_added || is_removed {
                    frame.stroke(
                        &Path::circle(p, r),
                        Stroke::default().with_color(color).with_width(1.6),
                    );
                } else {
                    frame.fill(&Path::circle(p, r), color);
                }

                if is_removed {
                    let strike_a = Point::new(p.x - r - 1.0, p.y);
                    let strike_b = Point::new(p.x + r + 1.0, p.y);
                    frame.stroke(
                        &Path::line(strike_a, strike_b),
                        Stroke::default().with_color(removed_color).with_width(1.4),
                    );
                }

                if !pad.number.is_empty() {
                    frame.fill_text(Text {
                        content: pad.number.clone(),
                        position: Point::new(p.x + 6.0, p.y - 5.0),
                        size: 10.0.into(),
                        color: label_color,
                        ..Text::default()
                    });
                }
            }
        });

        vec![geom]
    }
}

/// Extract pads from a `(footprint …)` body. Same recursion shape as
/// the symbol pin extractor — first-arg of `(pad N …)` is the pad
/// number, position from the standard `(at x y [rot])` child.
pub fn extract_pads(sexpr_text: &str) -> Vec<Pad> {
    let trimmed = sexpr_text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let parsed = match sexpr::parse(trimmed) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    let mut stack: Vec<&SExpr> = vec![&parsed];
    while let Some(node) = stack.pop() {
        if node.keyword() == Some("pad")
            && let Some(num) = node.first_arg()
        {
            let pos = at_position(node).unwrap_or([0.0, 0.0]);
            out.push(Pad {
                number: num.to_string(),
                pos,
            });
        }
        for child in node.children() {
            if matches!(child, SExpr::List(_)) {
                stack.push(child);
            }
        }
    }
    out
}

fn at_position(node: &SExpr) -> Option<[f64; 2]> {
    let at = node.find("at")?;
    let x = at.arg_f64(0)?;
    let y = at.arg_f64(1)?;
    Some([x, y])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_pads_two_pads() {
        let fp = r#"(footprint "R_0805_2012Metric"
            (pad "1" smd rect (at -1.0 0))
            (pad "2" smd rect (at  1.0 0)))"#;
        let pads = extract_pads(fp);
        assert_eq!(pads.len(), 2);
        let nums: BTreeSet<String> = pads.iter().map(|p| p.number.clone()).collect();
        assert!(nums.contains("1"));
        assert!(nums.contains("2"));
    }

    #[test]
    fn extract_pads_empty_body_returns_empty() {
        assert!(extract_pads("").is_empty());
        assert!(extract_pads("(footprint \"empty\")").is_empty());
    }
}
