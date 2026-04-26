//! Symbol diff canvas — render one side of a revision pair.
//!
//! Pins are pulled from the embedded S-expression (same parsing as
//! `signex_library::diff::extract_pins`, kept duplicated here because
//! the library crate's helper is private). Each pin is colored by the
//! diff result:
//!
//! * Added pins (only present in `next`)   → outlined green.
//! * Removed pins (only present in `prev`) → outlined red.
//! * Moved pins                            → blue arrow from old → new
//!   when both sides render the same canvas; the "this side" pin is
//!   drawn in the neutral pin color.
//! * Unchanged pins                        → neutral pin color.
//!
//! Position equality matches the library crate's epsilon (1 µm).

use std::collections::BTreeSet;

use iced::widget::canvas::{self, Cache, Geometry, Path, Stroke, Text};
use iced::{Color, Point, Rectangle, Theme};
use standard_parser::sexpr::{self, SExpr};

/// Side of the diff being rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Prev,
    Next,
}

/// One pin extracted from a `(symbol …)` body.
#[derive(Clone, Debug, PartialEq)]
pub struct Pin {
    /// Compound `<unit>:<number>` key (matches the diff crate's keying).
    pub key: String,
    pub pos: [f64; 2],
}

/// Canvas program that renders the pins for one side of the diff with
/// added/removed/moved coloring.
pub struct SymbolDiffCanvas {
    pub side: Side,
    pub pins: Vec<Pin>,
    pub added: BTreeSet<String>,
    pub removed: BTreeSet<String>,
    /// `(key, prev_pos, next_pos)` — present on both sides.
    pub moved: Vec<(String, [f64; 2], [f64; 2])>,
    pub cache: Cache,
}

impl SymbolDiffCanvas {
    pub fn new(
        side: Side,
        pins: Vec<Pin>,
        added: Vec<String>,
        removed: Vec<String>,
        moved: Vec<(String, [f64; 2], [f64; 2])>,
    ) -> Self {
        Self {
            side,
            pins,
            added: added.into_iter().collect(),
            removed: removed.into_iter().collect(),
            moved,
            cache: Cache::new(),
        }
    }

    fn bounds(&self) -> (f64, f64, f64, f64) {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        let mut expand = |x: f64, y: f64| {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        };

        for pin in &self.pins {
            expand(pin.pos[0], pin.pos[1]);
        }
        // Also expand to cover both endpoints of moved arrows so the
        // arrow heads aren't clipped against the frame edge.
        for (_, prev, next) in &self.moved {
            expand(prev[0], prev[1]);
            expand(next[0], next[1]);
        }

        if min_x > max_x {
            (-5.0, -5.0, 5.0, 5.0)
        } else {
            let pad = ((max_x - min_x).max(max_y - min_y)) * 0.15 + 2.0;
            (min_x - pad, min_y - pad, max_x + pad, max_y + pad)
        }
    }
}

impl canvas::Program<(), Theme> for SymbolDiffCanvas {
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
            let scale = scale_x.min(scale_y) * 0.9;

            let cx = bounds.width / 2.0;
            let cy = bounds.height / 2.0;
            let mid_x = (bx0 + bx1) / 2.0;
            let mid_y = (by0 + by1) / 2.0;

            // Standard Y is positive-down at parse time; we already match
            // that convention in the bounds calc, so just translate.
            let tx = |x: f64, y: f64| -> Point {
                Point::new(
                    cx + ((x - mid_x) * scale) as f32,
                    cy + ((y - mid_y) * scale) as f32,
                )
            };

            let added_color = Color::from_rgb(0.20, 0.78, 0.34);
            let removed_color = Color::from_rgb(0.93, 0.25, 0.25);
            let moved_color = Color::from_rgb(0.30, 0.55, 0.95);
            let neutral_color = Color::from_rgb(0.70, 0.72, 0.78);
            let label_color = Color::from_rgb(0.55, 0.58, 0.65);

            // Draw moved-arrows first so the pin glyphs land on top.
            // Only the `Next` side draws arrows so we don't render
            // them twice when the two canvases are placed side-by-side.
            if self.side == Side::Next {
                for (_key, from, to) in &self.moved {
                    let p0 = tx(from[0], from[1]);
                    let p1 = tx(to[0], to[1]);
                    frame.stroke(
                        &Path::line(p0, p1),
                        Stroke::default().with_color(moved_color).with_width(1.4),
                    );
                    // Tiny arrowhead at p1.
                    let ang = (p1.y - p0.y).atan2(p1.x - p0.x);
                    let head = 5.0_f32;
                    let a1 = ang + std::f32::consts::FRAC_PI_4 * 3.0;
                    let a2 = ang - std::f32::consts::FRAC_PI_4 * 3.0;
                    let head_a = Point::new(p1.x + a1.cos() * head, p1.y + a1.sin() * head);
                    let head_b = Point::new(p1.x + a2.cos() * head, p1.y + a2.sin() * head);
                    frame.stroke(
                        &Path::line(p1, head_a),
                        Stroke::default().with_color(moved_color).with_width(1.4),
                    );
                    frame.stroke(
                        &Path::line(p1, head_b),
                        Stroke::default().with_color(moved_color).with_width(1.4),
                    );
                }
            }

