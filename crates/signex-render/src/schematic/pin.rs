//! Pin rendering -- draws pin lines and optional name/number labels.
//!
//! A pin is defined in library-local coordinates. We apply the same
//! instance transform as for symbol graphics, then draw:
//! 1. A line from the pin position extending outward by `pin.length`.
//! 2. The pin name (outside the body) if `show_pin_names` is true.
//! 3. The pin number (inside the body) if `show_pin_numbers` is true.

use iced::Color;
use iced::widget::canvas::{self, LineCap, LineJoin, path};
use std::collections::HashMap;

use signex_types::schematic::{LibSymbol, Pin, PinShape, Point, Symbol};

use super::ScreenTransform;
use super::text::{display_text_content, display_text_with_overbars};

// ---------------------------------------------------------------------------
// Instance transform (duplicated for self-containment -- could be shared)
// ---------------------------------------------------------------------------

/// Delegate to the shared instance_transform in mod.rs.
fn instance_transform(sym: &Symbol, local: &Point) -> (f64, f64) {
    super::instance_transform(sym, local)
}

/// Apply instance rotation + mirror to a direction vector (no translation).
fn instance_rotate_dir(sym: &Symbol, dx: f64, dy: f64) -> (f64, f64) {
    let x = dx;
    let y = -dy; // lib Y-up → schematic Y-down

    let rad = -sym.rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let rx = x * cos - y * sin;
    let ry = x * sin + y * cos;

    let rx = if sym.mirror_y { -rx } else { rx };
    let ry = if sym.mirror_x { -ry } else { ry };

    (rx, ry)
}

// ---------------------------------------------------------------------------
// Pin direction from rotation
// ---------------------------------------------------------------------------

/// Returns the unit direction vector a pin extends from its position
/// based on the pin's local rotation (0=right, 90=up, 180=left, 270=down).
/// Returns the unit direction vector a pin extends from its connection-point
/// toward the symbol body, in lib-local Y-UP coordinates.
/// 0=right, 90=up-in-lib, 180=left, 270=down-in-lib.
/// Results are passed through instance_transform which applies Y-flip + rotation.
fn pin_direction(pin: &Pin) -> (f64, f64) {
    let deg = ((pin.rotation % 360.0) + 360.0) % 360.0;
    match deg as i32 {
        0 => (1.0, 0.0),
        90 => (0.0, 1.0), // up in lib Y-up space
        180 => (-1.0, 0.0),
        270 => (0.0, -1.0), // down in lib Y-up space
        _ => {
            let rad = deg.to_radians();
            (rad.cos(), rad.sin()) // lib Y-up: positive sin = upward
        }
    }
}

// ---------------------------------------------------------------------------
// Draw all pins for a symbol
// ---------------------------------------------------------------------------

/// Draw all pins of a library symbol at the instance's position,
/// filtering to only the matching unit and normal body style.
pub fn draw_symbol_pins(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    lib: &LibSymbol,
    transform: &ScreenTransform,
    pin_color: Color,
) {
    let visible_pins: Vec<&Pin> = lib
        .pins
        .iter()
        .filter(|lp| (lp.unit == 0 || lp.unit == sym.unit) && lp.pin.visible)
        .filter(|lp| lp.body_style == 0 || lp.body_style == 1)
        .map(|lp| &lp.pin)
        .collect();

    let mut stack_groups: HashMap<(u64, u64, u64), Vec<usize>> = HashMap::new();
    for (index, pin) in visible_pins.iter().enumerate() {
        stack_groups.entry(stack_key(pin)).or_default().push(index);
    }

    for (visible_index, pin) in visible_pins.iter().enumerate() {
        let stack = stack_groups.get(&stack_key(pin));
        let stack_total = stack.map(|pins| pins.len()).unwrap_or(1);
        let stack_index = stack
            .and_then(|pins| pins.iter().position(|idx| *idx == visible_index))
            .unwrap_or(0);
        draw_pin(
            frame,
            sym,
            lib,
            pin,
            transform,
            pin_color,
            StackPlacement {
                index: stack_index,
                total: stack_total,
            },
        );
    }
}

