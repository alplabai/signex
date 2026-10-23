//! Miniature schematic symbol preview canvas.
//!
//! Renders a LibSymbol's graphics + pins in a small preview box,
//! auto-fitted to the available size.

use iced::widget::canvas::{self, Cache, Geometry, Path, Stroke, Text};
use iced::{Color, Element, Length, Point, Rectangle, Size, Theme};
use signex_types::schematic::{Graphic, LibSymbol, circumcircle};

/// Canvas program that draws a LibSymbol preview.
pub struct SymbolPreview {
    symbol: LibSymbol,
    /// Unit drawn; items tagged unit `0` are common to every unit.
    unit: u32,
    /// Body style drawn; items tagged body style `0` are common to all.
    body_style: u32,
    cache: Cache,
}

/// Segments used to flatten an arc or a cubic Bezier into a polyline.
const CURVE_SEGMENTS: usize = 32;

/// Whether an item tagged `(unit, body_style)` belongs to the shown
/// `(shown_unit, shown_body_style)`. Tag `0` means "common to all".
fn is_shown(unit: u32, body_style: u32, shown_unit: u32, shown_body_style: u32) -> bool {
    (unit == 0 || unit == shown_unit) && (body_style == 0 || body_style == shown_body_style)
}

/// Points along the circular arc that runs from `start` through `mid` to
/// `end`, in the input's own space. Collinear points have no circle, so the
/// three points are returned as a polyline.
fn arc_points(start: (f64, f64), mid: (f64, f64), end: (f64, f64)) -> Vec<(f64, f64)> {
    use std::f64::consts::TAU;

    let point = |(x, y): (f64, f64)| signex_types::schematic::Point { x, y };
    let Some((ox, oy, radius)) = circumcircle(point(start), point(mid), point(end)) else {
        return vec![start, mid, end];
    };
    let ((ax, ay), (bx, by), (cx, cy)) = (start, mid, end);

    let angle = |x: f64, y: f64| (y - oy).atan2(x - ox);
    let a_start = angle(ax, ay);
    let ccw = |to: f64| (to - a_start).rem_euclid(TAU);
    let to_mid = ccw(angle(bx, by));
    let to_end = ccw(angle(cx, cy));
    // `mid` lies on the arc: counter-clockwise when reached before `end`.
    let sweep = if to_mid <= to_end {
        to_end
    } else {
        to_end - TAU
    };

    (0..=CURVE_SEGMENTS)
        .map(|i| {
            let a = a_start + sweep * (i as f64 / CURVE_SEGMENTS as f64);
            (ox + radius * a.cos(), oy + radius * a.sin())
        })
        .collect()
}

/// Points along the cubic Bezier `start, cp1, cp2, end`. Any other control
/// point count is returned unchanged as a polyline.
fn bezier_points(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let [p0, p1, p2, p3] = points else {
        return points.to_vec();
    };
    (0..=CURVE_SEGMENTS)
        .map(|i| {
            let t = i as f64 / CURVE_SEGMENTS as f64;
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            (
                w[0] * p0.0 + w[1] * p1.0 + w[2] * p2.0 + w[3] * p3.0,
                w[0] * p0.1 + w[1] * p1.1 + w[2] * p2.1 + w[3] * p3.1,
            )
        })
        .collect()
}

/// Direction vector for a pin's stub in **library space** (Y-up): the pin
/// extends from its anchor position toward this unit vector, scaled by
/// `pin.length`. Rotation is in degrees.
///
/// This is the same library-space convention as `signex-output`'s
/// `pin_direction` (`crates/signex-output/src/svg/symbols.rs`) and
/// `signex-engine`'s autoplace pass (`transform/autoplace.rs`): 90°
/// points "up" (`+y`) in Y-up library space. [`library_to_screen`] then
/// applies the single y-flip that turns that "up" into a smaller
/// screen-space y, matching every other consumer of library coordinates.
fn pin_stub_direction(rotation: f64) -> (f64, f64) {
    match rotation as i32 {
        0 => (1.0, 0.0),
        90 => (0.0, 1.0),
        180 => (-1.0, 0.0),
        270 => (0.0, -1.0),
        _ => (1.0, 0.0),
    }
}

/// Map a library-space point (Y-up) to frame/screen space (Y-down),
/// centered and scaled to fit the preview box.
///
/// Mirrors the single y-flip in `signex_types::schematic::SymbolTransform
/// ::apply` / `signex-output`'s `symbol_world_point`: library Y grows
/// up, frame Y grows down, so the y term must be negated (about the
/// bounding-box midpoint) rather than passed through unchanged.
fn library_to_screen(x: f64, y: f64, mid_x: f64, mid_y: f64, scale: f64, center: Point) -> Point {
    Point::new(
        center.x + ((x - mid_x) * scale) as f32,
        center.y + ((mid_y - y) * scale) as f32,
    )
}

