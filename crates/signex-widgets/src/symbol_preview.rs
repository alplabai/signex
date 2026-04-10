//! Miniature schematic symbol preview canvas.
//!
//! Renders a LibSymbol's graphics + pins in a small preview box,
//! auto-fitted to the available size.

use iced::widget::canvas::{self, Cache, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Element, Length, Point, Rectangle, Size, Theme};
use signex_types::schematic::{Graphic, LibSymbol, Pin};

/// Canvas program that draws a LibSymbol preview.
pub struct SymbolPreview {
    symbol: LibSymbol,
    cache: Cache,
}

impl SymbolPreview {
    pub fn new(symbol: LibSymbol) -> Self {
        Self {
            symbol,
            cache: Cache::new(),
        }
    }

    /// Compute the bounding box of all graphics + pins.
    fn bounds(&self) -> (f64, f64, f64, f64) {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        let mut expand = |x: f64, y: f64| {
            if x < min_x { min_x = x; }
            if y < min_y { min_y = y; }
            if x > max_x { max_x = x; }
            if y > max_y { max_y = y; }
        };

        for g in &self.symbol.graphics {
            match g {
                Graphic::Rectangle { start, end, .. } => {
                    expand(start.x, start.y);
                    expand(end.x, end.y);
                }
                Graphic::Polyline { points, .. } => {
                    for p in points {
                        expand(p.x, p.y);
                    }
                }
                Graphic::Circle { center, radius, .. } => {
                    expand(center.x - radius, center.y - radius);
                    expand(center.x + radius, center.y + radius);
                }
                Graphic::Arc { start, mid, end, .. } => {
                    expand(start.x, start.y);
                    expand(mid.x, mid.y);
                    expand(end.x, end.y);
                }
                Graphic::Text { position, .. } => {
                    expand(position.x, position.y);
                }
                Graphic::TextBox { .. } => {}
            }
        }

        for pin in &self.symbol.pins {
            expand(pin.position.x, pin.position.y);
            // Pin extends by length in the pin's direction
            let (dx, dy) = match pin.rotation as i32 {
                0 => (pin.length, 0.0),
                90 => (0.0, -pin.length),
                180 => (-pin.length, 0.0),
                270 => (0.0, pin.length),
                _ => (pin.length, 0.0),
            };
            expand(pin.position.x + dx, pin.position.y + dy);
        }

        if min_x > max_x {
            // Empty symbol
            (0.0, 0.0, 10.0, 10.0)
        } else {
            // Add padding
            let pad = ((max_x - min_x).max(max_y - min_y)) * 0.1 + 2.0;
            (min_x - pad, min_y - pad, max_x + pad, max_y + pad)
        }
    }
}

impl canvas::Program<(), Theme> for SymbolPreview {
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

            // Scale to fit the frame
            let scale_x = bounds.width as f64 / bw;
            let scale_y = bounds.height as f64 / bh;
            let scale = scale_x.min(scale_y) * 0.9; // 90% fill

            let cx = bounds.width / 2.0;
            let cy = bounds.height / 2.0;
            let mid_x = (bx0 + bx1) / 2.0;
            let mid_y = (by0 + by1) / 2.0;

            // Transform: KiCad coords → frame coords
            // KiCad Y is inverted (positive down in KiCad schematic)
            let tx = |x: f64, y: f64| -> Point {
                Point::new(
                    cx + ((x - mid_x) * scale) as f32,
                    cy + ((y - mid_y) * scale) as f32,
                )
            };

            let body_color = Color::from_rgb(0.4, 0.65, 0.85);
            let fill_color = Color::from_rgb(0.15, 0.20, 0.28);
            let pin_color = Color::from_rgb(0.75, 0.75, 0.78);
            let text_color = Color::from_rgb(0.60, 0.60, 0.65);