fn stack_key(pin: &Pin) -> (u64, u64, u64) {
    (
        pin.position.x.to_bits(),
        pin.position.y.to_bits(),
        pin.rotation.to_bits(),
    )
}

#[derive(Clone, Copy)]
struct StackPlacement {
    index: usize,
    total: usize,
}

/// Draw a single pin: line + optional name + optional number.
fn draw_pin(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    lib: &LibSymbol,
    pin: &Pin,
    transform: &ScreenTransform,
    pin_color: Color,
    stack: StackPlacement,
) {
    if !pin.visible {
        return;
    }

    // Pin position is the connection point (the end the wire connects to).
    // The pin line extends from the position toward the symbol body.
    let (dir_x, dir_y) = pin_direction(pin);
    let len = pin.length;

    // Endpoint = position (connection end)
    // Body end = position + direction * length (inside the symbol body)
    let body_end = Point::new(pin.position.x + dir_x * len, pin.position.y + dir_y * len);

    // Transform to world coordinates
    let (wx1, wy1) = instance_transform(sym, &pin.position);
    let (wx2, wy2) = instance_transform(sym, &body_end);

    let p1 = transform.to_screen_point(wx1, wy1);
    let p2 = transform.to_screen_point(wx2, wy2);

    let stroke_width = transform.world_len(0.15).max(0.5);
    let stroke = canvas::Stroke {
        line_cap: LineCap::Square,
        line_join: LineJoin::Miter,
        ..canvas::Stroke::default()
            .with_color(pin_color)
            .with_width(stroke_width)
    };

    // INVERTED / INVERTED_CLOCK draw their own (shortened) line inside draw_pin_shape.
    // All other shapes get the full pin line drawn here.
    let shape_draws_own_line = matches!(pin.shape, PinShape::Inverted | PinShape::InvertedClock);
    if !shape_draws_own_line {
        let path = canvas::Path::new(|b: &mut path::Builder| {
            b.move_to(p1);
            b.line_to(p2);
        });
        frame.stroke(&path, stroke);
    }

    // Draw shape decorator at the connection end (p1).
    draw_pin_shape(frame, p1, p2, pin.shape, stroke_width, transform, pin_color);

    // Small circle at the connection point (endpoint) — removed, KiCad doesn't draw this

    // Pin name is drawn near the symbol-side end of the pin.
    let font_size_mm = crate::SCHEMATIC_TEXT_MM;
    let screen_font = transform.world_len(font_size_mm).abs();

    if screen_font >= 1.0
        && lib.show_pin_names
        && pin.name_visible
        && !pin.name.is_empty()
        && pin.name != "~"
    {
        // KiCad pin-name placement has two modes keyed on `pin_name_offset`:
        //
        // * offset > 0  — name along the pin, INSIDE body, at
        //   `body_end + dir * offset`. Used by Device:R, Device:C, ICs etc.
        //   (typical 0.254–0.508 mm).
        //
        // * offset == 0 — name PERPENDICULAR to the pin at the tip, OUTSIDE
        //   body. Common on discrete-transistor symbols (NMOS/PMOS G/D/S).
        //   KiCad renders this with a small perpendicular gap so the
        //   character sits beside the pin line, not on top of it.
        let (np, h_align, v_align);
        if lib.pin_name_offset.abs() < 0.01 {
            // offset = 0: anchor at pin tip, text sits perpendicular
            // above/beside the pin.
            let perp_gap = 0.508; // KiCad default visual gap in mm
            // World-space pin direction (body → tip) after instance transform.
            let (wdx, wdy) = instance_rotate_dir(sym, -dir_x, -dir_y);
            let (nwx, nwy) = instance_transform(sym, &pin.position);
            if wdx.abs() > wdy.abs() {
                // Horizontal pin on screen: name above the pin, centered on tip.
                let gap_px = transform.world_len(perp_gap).abs();
                np = iced::Point::new(
                    transform.to_screen_point(nwx, nwy).x,
                    transform.to_screen_point(nwx, nwy).y - gap_px,
                );
                h_align = iced::alignment::Horizontal::Center;
                v_align = iced::alignment::Vertical::Bottom;
            } else {
                // Vertical pin on screen: name beside the pin tip.
                // Put it on the RIGHT of the tip, vertically centered.
                let gap_px = transform.world_len(perp_gap).abs();
                np = iced::Point::new(
                    transform.to_screen_point(nwx, nwy).x + gap_px,
                    transform.to_screen_point(nwx, nwy).y,
                );
                h_align = iced::alignment::Horizontal::Left;
                v_align = iced::alignment::Vertical::Center;
            }
        } else {
            // offset > 0: place name along the pin, just inside the body.
            // Anchor sits at the body-side end of the pin shifted inward by
            // `pin_name_offset`. The text then extends further inward from
            // the anchor — so the alignment edge ("near" edge in screen)
            // must face the pin tip. Otherwise the text's half-height would
            // overrun the body border for vertical pins (VCC/GND on ICs).
            let name_offset = lib.pin_name_offset;
            let name_pos = Point::new(
                body_end.x + dir_x * name_offset,
                body_end.y + dir_y * name_offset,
            );
            let (nwx, nwy) = instance_transform(sym, &name_pos);
            np = transform.to_screen_point(nwx, nwy);
            // Screen-space pin direction (tip → body), which is the
            // direction text should extend in from its anchor.
            let (wdx, wdy) = instance_rotate_dir(sym, dir_x, dir_y);
            if wdx.abs() > wdy.abs() {
                // Horizontal pin on screen.
                h_align = if wdx > 0.0 {
                    iced::alignment::Horizontal::Left
                } else {
                    iced::alignment::Horizontal::Right
                };
                v_align = iced::alignment::Vertical::Center;
            } else {
                // Vertical pin on screen. wdy is in screen Y-down after
                // instance_rotate_dir (which returns lib-flipped Y). So
                // wdy > 0 means the pin extends DOWN on screen (tip above)
                // and the text must extend further down → align_y Top.
                h_align = iced::alignment::Horizontal::Center;
                v_align = if wdy > 0.0 {
                    iced::alignment::Vertical::Top
                } else {
                    iced::alignment::Vertical::Bottom
                };
            }
        }

        // Render plain glyphs (no combining overline chars — those sit
        // flush against the cap-height). Any overbar segments are drawn as
        // a separate stroke above the text with a small visible gap, which
        // matches KiCad's look.
        let (plain, overbars) = display_text_with_overbars(&pin.name);
        let text = canvas::Text {
            content: plain.clone(),
            position: np,
            color: pin_color,
            size: iced::Pixels(screen_font),
            font: crate::canvas_font(),
            align_x: h_align.into(),
            align_y: v_align,
            ..canvas::Text::default()
        };
        frame.fill_text(text);

        if !overbars.is_empty() {
            draw_overbars(
                frame,
                &plain,
                &overbars,
                np,
                screen_font,
                h_align,
                v_align,
                pin_color,
            );
        }
    }

    // Pin number (inside the body, along the pin line)
    if screen_font >= 1.0 && lib.show_pin_numbers && pin.number_visible && !pin.number.is_empty() {
        // Number is placed at the midpoint of the pin line.
        let mid = Point::new(
            pin.position.x + dir_x * len * 0.5,
            pin.position.y + dir_y * len * 0.5,
        );
        let (mwx, mwy) = instance_transform(sym, &mid);
        let np_base = transform.to_screen_point(mwx, mwy);

        // Compute world-space pin direction (screen Y-down system).
        let (wdx, wdy) = instance_rotate_dir(sym, dir_x, dir_y);
        let perp_offset_px = transform.world_len(0.8);

        // Always offset toward screen-up for horizontal pins, screen-left for
        // vertical pins.  This keeps numbers above (never below) the pin line
        // regardless of lib-local rotation (0° vs 180° pins agree).
        let (perp_sx, perp_sy, num_align) = if wdx.abs() >= wdy.abs() {
            // Horizontal pin → above line (screen -Y), centered on X.
            (0.0_f32, -1.0_f32, iced::alignment::Horizontal::Center)
        } else {
            // Vertical pin → left of line (screen -X), right-aligned text.
            (-1.0_f32, 0.0_f32, iced::alignment::Horizontal::Right)
        };

        let np = iced::Point::new(
            np_base.x + perp_sx * perp_offset_px,
            np_base.y + perp_sy * perp_offset_px,
        );

        let small_font = (screen_font * 0.8).abs();
        if small_font < 1.0 {
            return;
        }
        let fanout_step_px = transform.world_len(0.9).max(small_font * 0.8);
        let stack_center = stack.index as f32 - (stack.total as f32 - 1.0) * 0.5;
        let line_dx = p2.x - p1.x;
        let line_dy = p2.y - p1.y;
        let line_len = (line_dx * line_dx + line_dy * line_dy).sqrt().max(0.001);
        let stack_np = iced::Point::new(
            np.x + (line_dx / line_len) * fanout_step_px * stack_center,
            np.y + (line_dy / line_len) * fanout_step_px * stack_center,
        );
        let text = canvas::Text {
            content: display_text_content(&pin.number),
            position: stack_np,
            color: pin_color,
            size: iced::Pixels(small_font),
            font: crate::canvas_font(),
            align_x: num_align.into(),
            align_y: iced::alignment::Vertical::Center,
            ..canvas::Text::default()
        };
        frame.fill_text(text);
    }
}

