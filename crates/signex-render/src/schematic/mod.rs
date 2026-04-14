//! Schematic element rendering -- wires, symbols, labels, pins, etc.
//!
//! Each submodule handles rendering one schematic element type using
//! the Iced Canvas `Frame` API. All functions are pure: they take data,
//! a `ScreenTransform`, and colors, then draw onto the frame.

pub mod drawing;
pub mod hit_test;
pub mod junction;
pub mod label;
pub mod pin;
pub mod selection;
pub mod symbol;
pub mod text;
pub mod wire;

use iced::Rectangle;
use iced::widget::canvas;

use signex_types::schematic::{LabelType, SchematicSheet};
use signex_types::theme::CanvasColors;

use crate::colors::to_iced;

// ---------------------------------------------------------------------------
// ScreenTransform -- decouples rendering from the app-layer Camera
// ---------------------------------------------------------------------------

/// Converts world coordinates (mm) to screen pixels.
///
/// The app layer constructs this from its `Camera` before calling render
/// functions, so `signex-render` never depends on `signex-app`.
#[derive(Debug, Clone, Copy)]
pub struct ScreenTransform {
    /// Screen-pixel offset of the world origin.
    pub offset_x: f32,
    pub offset_y: f32,
    /// Pixels per mm -- higher values mean more zoom.
    pub scale: f32,
}

impl ScreenTransform {
    /// Convert a world-space point (mm) to screen pixels.
    #[inline]
    pub fn world_to_screen(&self, x: f64, y: f64) -> (f32, f32) {
        (
            x as f32 * self.scale + self.offset_x,
            y as f32 * self.scale + self.offset_y,
        )
    }

    /// Convert a world-space distance (mm) to screen pixels.
    #[inline]
    pub fn world_len(&self, mm: f64) -> f32 {
        mm as f32 * self.scale
    }

    /// Return the iced `Point` for a world coordinate.
    #[inline]
    pub fn to_screen_point(&self, x: f64, y: f64) -> iced::Point {
        let (sx, sy) = self.world_to_screen(x, y);
        iced::Point::new(sx, sy)
    }
}

// ---------------------------------------------------------------------------
// Shared geometry helpers (used by symbol, drawing, hit_test)
// ---------------------------------------------------------------------------

/// Compute the circumscribed circle center and radius from three points.
/// Returns `None` if the points are collinear.
pub(super) fn circle_from_three_points(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x3: f64,
    y3: f64,
) -> Option<(f64, f64, f64)> {
    let d = 2.0 * (x1 * (y2 - y3) + x2 * (y3 - y1) + x3 * (y1 - y2));
    if d.abs() < 1e-10 {
        return None;
    }
    let ux = ((x1 * x1 + y1 * y1) * (y2 - y3)
        + (x2 * x2 + y2 * y2) * (y3 - y1)
        + (x3 * x3 + y3 * y3) * (y1 - y2))
        / d;
    let uy = ((x1 * x1 + y1 * y1) * (x3 - x2)
        + (x2 * x2 + y2 * y2) * (x1 - x3)
        + (x3 * x3 + y3 * y3) * (x2 - x1))
        / d;
    let r = ((x1 - ux).powi(2) + (y1 - uy).powi(2)).sqrt();
    Some((ux, uy, r))
}

/// Check if `mid_angle` lies between `start_angle` and `end_angle` when
/// going counter-clockwise from start to end.
pub(super) fn is_angle_between_ccw(start: f64, mid: f64, end: f64) -> bool {
    let tau = std::f64::consts::TAU;
    let normalize = |a: f64| ((a % tau) + tau) % tau;
    let s = normalize(start);
    let m = normalize(mid);
    let e = normalize(end);
    if s <= e {
        s <= m && m <= e
    } else {
        m >= s || m <= e
    }
}

/// Return symbol field display position.
///
/// In our data model, field positions are stored as absolute schematic
/// coordinates from `.kicad_sch` and should be rendered directly.
pub(super) fn field_display_pos(
    prop_pos: &signex_types::schematic::Point,
    _sym: &signex_types::schematic::Symbol,
) -> (f64, f64) {
    (prop_pos.x, prop_pos.y)
}

