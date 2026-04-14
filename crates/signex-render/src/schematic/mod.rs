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