            // Draw graphics
            for g in &self.symbol.graphics {
                match g {
                    Graphic::Rectangle { start, end, .. } => {
                        let p1 = tx(start.x, start.y);
                        let p2 = tx(end.x, end.y);
                        let top_left = Point::new(p1.x.min(p2.x), p1.y.min(p2.y));
                        let size = Size::new((p2.x - p1.x).abs(), (p2.y - p1.y).abs());

                        frame.fill_rectangle(top_left, size, fill_color);
                        frame.stroke(
                            &Path::rectangle(top_left, size),
                            Stroke::default().with_color(body_color).with_width(1.5),
                        );
                    }
                    Graphic::Polyline { points, .. } => {
                        if points.len() >= 2 {
                            let path = Path::new(|b| {
                                let p0 = tx(points[0].x, points[0].y);
                                b.move_to(p0);
                                for pt in &points[1..] {
                                    b.line_to(tx(pt.x, pt.y));
                                }
                            });
                            frame.stroke(
                                &path,
                                Stroke::default().with_color(body_color).with_width(1.5),
                            );
                        }
                    }
                    Graphic::Circle { center, radius, .. } => {
                        let c = tx(center.x, center.y);
                        let r = (*radius * scale) as f32;
                        frame.stroke(
                            &Path::circle(c, r),
                            Stroke::default().with_color(body_color).with_width(1.5),
                        );
                    }
                    Graphic::Arc { start, mid, end, .. } => {
                        // Approximate arc as polyline through 3 points
                        let p1 = tx(start.x, start.y);
                        let p2 = tx(mid.x, mid.y);
                        let p3 = tx(end.x, end.y);
                        let path = Path::new(|b| {
                            b.move_to(p1);
                            b.quadratic_curve_to(p2, p3);
                        });
                        frame.stroke(
                            &path,
                            Stroke::default().with_color(body_color).with_width(1.5),
                        );
                    }
                    Graphic::Text { .. } | Graphic::TextBox { .. } => {}
                }
            }

            // Draw pins
            for pin in &self.symbol.pins {
                let origin = tx(pin.position.x, pin.position.y);
                let (dx, dy) = match pin.rotation as i32 {
                    0 => (pin.length * scale, 0.0),
                    90 => (0.0, -pin.length * scale),
                    180 => (-pin.length * scale, 0.0),
                    270 => (0.0, pin.length * scale),
                    _ => (pin.length * scale, 0.0),
                };
                let tip = Point::new(
                    origin.x + dx as f32,
                    origin.y + dy as f32,
                );

                // Pin line
                frame.stroke(
                    &Path::line(origin, tip),
                    Stroke::default().with_color(pin_color).with_width(1.0),
                );

                // Pin number (small, near origin)
                if self.symbol.show_pin_numbers && !pin.number.is_empty() {
                    let font_sz = (6.0 * scale).clamp(5.0, 10.0) as f32;
                    frame.fill_text(Text {
                        content: pin.number.clone(),
                        position: Point::new(
                            (origin.x + tip.x) / 2.0,
                            (origin.y + tip.y) / 2.0 - font_sz,
                        ),
                        size: font_sz.into(),
                        color: text_color,
                        ..Text::default()
                    });
                }

                // Pin name (at the tip end)
                if self.symbol.show_pin_names && !pin.name.is_empty() && pin.name != "~" {
                    let font_sz = (6.0 * scale).clamp(5.0, 10.0) as f32;
                    let name_pos = match pin.rotation as i32 {
                        0 => Point::new(tip.x + 2.0, tip.y - font_sz / 2.0),
                        180 => Point::new(tip.x - 2.0, tip.y - font_sz / 2.0),
                        _ => Point::new(tip.x + 2.0, tip.y),
                    };
                    frame.fill_text(Text {
                        content: pin.name.clone(),
                        position: name_pos,
                        size: font_sz.into(),
                        color: text_color,
                        ..Text::default()
                    });
                }
            }
        });

        vec![geom]
    }
}

/// Create a symbol preview element.
pub fn symbol_preview(symbol: LibSymbol, height: f32) -> Element<'static, ()> {
    iced::widget::canvas(SymbolPreview::new(symbol))
        .width(Length::Fill)
        .height(height)
        .into()
}