/// Compute KiCad-like effective field draw properties under symbol TRANSFORM.
///
/// Returns `(draw_rotation_deg, effective_h_align, effective_v_align)` where
/// alignment flips follow transform parity and reading direction.
pub(super) fn field_effective_style(
    prop: &signex_types::schematic::TextProp,
    sym: &signex_types::schematic::Symbol,
) -> (
    f64,
    signex_types::schematic::HAlign,
    signex_types::schematic::VAlign,
) {
    use signex_types::schematic::{HAlign, VAlign};

    let is_horiz = |angle: f64| {
        let a = angle.rem_euclid(180.0);
        a < 0.1 || (180.0 - a) < 0.1
    };

    let rad = sym.rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let sx = if sym.mirror_x { -1.0 } else { 1.0 };
    let sy = if sym.mirror_y { -1.0 } else { 1.0 };

    // Matrix for field display transform:
    // [tx]   [x1 x2] [rx]
    // [ty] = [y1 y2] [ry]
    let x1 = sy * cos;
    let x2 = sy * sin;
    let y1 = sx * sin;
    let y2 = -sx * cos;

    let orig_horiz = is_horiz(prop.rotation);
    let screen_horiz = (x1.abs() > 1e-6) ^ !orig_horiz;
    let draw_rotation = if screen_horiz { 0.0 } else { 90.0 };

    let flip_h = if orig_horiz {
        if screen_horiz { x1 < 0.0 } else { x2 > 0.0 }
    } else if screen_horiz {
        y1 > 0.0
    } else {
        y2 < 0.0
    };

    let mut h = prop.justify_h;
    if flip_h {
        h = match h {
            HAlign::Left => HAlign::Right,
            HAlign::Right => HAlign::Left,
            HAlign::Center => HAlign::Center,
        };
    }

    let mut v = prop.justify_v;
    let det = x1 * y2 - x2 * y1;
    if det < 0.0 && (orig_horiz == (x1 > 0.0)) {
        v = match v {
            VAlign::Top => VAlign::Bottom,
            VAlign::Bottom => VAlign::Top,
            VAlign::Center => VAlign::Center,
        };
    }

    (draw_rotation, h, v)
}

/// Transform a local library-space point through a symbol instance's
/// position, rotation, and mirror state, returning a world-space point.
pub(super) fn instance_transform(
    sym: &signex_types::schematic::Symbol,
    local: &signex_types::schematic::Point,
) -> (f64, f64) {
    // Step 1: Flip Y — KiCad library coords are Y-up, schematic is Y-down.
    let x = local.x;
    let y = -local.y;
    // Step 2: Rotate by NEGATIVE angle.
    let rad = -sym.rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let rx = x * cos - y * sin;
    let ry = x * sin + y * cos;
    // Step 3: Mirror applied AFTER rotation (KiCad convention).
    let rx = if sym.mirror_y { -rx } else { rx };
    let ry = if sym.mirror_x { -ry } else { ry };
    // Step 4: Translate to world position.
    (rx + sym.position.x, ry + sym.position.y)
}

// ---------------------------------------------------------------------------
// Main render entry point
// ---------------------------------------------------------------------------