// ---------------------------------------------------------------------------
// Pin shape decorators (mirroring KiCad SCH_PAINTER pin shape logic)
// ---------------------------------------------------------------------------

/// Draw two connected segments A→B and B→C (KiCad `triLine`).
fn tri_line(
    frame: &mut canvas::Frame,
    a: iced::Point,
    b: iced::Point,
    c: iced::Point,
    stroke: canvas::Stroke,
) {
    let path = canvas::Path::new(|p| {
        p.move_to(a);
        p.line_to(b);
        p.move_to(b);
        p.line_to(c);
    });
    frame.stroke(&path, stroke);
}

/// Draw the pin shape decorator.
///
/// Coordinate mapping vs KiCad `SCH_PAINTER`:
/// * `p1` = connection end (wire attaches here) = KiCad `pos = GetPosition()`
/// * `p2` = body end (at symbol body boundary)  = KiCad `p0 = GetPinRoot()`
/// * `bdx/bdy` = direction FROM body TOWARD connection = KiCad `dir`
///
/// All KiCad decorators are anchored at `p0` (body end) — so we anchor at
/// `p2`.  For `Inverted` and `InvertedClock`, this function also draws the
/// shortened pin line; `draw_pin` skips the full line for those two shapes.
fn draw_pin_shape(
    frame: &mut canvas::Frame,
    p1: iced::Point,
    p2: iced::Point,
    shape: PinShape,
    stroke_width: f32,
    transform: &ScreenTransform,
    color: Color,
) {
    if matches!(shape, PinShape::Line) {
        return;
    }

    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;
    let len_px = (dx * dx + dy * dy).sqrt();
    if len_px < 0.1 {
        return;
    }

    // Unit direction from p1 (connection) toward p2 (body).
    let sdx = dx / len_px;
    let sdy = dy / len_px;

    // KiCad "dir": from body (p2) toward connection (p1).
    let bdx = -sdx;
    let bdy = -sdy;

    // Wing (perpendicular) direction for clock triangles.
    // Horizontal pin: |bdx|≥|bdy| → wing = (0, 1).
    // Vertical pin:   |bdy|>|bdx| → wing = (1, 0).
    let wing_x = bdy.abs(); // = |sdy|
    let wing_y = bdx.abs(); // = |sdx|

    // Forward / low helpers for InputLow, ClockLow, OutputLow.
    // KiCad uses absolute screen-space -Y (up) for horizontal, -X (left) for vertical.
    let is_horiz = bdx.abs() >= bdy.abs();
    let (fwd_x, fwd_y) = if is_horiz {
        (bdx, 0.0_f32)
    } else {
        (0.0_f32, bdy)
    };

    // Decorator sizes in screen pixels.
    let radius = transform.world_len(0.508).max(2.0);
    let diam = radius * 2.0;
    let clock_size = transform.world_len(0.762).max(2.5);

    let stroke = canvas::Stroke::default()
        .with_color(color)
        .with_width(stroke_width);

    let pt = |x: f32, y: f32| iced::Point::new(x, y);

    match shape {
        PinShape::Line => unreachable!(),

        // ── INVERTED ─────────────────────────────────────────────────────────
        // Open circle just outside body (toward connection), then shortened line.
        // KiCad: DrawCircle(p0 + dir*radius, radius); DrawLine(p0+dir*diam, pos)
        PinShape::Inverted => {
            let cx = p2.x + bdx * radius;
            let cy = p2.y + bdy * radius;
            frame.stroke(&canvas::Path::circle(pt(cx, cy), radius), stroke);
            let path = canvas::Path::new(|b| {
                b.move_to(pt(p2.x + bdx * diam, p2.y + bdy * diam));
                b.line_to(p1);
            });
            frame.stroke(&path, stroke);
        }

        // ── CLOCK ────────────────────────────────────────────────────────────
        // Triangle at body end: ±perp wings, apex INTO the body.
        // KiCad: triLine(p0±perp*cs, p0 - dir*cs); full line drawn by caller.
        PinShape::Clock => {
            tri_line(
                frame,
                pt(p2.x + wing_x * clock_size, p2.y + wing_y * clock_size),
                pt(p2.x + sdx * clock_size, p2.y + sdy * clock_size), // INTO body
                pt(p2.x - wing_x * clock_size, p2.y - wing_y * clock_size),
                stroke,
            );
        }

        // ── INVERTED_CLOCK ───────────────────────────────────────────────────
        // Clock triangle at body end + Inverted circle + shortened line.
        PinShape::InvertedClock => {
            tri_line(
                frame,
                pt(p2.x + wing_x * clock_size, p2.y + wing_y * clock_size),
                pt(p2.x + sdx * clock_size, p2.y + sdy * clock_size),
                pt(p2.x - wing_x * clock_size, p2.y - wing_y * clock_size),
                stroke,
            );
            let cx = p2.x + bdx * radius;
            let cy = p2.y + bdy * radius;
            frame.stroke(&canvas::Path::circle(pt(cx, cy), radius), stroke);
            let path = canvas::Path::new(|b| {
                b.move_to(pt(p2.x + bdx * diam, p2.y + bdy * diam));
                b.line_to(p1);
            });
            frame.stroke(&path, stroke);
        }

        // ── INPUT_LOW ────────────────────────────────────────────────────────
        // IEEE active-low input: L-shape anchored at body end.
        // KiCad horiz: triLine(p0+(dir.x,0)*d, p0+(dir.x,-1)*d, p0)
        // KiCad vert:  triLine(p0+(0,dir.y)*d, p0+(-1,dir.y)*d, p0)
        PinShape::InputLow => {
            let ax = p2.x + fwd_x * diam;
            let ay = p2.y + fwd_y * diam;
            let (bx, by) = if is_horiz {
                (ax, ay - diam) // -Y = up on screen
            } else {
                (ax - diam, ay) // -X = left on screen
            };
            tri_line(frame, pt(ax, ay), pt(bx, by), p2, stroke);
        }

        // ── CLOCK_LOW / EDGE_CLOCK_HIGH ──────────────────────────────────────
        // Clock triangle + InputLow L-shape (KiCad treats these identically).
        PinShape::ClockLow | PinShape::EdgeClockHigh => {
            tri_line(
                frame,
                pt(p2.x + wing_x * clock_size, p2.y + wing_y * clock_size),
                pt(p2.x + sdx * clock_size, p2.y + sdy * clock_size),
                pt(p2.x - wing_x * clock_size, p2.y - wing_y * clock_size),
                stroke,
            );
            let ax = p2.x + fwd_x * diam;
            let ay = p2.y + fwd_y * diam;
            let (bx, by) = if is_horiz {
                (ax, ay - diam)
            } else {
                (ax - diam, ay)
            };
            tri_line(frame, pt(ax, ay), pt(bx, by), p2, stroke);
        }

        // ── OUTPUT_LOW ───────────────────────────────────────────────────────
        // IEEE active-low output: diagonal "flag" line.
        // KiCad horiz: line(p0 - (0,diam), p0 + dir.x*diam)
        // KiCad vert:  line(p0 - (diam,0), p0 + dir.y*diam)
        PinShape::OutputLow => {
            let (start, end_pt) = if is_horiz {
                (pt(p2.x, p2.y - diam), pt(p2.x + bdx * diam, p2.y))
            } else {
                (pt(p2.x - diam, p2.y), pt(p2.x, p2.y + bdy * diam))
            };
            let path = canvas::Path::new(|b| {
                b.move_to(start);
                b.line_to(end_pt);
            });
            frame.stroke(&path, stroke);
        }

        // ── NON_LOGIC ────────────────────────────────────────────────────────
        // X cross at body end using two diagonal lines.
        // KiCad: d1=(dir.x+dir.y, dir.y-dir.x), d2=(dir.x-dir.y, dir.x+dir.y)
        PinShape::NonLogic => {
            let d1x = bdx + bdy;
            let d1y = bdy - bdx;
            let d2x = bdx - bdy;
            let d2y = bdx + bdy;
            let path = canvas::Path::new(|b| {
                b.move_to(pt(p2.x - d1x * radius, p2.y - d1y * radius));
                b.line_to(pt(p2.x + d1x * radius, p2.y + d1y * radius));
                b.move_to(pt(p2.x - d2x * radius, p2.y - d2y * radius));
                b.line_to(pt(p2.x + d2x * radius, p2.y + d2y * radius));
            });
            frame.stroke(&path, stroke);
        }
    }
}