            // Draw each pin as a small circle. Pin size is mostly
            // visual, ~3 px in canvas pixels.
            for pin in &self.pins {
                let p = tx(pin.pos[0], pin.pos[1]);
                let is_added = self.added.contains(&pin.key) && self.side == Side::Next;
                let is_removed = self.removed.contains(&pin.key) && self.side == Side::Prev;
                let is_moved = self.moved.iter().any(|(k, _, _)| k == &pin.key);

                let color = if is_added {
                    added_color
                } else if is_removed {
                    removed_color
                } else if is_moved {
                    moved_color
                } else {
                    neutral_color
                };

                // Outline-only when added/removed per spec.
                let r = 3.5_f32;
                if is_added || is_removed {
                    frame.stroke(
                        &Path::circle(p, r),
                        Stroke::default().with_color(color).with_width(1.6),
                    );
                } else {
                    frame.fill(&Path::circle(p, r), color);
                }

                // Strike-through for removed pins.
                if is_removed {
                    let strike_a = Point::new(p.x - r - 1.0, p.y);
                    let strike_b = Point::new(p.x + r + 1.0, p.y);
                    frame.stroke(
                        &Path::line(strike_a, strike_b),
                        Stroke::default().with_color(removed_color).with_width(1.4),
                    );
                }

                // Pin number label, right of the glyph.
                let label = pin.key.split(':').next_back().unwrap_or(&pin.key);
                if !label.is_empty() {
                    frame.fill_text(Text {
                        content: label.to_string(),
                        position: Point::new(p.x + 5.0, p.y - 5.0),
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

/// Walk the parsed S-expr tree and collect every `(pin … (number "N") … (at x y))`
/// node, keyed by `<unit>:<number>` to match the library crate's diff
/// format. Empty body or unparseable text returns an empty list.
pub fn extract_pins(sexpr_text: &str) -> Vec<Pin> {
    let trimmed = sexpr_text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let parsed = match sexpr::parse(trimmed) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    let mut stack: Vec<(&SExpr, u32)> = vec![(&parsed, 0)];
    while let Some((node, unit)) = stack.pop() {
        let next_unit = if node.keyword() == Some("symbol") {
            node.first_arg()
                .and_then(parse_symbol_unit_index)
                .unwrap_or(unit)
        } else {
            unit
        };
        if node.keyword() == Some("pin")
            && let Some(num) = pin_number(node)
        {
            let pos = at_position(node).unwrap_or([0.0, 0.0]);
            out.push(Pin {
                key: format!("{next_unit}:{num}"),
                pos,
            });
        }
        for child in node.children() {
            if matches!(child, SExpr::List(_)) {
                stack.push((child, next_unit));
            }
        }
    }
    out
}

/// `<root>_<unit>_<style>` → unit number. Mirrors the helper in
/// `signex-library/src/diff.rs`.
fn parse_symbol_unit_index(name: &str) -> Option<u32> {
    let unquoted = name.trim_matches('"');
    let mut parts = unquoted.rsplitn(3, '_');
    let _style = parts.next()?.parse::<u32>().ok()?;
    let unit = parts.next()?.parse::<u32>().ok()?;
    let _root = parts.next()?;
    Some(unit)
}

fn pin_number(node: &SExpr) -> Option<String> {
    node.find("number")
        .and_then(|n| n.first_arg())
        .map(|s| s.to_string())
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

    const TWO_PIN: &str = r#"
        (symbol "R0805"
            (pin passive line (at -2.54 0 0) (length 1.27) (name "1") (number "1"))
            (pin passive line (at  2.54 0 180) (length 1.27) (name "2") (number "2")))
    "#;

    #[test]
    fn extract_pins_returns_unit_keyed_pins() {
        let pins = extract_pins(TWO_PIN);
        assert_eq!(pins.len(), 2);
        assert!(pins.iter().any(|p| p.key == "0:1"));
        assert!(pins.iter().any(|p| p.key == "0:2"));
    }

    #[test]
    fn extract_pins_handles_empty_and_garbage() {
        assert!(extract_pins("").is_empty());
        assert!(extract_pins("   ").is_empty());
        assert!(extract_pins("not actually s-expr").is_empty());
    }

    #[test]
    fn extract_pins_keeps_multi_unit_distinct() {
        let dual = r#"
            (symbol "LM358"
                (symbol "LM358_1_1"
                    (pin input line (at -5 0 0) (length 2) (name "IN+") (number "1")))
                (symbol "LM358_2_1"
                    (pin input line (at -5 -10 0) (length 2) (name "IN+") (number "1"))))
        "#;
        let pins = extract_pins(dual);
        assert_eq!(pins.len(), 2);
        let keys: BTreeSet<String> = pins.iter().map(|p| p.key.clone()).collect();
        assert!(keys.contains("1:1"));
        assert!(keys.contains("2:1"));
    }
}