/// Draw all elements of a schematic sheet onto the canvas frame.
///
/// Elements are rendered in z-order so that higher-layer items paint
/// on top of lower ones.
pub fn render_schematic(
    frame: &mut canvas::Frame,
    sheet: &SchematicSheet,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    _bounds: Rectangle,
) {
    let body_color = to_iced(&colors.body);
    let body_fill_color = to_iced(&colors.body_fill);
    let wire_color = to_iced(&colors.wire);
    let junction_color = to_iced(&colors.junction);
    let pin_color = to_iced(&colors.pin);
    let reference_color = to_iced(&colors.reference);
    let value_color = to_iced(&colors.value);
    let no_connect_color = to_iced(&colors.no_connect);
    let bus_color = to_iced(&colors.bus);
    let power_color = to_iced(&colors.power);

    // Z=1: Drawing primitives (lines, rects, circles, arcs, polylines)
    for d in &sheet.drawings {
        drawing::draw_sch_drawing(frame, d, transform, body_color);
    }

    // Z=2: Wires
    for w in &sheet.wires {
        wire::draw_wire(frame, w, transform, wire_color);
    }

    // Z=3: Buses
    for b in &sheet.buses {
        wire::draw_bus(frame, b, transform, bus_color);
    }

    // Z=4: Bus entries
    for be in &sheet.bus_entries {
        wire::draw_bus_entry(frame, be, transform, bus_color);
    }

    // Z=5: Junctions
    for j in &sheet.junctions {
        junction::draw_junction(frame, j, transform, junction_color);
    }

    // Z=6: No-connect markers
    for nc in &sheet.no_connects {
        junction::draw_no_connect(frame, nc, transform, no_connect_color);
    }

    // Z=7-9: Labels (net, global, hierarchical, power)
    for lbl in &sheet.labels {
        let color = match lbl.label_type {
            LabelType::Net => to_iced(&colors.net_label),
            LabelType::Global => to_iced(&colors.global_label),
            LabelType::Hierarchical => to_iced(&colors.hier_label),
            LabelType::Power => to_iced(&colors.power),
        };
        label::draw_label(frame, lbl, transform, color);
    }

    // Z=10-11: Symbol bodies + pins
    for sym in &sheet.symbols {
        if let Some(lib_sym) = sheet.lib_symbols.get(&sym.lib_id) {
            symbol::draw_symbol(
                frame,
                sym,
                lib_sym,
                transform,
                body_color,
                body_fill_color,
                pin_color,
            );

            // Pins
            pin::draw_symbol_pins(frame, sym, lib_sym, transform, pin_color);

            // Reference text — power symbols (#PWR refs) are always hidden
            if let Some(ref ref_text) = sym.ref_text
                && !ref_text.hidden
                && !sym.is_power
            {
                let dpos = field_display_pos(&ref_text.position, sym);
                text::draw_text_prop(frame, &sym.reference, ref_text, sym, dpos, transform, reference_color);
            }

            // Value text
            if let Some(ref val_text) = sym.val_text
                && !val_text.hidden
            {
                let dpos = field_display_pos(&val_text.position, sym);
                text::draw_text_prop(frame, &sym.value, val_text, sym, dpos, transform, value_color);
            }
        } else if sym.is_power {
            // Built-in Altium-style power symbol rendering (no lib_symbol needed)
            draw_builtin_power(frame, sym, transform, power_color, value_color);
        }
    }

    // Z=11b: Child sheets (hierarchical sheets)
    for child in &sheet.child_sheets {
        drawing::draw_child_sheet(frame, child, transform, body_color, body_fill_color);
    }

    // Z=12: Text notes
    for tn in &sheet.text_notes {
        text::draw_text_note(frame, tn, transform, to_iced(&colors.body));
    }
}

// ---------------------------------------------------------------------------
// Built-in Altium-style power symbol rendering
// ---------------------------------------------------------------------------