// ---------------------------------------------------------------------------
// Overbar rendering (horizontal text only)
// ---------------------------------------------------------------------------

/// Draw horizontal overbar strokes above the character ranges specified in
/// `overbars` (`(start_char_idx, char_count)` pairs into `plain`). Assumes
/// the backing text was rendered horizontally (0° rotation) — which is the
/// case for all pin names today. Placement is Iosevka-calibrated: each
/// character advances `0.55 × font_size`, cap-height is `~0.72 × font_size`,
/// and the gap above the cap is `0.18 × font_size`.
#[allow(clippy::too_many_arguments)]
fn draw_overbars(
    frame: &mut canvas::Frame,
    plain: &str,
    overbars: &[(usize, usize)],
    anchor: iced::Point,
    font_size_px: f32,
    h_align: iced::alignment::Horizontal,
    v_align: iced::alignment::Vertical,
    color: Color,
) {
    let total_chars = plain.chars().count() as f32;
    let char_w = font_size_px * 0.55;
    let total_w = total_chars * char_w;
    let cap = font_size_px * 0.72;
    let gap = font_size_px * 0.24;
    let line_w = (font_size_px * 0.06).max(0.8);

    let text_left = match h_align {
        iced::alignment::Horizontal::Left => anchor.x,
        iced::alignment::Horizontal::Right => anchor.x - total_w,
        iced::alignment::Horizontal::Center => anchor.x - total_w * 0.5,
    };
    // Cap-top = top of the tallest glyph. Overline sits `gap` above it.
    let overline_y = match v_align {
        iced::alignment::Vertical::Top => anchor.y + (font_size_px - cap) * 0.5 - gap,
        iced::alignment::Vertical::Bottom => anchor.y - cap - gap,
        iced::alignment::Vertical::Center => anchor.y - cap * 0.5 - gap,
    };

    let stroke = canvas::Stroke::default()
        .with_color(color)
        .with_width(line_w)
        .with_line_cap(LineCap::Butt);

    for (start, len) in overbars {
        let x0 = text_left + (*start as f32) * char_w;
        let x1 = x0 + (*len as f32) * char_w;
        let path = canvas::Path::new(|b| {
            b.move_to(iced::Point::new(x0, overline_y));
            b.line_to(iced::Point::new(x1, overline_y));
        });
        frame.stroke(&path, stroke);
    }
}