impl SymbolPreview {
    pub fn new(symbol: LibSymbol) -> Self {
        Self {
            symbol,
            unit: 1,
            body_style: 1,
            cache: Cache::new(),
        }
    }

    /// Draw `unit` / `body_style` instead of the default unit 1, body style 1.
    pub fn with_unit(mut self, unit: u32, body_style: u32) -> Self {
        self.unit = unit;
        self.body_style = body_style;
        self
    }

    fn shows(&self, unit: u32, body_style: u32) -> bool {
        is_shown(unit, body_style, self.unit, self.body_style)
    }

    /// Compute the bounding box of all graphics + pins.
    fn bounds(&self) -> (f64, f64, f64, f64) {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        let mut expand = |x: f64, y: f64| {
            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
        };

        for g in self
            .symbol
            .graphics
            .iter()
            .filter(|g| self.shows(g.unit, g.body_style))
        {
            match &g.graphic {
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
                Graphic::Arc {
                    start, mid, end, ..
                } => {
                    for (x, y) in arc_points((start.x, start.y), (mid.x, mid.y), (end.x, end.y)) {
                        expand(x, y);
                    }
                }
                Graphic::Text { position, .. } => {
                    expand(position.x, position.y);
                }
                Graphic::Bezier { points, .. } => {
                    let pts: Vec<(f64, f64)> = points.iter().map(|p| (p.x, p.y)).collect();
                    for (x, y) in bezier_points(&pts) {
                        expand(x, y);
                    }
                }
                Graphic::TextBox { .. } => {}
            }
        }

        for lib_pin in self
            .symbol
            .pins
            .iter()
            .filter(|p| self.shows(p.unit, p.body_style))
        {
            let pin = &lib_pin.pin;
            expand(pin.position.x, pin.position.y);
            // Pin extends by length in the pin's direction, in the same
            // library-space (Y-up) convention as everywhere else in this
            // function -- no screen-space flip here, that happens once in
            // `library_to_screen`.
            let (dx, dy) = pin_stub_direction(pin.rotation);
            expand(
                pin.position.x + dx * pin.length,
                pin.position.y + dy * pin.length,
            );
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

            // Transform: library coords (Y-up) → frame coords (Y-down).
            // Library space is Y-up (a pin at `(0, +len)` points up);
            // frame/canvas space is Y-down. See `library_to_screen`.
            let center = Point::new(cx, cy);
            let tx =
                |x: f64, y: f64| -> Point { library_to_screen(x, y, mid_x, mid_y, scale, center) };

            let body_color = Color::from_rgb(0.4, 0.65, 0.85);
            let fill_color = Color::from_rgb(0.15, 0.20, 0.28);
            let pin_color = Color::from_rgb(0.75, 0.75, 0.78);
            let text_color = Color::from_rgb(0.60, 0.60, 0.65);

            // Draw graphics
            for g in self
                .symbol
                .graphics
                .iter()
                .filter(|g| self.shows(g.unit, g.body_style))
            {
                match &g.graphic {
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
                    Graphic::Arc {
                        start, mid, end, ..
                    } => {
                        let pts = arc_points((start.x, start.y), (mid.x, mid.y), (end.x, end.y));
                        stroke_polyline(frame, &pts, &tx, body_color);
                    }
                    Graphic::Bezier { points, .. } => {
                        let ctrl: Vec<(f64, f64)> = points.iter().map(|p| (p.x, p.y)).collect();
                        stroke_polyline(frame, &bezier_points(&ctrl), &tx, body_color);
                    }
                    Graphic::Text { .. } | Graphic::TextBox { .. } => {}
                }
            }

            // Draw pins
            for lib_pin in self
                .symbol
                .pins
                .iter()
                .filter(|p| self.shows(p.unit, p.body_style))
            {
                let pin = &lib_pin.pin;
                let origin = tx(pin.position.x, pin.position.y);
                // Compute the tip in library space, then run it through the
                // same `tx` as everything else -- this keeps the single
                // y-flip in one place instead of re-deriving a screen-space
                // sign here.
                let (dx, dy) = pin_stub_direction(pin.rotation);
                let tip = tx(
                    pin.position.x + dx * pin.length,
                    pin.position.y + dy * pin.length,
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

/// Stroke `pts` (library space) as one open polyline in the body colour.
fn stroke_polyline(
    frame: &mut canvas::Frame,
    pts: &[(f64, f64)],
    tx: &impl Fn(f64, f64) -> Point,
    color: Color,
) {
    let Some(((x0, y0), rest)) = pts.split_first() else {
        return;
    };
    if rest.is_empty() {
        return;
    }
    let path = Path::new(|b| {
        b.move_to(tx(*x0, *y0));
        for (x, y) in rest {
            b.line_to(tx(*x, *y));
        }
    });
    frame.stroke(&path, Stroke::default().with_color(color).with_width(1.5));
}

/// Create a symbol preview element.
pub fn symbol_preview(symbol: LibSymbol, height: f32) -> Element<'static, ()> {
    iced::widget::canvas(SymbolPreview::new(symbol))
        .width(Length::Fill)
        .height(height)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{
        LibGraphic, LibPin, Pin, PinDirection, PinShapeStyle, Point as LibPoint,
    };

    fn up_down_pin(rotation: f64) -> Pin {
        Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: LibPoint::ZERO,
            rotation,
            length: 3.0,
            name: String::new(),
            number: "1".to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        }
    }

    fn symbol_with_pin(rotation: f64) -> LibSymbol {
        LibSymbol {
            id: "test".to_string(),
            reference: String::new(),
            value: String::new(),
            footprint: String::new(),
            datasheet: String::new(),
            description: String::new(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: Vec::new(),
            pins: vec![LibPin {
                unit: 0,
                body_style: 1,
                pin: up_down_pin(rotation),
            }],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        }
    }

    #[test]
    fn pin_stub_direction_matches_canonical_library_space_convention() {
        // Must match `signex-output`'s `pin_direction` (svg/symbols.rs)
        // and `signex-engine`'s autoplace pass -- 90 deg is "up" (+y) in
        // library space, 270 deg is "down" (-y).
        assert_eq!(pin_stub_direction(0.0), (1.0, 0.0));
        assert_eq!(pin_stub_direction(90.0), (0.0, 1.0));
        assert_eq!(pin_stub_direction(180.0), (-1.0, 0.0));
        assert_eq!(pin_stub_direction(270.0), (0.0, -1.0));
    }

    #[test]
    fn library_to_screen_flips_y_about_the_midpoint() {
        let center = Point::new(50.0, 50.0);
        // A point "above" the midpoint in library space (larger y) must
        // land at a *smaller* screen y (canvas y grows downward) --
        // this is the flip issue #495 says `tx` was missing.
        let above = library_to_screen(0.0, 5.0, 0.0, 0.0, 10.0, center);
        let below = library_to_screen(0.0, -5.0, 0.0, 0.0, 10.0, center);
        assert!(
            above.y < center.y,
            "library +y must map above screen center"
        );
        assert!(
            below.y > center.y,
            "library -y must map below screen center"
        );
        assert!(above.y < below.y);
    }

    /// Regression test for issue #495: an Up (90 deg) pin's on-screen tip
    /// must land above (smaller y) its anchor, and a Down (270 deg) pin's
    /// tip must land below (larger y) its anchor -- not mirrored.
    ///
    /// This exercises the exact same pipeline `draw()` uses (`bounds()`
    /// to derive `mid_x`/`mid_y`/`scale`, then `pin_stub_direction` +
    /// `library_to_screen` for the pin endpoints) without needing an
    /// `iced::Renderer`.
    #[test]
    fn up_pin_screen_tip_is_above_anchor_down_pin_is_below() {
        let up_preview = SymbolPreview::new(symbol_with_pin(90.0));
        let (bx0, by0, bx1, by1) = up_preview.bounds();
        let mid_x = (bx0 + bx1) / 2.0;
        let mid_y = (by0 + by1) / 2.0;
        let scale = 10.0;
        let center = Point::new(100.0, 100.0);

        let pin = &up_preview.symbol.pins[0].pin;
        let origin = library_to_screen(pin.position.x, pin.position.y, mid_x, mid_y, scale, center);
        let (dx, dy) = pin_stub_direction(pin.rotation);
        let tip = library_to_screen(
            pin.position.x + dx * pin.length,
            pin.position.y + dy * pin.length,
            mid_x,
            mid_y,
            scale,
            center,
        );
        assert!(
            tip.y < origin.y,
            "an Up pin must render its stub going up (smaller screen y): origin={origin:?} tip={tip:?}"
        );

        let down_preview = SymbolPreview::new(symbol_with_pin(270.0));
        let (bx0, by0, bx1, by1) = down_preview.bounds();
        let mid_x = (bx0 + bx1) / 2.0;
        let mid_y = (by0 + by1) / 2.0;

        let pin = &down_preview.symbol.pins[0].pin;
        let origin = library_to_screen(pin.position.x, pin.position.y, mid_x, mid_y, scale, center);
        let (dx, dy) = pin_stub_direction(pin.rotation);
        let tip = library_to_screen(
            pin.position.x + dx * pin.length,
            pin.position.y + dy * pin.length,
            mid_x,
            mid_y,
            scale,
            center,
        );
        assert!(
            tip.y > origin.y,
            "a Down pin must render its stub going down (larger screen y): origin={origin:?} tip={tip:?}"
        );
    }

    /// Regression test for the `bounds()` half of #495: the pin-tip
    /// expansion must stay in the same library-space (Y-up) convention as
    /// the rest of `bounds()`, so an Up pin grows the box toward +y, not
    /// -y (the box's vertical midpoint tells them apart).
    #[test]
    fn bounds_grows_toward_positive_y_for_an_up_pin() {
        let preview = SymbolPreview::new(symbol_with_pin(90.0));
        let (_, min_y, _, max_y) = preview.bounds();
        let mid_y = (min_y + max_y) / 2.0;
        assert!(
            mid_y > 0.0,
            "Up pin's tip must pull the bbox toward +y in library space, got mid_y={mid_y}"
        );
    }

    fn near(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn semicircle_arc_peaks_at_the_mid_point_not_halfway() {
        let pts = arc_points((-1.0, 0.0), (0.0, 1.0), (1.0, 0.0));
        assert!(near(pts[0], (-1.0, 0.0)));
        assert!(near(*pts.last().unwrap(), (1.0, 0.0)));
        let peak = pts.iter().map(|p| p.1).fold(f64::MIN, f64::max);
        assert!((peak - 1.0).abs() < 1e-9, "apex must reach y=1, got {peak}");
        assert!(
            pts.iter().all(|p| (p.0.hypot(p.1) - 1.0).abs() < 1e-9),
            "every sample must sit on the unit circle"
        );
    }

    #[test]
    fn clockwise_arc_goes_through_mid_the_other_way() {
        // start (1,0) -> mid (0,-1) -> end (-1,0): lower semicircle.
        let pts = arc_points((1.0, 0.0), (0.0, -1.0), (-1.0, 0.0));
        let trough = pts.iter().map(|p| p.1).fold(f64::MAX, f64::min);
        assert!((trough + 1.0).abs() < 1e-9, "got {trough}");
        assert!(pts.iter().all(|p| p.1 <= 1e-9), "must stay below the axis");
    }

    #[test]
    fn collinear_arc_falls_back_to_a_polyline() {
        let pts = arc_points((0.0, 0.0), (1.0, 0.0), (2.0, 0.0));
        assert_eq!(pts, vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]);
    }

    #[test]
    fn bezier_is_flattened_through_a_curve_not_its_control_polygon() {
        let ctrl = [(0.0, 0.0), (0.0, 3.0), (3.0, 3.0), (3.0, 0.0)];
        let pts = bezier_points(&ctrl);
        assert!(pts.len() > ctrl.len());
        assert!(near(pts[0], ctrl[0]));
        assert!(near(*pts.last().unwrap(), ctrl[3]));
        // The curve's apex is 0.75 * 3, well below the control point at y=3.
        let peak = pts.iter().map(|p| p.1).fold(f64::MIN, f64::max);
        assert!((peak - 2.25).abs() < 1e-9, "got {peak}");
    }

    #[test]
    fn item_filter_keeps_the_shown_unit_and_common_items_only() {
        assert!(is_shown(1, 1, 1, 1));
        assert!(!is_shown(2, 1, 1, 1), "another unit's item is hidden");
        assert!(!is_shown(1, 2, 1, 1), "another body style is hidden");
        assert!(is_shown(0, 1, 2, 1), "unit 0 is common to every unit");
        assert!(is_shown(1, 0, 1, 2), "body style 0 is common to all");
    }

    #[test]
    fn bounds_ignore_pins_and_graphics_of_other_units() {
        let mut symbol = symbol_with_pin(0.0);
        symbol.pins.push(LibPin {
            unit: 2,
            body_style: 1,
            pin: Pin {
                position: LibPoint { x: 500.0, y: 500.0 },
                ..up_down_pin(0.0)
            },
        });
        symbol.graphics.push(LibGraphic {
            unit: 2,
            body_style: 1,
            graphic: Graphic::Circle {
                center: LibPoint {
                    x: -500.0,
                    y: -500.0,
                },
                radius: 10.0,
                width: 0.0,
                fill: Default::default(),
            },
        });
        let unit1 = SymbolPreview::new(symbol.clone()).bounds();
        assert!(unit1.2 < 100.0 && unit1.0 > -100.0, "unit 1 box: {unit1:?}");
        let unit2 = SymbolPreview::new(symbol).with_unit(2, 1).bounds();
        assert!(unit2.2 > 400.0 && unit2.0 < -400.0, "unit 2 box: {unit2:?}");
    }
}