/// Draw a built-in power symbol when no lib_symbol definition exists.
/// Renders Altium-style shapes: GND (3 horizontal lines), VCC (bar + arrow),
/// Earth (diagonal hatch), Signal GND (triangle), generic (bar + label).
fn draw_builtin_power(
    frame: &mut canvas::Frame,
    sym: &signex_types::schematic::Symbol,
    transform: &ScreenTransform,
    color: iced::Color,
    label_color: iced::Color,
) {
    use iced::widget::canvas::path;

    let sw = (transform.scale * 0.15).clamp(0.5, 2.0);
    let stroke = canvas::Stroke::default().with_color(color).with_width(sw);

    // Pin line: vertical stub from anchor upward (1.27mm) — connection point
    let pin_len = 1.27;
    let (p0x, p0y) = instance_transform(sym, &signex_types::schematic::Point::new(0.0, 0.0));
    let (p1x, p1y) = instance_transform(sym, &signex_types::schematic::Point::new(0.0, pin_len));
    let s0 = transform.to_screen_point(p0x, p0y);
    let s1 = transform.to_screen_point(p1x, p1y);
    frame.stroke(&canvas::Path::line(s0, s1), stroke);

    // Identify power type from lib_id or value
    let id = sym.lib_id.to_lowercase();
    let net = sym.value.to_uppercase();

    if id.contains("gnd") && !id.contains("earth") && !id.contains("gndref") {
        // GND: 3 horizontal lines of decreasing width
        let bar_w = 2.54;
        for (i, frac) in [1.0_f64, 0.65, 0.3].iter().enumerate() {
            let dy = pin_len + 0.4 * i as f64;
            let hw = bar_w * 0.5 * frac;
            let (lx, ly) = instance_transform(sym, &signex_types::schematic::Point::new(-hw, dy));
            let (rx, ry) = instance_transform(sym, &signex_types::schematic::Point::new(hw, dy));
            let sl = transform.to_screen_point(lx, ly);
            let sr = transform.to_screen_point(rx, ry);
            frame.stroke(&canvas::Path::line(sl, sr), stroke);
        }
    } else if id.contains("gndref") {
        // Signal GND: downward triangle
        let hw = 1.27;
        let tri_h = 1.27;
        let base_y = pin_len;
        let pts = [
            signex_types::schematic::Point::new(-hw, base_y),
            signex_types::schematic::Point::new(hw, base_y),
            signex_types::schematic::Point::new(0.0, base_y + tri_h),
        ];
        let screen_pts: Vec<iced::Point> = pts
            .iter()
            .map(|p| {
                let (wx, wy) = instance_transform(sym, p);
                transform.to_screen_point(wx, wy)
            })
            .collect();
        let tri = canvas::Path::new(|b: &mut path::Builder| {
            b.move_to(screen_pts[0]);
            b.line_to(screen_pts[1]);
            b.line_to(screen_pts[2]);
            b.close();
        });
        frame.stroke(&tri, stroke);
    } else if id.contains("earth") {
        // Earth: horizontal bar + 3 diagonal hatch lines
        let bar_w = 2.54;
        let base_y = pin_len;
        let hw = bar_w * 0.5;
        let (lx, ly) = instance_transform(sym, &signex_types::schematic::Point::new(-hw, base_y));
        let (rx, ry) = instance_transform(sym, &signex_types::schematic::Point::new(hw, base_y));
        frame.stroke(
            &canvas::Path::line(
                transform.to_screen_point(lx, ly),
                transform.to_screen_point(rx, ry),
            ),
            stroke,
        );
        // Diagonal hatch lines below bar
        for i in 0..3 {
            let x_off = -hw + (i as f64 + 0.5) * (bar_w / 3.0);
            let (hx1, hy1) = instance_transform(
                sym,
                &signex_types::schematic::Point::new(x_off, base_y),
            );
            let (hx2, hy2) = instance_transform(
                sym,
                &signex_types::schematic::Point::new(x_off - 0.5, base_y + 0.8),
            );
            frame.stroke(
                &canvas::Path::line(
                    transform.to_screen_point(hx1, hy1),
                    transform.to_screen_point(hx2, hy2),
                ),
                stroke,
            );
        }
    } else {
        // VCC / generic power: horizontal bar at top of pin
        let bar_w = 2.54;
        let base_y = pin_len;
        let hw = bar_w * 0.5;
        let (lx, ly) = instance_transform(sym, &signex_types::schematic::Point::new(-hw, base_y));
        let (rx, ry) = instance_transform(sym, &signex_types::schematic::Point::new(hw, base_y));
        frame.stroke(
            &canvas::Path::line(
                transform.to_screen_point(lx, ly),
                transform.to_screen_point(rx, ry),
            ),
            stroke,
        );
    }

    // Draw value label (net name) above the symbol
    let label_y = pin_len + 1.5;
    let font_size_mm = 1.27;
    let screen_font = (transform.world_len(font_size_mm) * crate::canvas_font_size_scale()).abs();
    if screen_font >= 1.0 {
        let (tx, ty) = instance_transform(
            sym,
            &signex_types::schematic::Point::new(0.0, label_y),
        );
        let sp = transform.to_screen_point(tx, ty);
        frame.fill_text(canvas::Text {
            content: net,
            position: sp,
            color: label_color,
            size: iced::Pixels(screen_font),
            font: crate::canvas_font(),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Top,
            ..canvas::Text::default()
        });
    }
}
