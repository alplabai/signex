use signex_types::schematic::{SchDrawing, SchematicSheet, SelectedItem, SelectedKind};

use crate::command::MirrorAxis;

use super::Engine;

impl Engine {
    pub(crate) fn contains_selected_item(&self, item: &SelectedItem) -> bool {
        match item.kind {
            SelectedKind::Wire => self
                .document
                .wires
                .iter()
                .any(|wire| wire.uuid == item.uuid),
            SelectedKind::Bus => self.document.buses.iter().any(|bus| bus.uuid == item.uuid),
            SelectedKind::Label => self
                .document
                .labels
                .iter()
                .any(|label| label.uuid == item.uuid),
            SelectedKind::Junction => self
                .document
                .junctions
                .iter()
                .any(|junction| junction.uuid == item.uuid),
            SelectedKind::NoConnect => self
                .document
                .no_connects
                .iter()
                .any(|no_connect| no_connect.uuid == item.uuid),
            SelectedKind::Symbol => self
                .document
                .symbols
                .iter()
                .any(|symbol| symbol.uuid == item.uuid),
            SelectedKind::TextNote => self
                .document
                .text_notes
                .iter()
                .any(|text_note| text_note.uuid == item.uuid),
            SelectedKind::SheetPin => self.document.child_sheets.iter().any(|child_sheet| {
                child_sheet
                    .pins
                    .iter()
                    .any(|sheet_pin| sheet_pin.uuid == item.uuid)
            }),
            SelectedKind::Drawing => self.document.drawings.iter().any(|d| {
                let u = match d {
                    SchDrawing::Line { uuid, .. }
                    | SchDrawing::Rect { uuid, .. }
                    | SchDrawing::Circle { uuid, .. }
                    | SchDrawing::Arc { uuid, .. }
                    | SchDrawing::Polyline { uuid, .. } => *uuid,
                };
                u == item.uuid
            }),
            _ => false,
        }
    }

    pub(super) fn remove_selected_item(&mut self, item: &SelectedItem) -> bool {
        match item.kind {
            SelectedKind::Wire => remove_by_uuid(&mut self.document.wires, item.uuid),
            SelectedKind::Bus => remove_by_uuid(&mut self.document.buses, item.uuid),
            SelectedKind::Label => remove_by_uuid(&mut self.document.labels, item.uuid),
            SelectedKind::Junction => remove_by_uuid(&mut self.document.junctions, item.uuid),
            SelectedKind::NoConnect => remove_by_uuid(&mut self.document.no_connects, item.uuid),
            SelectedKind::Symbol => remove_by_uuid(&mut self.document.symbols, item.uuid),
            SelectedKind::TextNote => remove_by_uuid(&mut self.document.text_notes, item.uuid),
            SelectedKind::Drawing => {
                let before_len = self.document.drawings.len();
                self.document.drawings.retain(|d| {
                    let u = match d {
                        SchDrawing::Line { uuid, .. }
                        | SchDrawing::Rect { uuid, .. }
                        | SchDrawing::Circle { uuid, .. }
                        | SchDrawing::Arc { uuid, .. }
                        | SchDrawing::Polyline { uuid, .. } => *uuid,
                    };
                    u != item.uuid
                });
                self.document.drawings.len() != before_len
            }
            _ => false,
        }
    }

    pub(super) fn move_selected_item(&mut self, item: &SelectedItem, dx: f64, dy: f64) -> bool {
        match item.kind {
            SelectedKind::Symbol => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    symbol.position.x += dx;
                    symbol.position.y += dy;
                    if let Some(ref mut ref_text) = symbol.ref_text {
                        ref_text.position.x += dx;
                        ref_text.position.y += dy;
                    }
                    if let Some(ref mut val_text) = symbol.val_text {
                        val_text.position.x += dx;
                        val_text.position.y += dy;
                    }
                    true
                })
                .unwrap_or(false),
            SelectedKind::Wire => self
                .document
                .wires
                .iter_mut()
                .find(|wire| wire.uuid == item.uuid)
                .map(|wire| {
                    wire.start.x += dx;
                    wire.start.y += dy;
                    wire.end.x += dx;
                    wire.end.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::Bus => self
                .document
                .buses
                .iter_mut()
                .find(|bus| bus.uuid == item.uuid)
                .map(|bus| {
                    bus.start.x += dx;
                    bus.start.y += dy;
                    bus.end.x += dx;
                    bus.end.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::Label => self
                .document
                .labels
                .iter_mut()
                .find(|label| label.uuid == item.uuid)
                .map(|label| {
                    label.position.x += dx;
                    label.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::Junction => self
                .document
                .junctions
                .iter_mut()
                .find(|junction| junction.uuid == item.uuid)
                .map(|junction| {
                    junction.position.x += dx;
                    junction.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::NoConnect => self
                .document
                .no_connects
                .iter_mut()
                .find(|no_connect| no_connect.uuid == item.uuid)
                .map(|no_connect| {
                    no_connect.position.x += dx;
                    no_connect.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::TextNote => self
                .document
                .text_notes
                .iter_mut()
                .find(|text_note| text_note.uuid == item.uuid)
                .map(|text_note| {
                    text_note.position.x += dx;
                    text_note.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::ChildSheet => self
                .document
                .child_sheets
                .iter_mut()
                .find(|child_sheet| child_sheet.uuid == item.uuid)
                .map(|child_sheet| {
                    child_sheet.position.x += dx;
                    child_sheet.position.y += dy;
                    for sheet_pin in &mut child_sheet.pins {
                        sheet_pin.position.x += dx;
                        sheet_pin.position.y += dy;
                    }
                    true
                })
                .unwrap_or(false),
            SelectedKind::SheetPin => {
                for child_idx in 0..self.document.child_sheets.len() {
                    if let Some(pin_idx) = self.document.child_sheets[child_idx]
                        .pins
                        .iter()
                        .position(|sheet_pin| sheet_pin.uuid == item.uuid)
                    {
                        let (cx, cy, cw, ch) = {
                            let c = &self.document.child_sheets[child_idx];
                            (c.position.x, c.position.y, c.size.0, c.size.1)
                        };
                        let pin = &mut self.document.child_sheets[child_idx].pins[pin_idx];
                        super::sheet::lock_sheet_pin_to_child_edge(pin, dx, dy, cx, cy, cw, ch);
                        pin.user_moved = true;
                        return true;
                    }
                }
                false
            }
            SelectedKind::BusEntry => self
                .document
                .bus_entries
                .iter_mut()
                .find(|bus_entry| bus_entry.uuid == item.uuid)
                .map(|bus_entry| {
                    bus_entry.position.x += dx;
                    bus_entry.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::SymbolRefField => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    let (field_dx, field_dy) = inverse_field_display_delta(dx, dy);
                    if let Some(ref mut ref_text) = symbol.ref_text {
                        ref_text.position.x += field_dx;
                        ref_text.position.y += field_dy;
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false),
            SelectedKind::SymbolValField => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    let (field_dx, field_dy) = inverse_field_display_delta(dx, dy);
                    if let Some(ref mut val_text) = symbol.val_text {
                        val_text.position.x += field_dx;
                        val_text.position.y += field_dy;
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false),
            SelectedKind::Drawing => self
                .document
                .drawings
                .iter_mut()
                .find(|d| {
                    let u = match d {
                        SchDrawing::Line { uuid, .. }
                        | SchDrawing::Rect { uuid, .. }
                        | SchDrawing::Circle { uuid, .. }
                        | SchDrawing::Arc { uuid, .. }
                        | SchDrawing::Polyline { uuid, .. } => *uuid,
                    };
                    u == item.uuid
                })
                .map(|d| {
                    match d {
                        SchDrawing::Line { start, end, .. } => {
                            start.x += dx;
                            start.y += dy;
                            end.x += dx;
                            end.y += dy;
                        }
                        SchDrawing::Rect { start, end, .. } => {
                            start.x += dx;
                            start.y += dy;
                            end.x += dx;
                            end.y += dy;
                        }
                        SchDrawing::Circle { center, .. } => {
                            center.x += dx;
                            center.y += dy;
                        }
                        SchDrawing::Arc {
                            start, mid, end, ..
                        } => {
                            start.x += dx;
                            start.y += dy;
                            mid.x += dx;
                            mid.y += dy;
                            end.x += dx;
                            end.y += dy;
                        }
                        SchDrawing::Polyline { points, .. } => {
                            for p in points {
                                p.x += dx;
                                p.y += dy;
                            }
                        }
                    }
                    true
                })
                .unwrap_or(false),
        }
    }

    pub(super) fn rotate_selected_item(&mut self, item: &SelectedItem, angle_degrees: f64) -> bool {
        match item.kind {
            SelectedKind::Symbol => {
                let lib_symbols = &self.document.lib_symbols.clone();
                self.document
                    .symbols
                    .iter_mut()
                    .find(|symbol| symbol.uuid == item.uuid)
                    .map(|symbol| {
                        symbol.rotation = (symbol.rotation + angle_degrees).rem_euclid(360.0);
                        if let Some(lib) = lib_symbols.get(&symbol.lib_id) {
                            autoplace_fields(symbol, lib);
                        }
                        true
                    })
                    .unwrap_or(false)
            }
            _ => false,
        }
    }

    pub(super) fn mirror_selected_item(&mut self, item: &SelectedItem, axis: MirrorAxis) -> bool {
        match item.kind {
            SelectedKind::Symbol => {
                let lib_symbols = &self.document.lib_symbols.clone();
                self.document
                    .symbols
                    .iter_mut()
                    .find(|symbol| symbol.uuid == item.uuid)
                    .map(|symbol| {
                        match axis {
                            MirrorAxis::Horizontal => symbol.mirror_y = !symbol.mirror_y,
                            MirrorAxis::Vertical => symbol.mirror_x = !symbol.mirror_x,
                        }
                        if let Some(lib) = lib_symbols.get(&symbol.lib_id) {
                            autoplace_fields(symbol, lib);
                        }
                        true
                    })
                    .unwrap_or(false)
            }
            _ => false,
        }
    }
}

/// Reposition the visible Reference and Value fields on the side of the
/// symbol body with the fewest pins, mirroring KiCad's `AutoplaceFields`
/// behaviour. Fields are stacked vertically and given a justify/rotation
/// that always renders horizontally.
fn autoplace_fields(symbol: &mut signex_types::schematic::Symbol, lib: &signex_types::schematic::LibSymbol) {
    use signex_types::schematic::{HAlign, VAlign};

    // 1a. Body bbox in world space (graphics only). Used as the geometric
    //     reference for pin-side classification — its centre is the natural
    //     pivot, just like KiCad's `SCH_SYMBOL::GetBodyBoundingBox()`.
    let mut body_bbox: Option<(f64, f64, f64, f64)> = None;
    let extend = |bbox: &mut Option<(f64, f64, f64, f64)>, x: f64, y: f64| match bbox {
        None => *bbox = Some((x, y, x, y)),
        Some((lx, ly, hx, hy)) => {
            if x < *lx { *lx = x; }
            if y < *ly { *ly = y; }
            if x > *hx { *hx = x; }
            if y > *hy { *hy = y; }
        }
    };
    for g in &lib.graphics {
        if g.unit != 0 && g.unit != symbol.unit {
            continue;
        }
        let pts = graphic_extent_points(&g.graphic);
        for (lx, ly) in pts {
            let (wx, wy) = transform_local_point(symbol, lx, ly);
            extend(&mut body_bbox, wx, wy);
        }
    }
    let body = body_bbox.unwrap_or((
        symbol.position.x - 1.27,
        symbol.position.y - 1.27,
        symbol.position.x + 1.27,
        symbol.position.y + 1.27,
    ));
    let (body_min_x, body_min_y, body_max_x, body_max_y) = body;
    let body_cx = (body_min_x + body_max_x) * 0.5;
    let body_cy = (body_min_y + body_max_y) * 0.5;

    // 1b. Outer bbox = body + visible pin endpoints. This is the same
    //     rectangle the selection overlay shows, and the autoplace anchors
    //     here so fields sit just past the pin extents.
    let mut outer_bbox = body_bbox;
    for p in &lib.pins {
        if p.unit != 0 && p.unit != symbol.unit {
            continue;
        }
        if !p.pin.visible {
            continue;
        }
        let rad = p.pin.rotation.to_radians();
        let (sx, sy) = (p.pin.position.x, p.pin.position.y);
        let (ex, ey) = (sx + p.pin.length * rad.cos(), sy + p.pin.length * rad.sin());
        for (lx, ly) in [(sx, sy), (ex, ey)] {
            let (wx, wy) = transform_local_point(symbol, lx, ly);
            extend(&mut outer_bbox, wx, wy);
        }
    }
    let (min_x, min_y, max_x, max_y) = outer_bbox.unwrap_or(body);
    let cx = (min_x + max_x) * 0.5;
    let cy = (min_y + max_y) * 0.5;

    // 2. Count pins on each side by the world-space position of each pin's
    //    connection point relative to the body bbox centre. Using the body
    //    centre (not the outer centre) keeps the classification independent
    //    of the pin lengths the user happens to have, matching KiCad's
    //    AutoplaceFields side-selection.
    let (mut pins_right, mut pins_left, mut pins_top, mut pins_bottom) = (0u32, 0u32, 0u32, 0u32);
    for p in &lib.pins {
        if p.unit != 0 && p.unit != symbol.unit {
            continue;
        }
        let (wx, wy) = transform_local_point(symbol, p.pin.position.x, p.pin.position.y);
        let dx = wx - body_cx;
        let dy = wy - body_cy;
        if dx.abs() >= dy.abs() {
            if dx >= 0.0 { pins_right += 1; } else { pins_left += 1; }
        } else if dy >= 0.0 {
            pins_bottom += 1;
        } else {
            pins_top += 1;
        }
    }

    // 3. Pick the side with the fewest pins. Ties resolved Right > Left > Top > Bottom
    //    to match KiCad's preference for horizontal placement.
    #[derive(Clone, Copy)]
    enum Side { Right, Left, Top, Bottom }
    let candidates = [
        (Side::Right, pins_right, 0),
        (Side::Left, pins_left, 1),
        (Side::Top, pins_top, 2),
        (Side::Bottom, pins_bottom, 3),
    ];
    let side = candidates
        .iter()
        .min_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)))
        .map(|(s, _, _)| *s)
        .unwrap_or(Side::Right);

    // 4. Collect visible fields in stacking order: Reference first, Value second.
    let mut fields: Vec<&mut signex_types::schematic::TextProp> = Vec::new();
    if let Some(rt) = symbol.ref_text.as_mut()
        && !rt.hidden
    {
        fields.push(rt);
    }
    if let Some(vt) = symbol.val_text.as_mut()
        && !vt.hidden
    {
        fields.push(vt);
    }
    if fields.is_empty() {
        symbol.fields_autoplaced = true;
        return;
    }

    // 5. Anchor and per-field justify. Fields are always stacked vertically.
    //    `line_height` is roughly KiCad's 1.6 * text_size; using the first
    //    visible field's font size keeps it scale-correct.
    //
    //    Justify-V is chosen so the field block sits cleanly outside the
    //    body: VAlign::Top below the body grows downward from the anchor,
    //    VAlign::Bottom above the body grows upward, and Center is used
    //    on horizontal sides where the block straddles cy symmetrically.
    let font_size = fields[0].font_size.max(0.1);
    let line_height = font_size * 1.6;
    // KiCad's AutoplaceFields keeps roughly two text-heights of clearance
    // between the body bbox and the closest field so the stack visibly
    // separates from the symbol. A single text-height was still cramped
    // compared to KiCad's reference rendering.
    let margin = (font_size * 2.0).max(1.016);
    let n = fields.len() as f64;

    let (anchor_x, anchor_y_first, justify_h, justify_v): (f64, f64, HAlign, VAlign) = match side {
        Side::Right => (
            max_x + margin,
            cy - (n - 1.0) * line_height * 0.5,
            HAlign::Left,
            VAlign::Center,
        ),
        Side::Left => (
            min_x - margin,
            cy - (n - 1.0) * line_height * 0.5,
            HAlign::Right,
            VAlign::Center,
        ),
        Side::Top => (
            cx,
            min_y - margin - (n - 1.0) * line_height,
            HAlign::Center,
            VAlign::Bottom,
        ),
        Side::Bottom => (cx, max_y + margin, HAlign::Center, VAlign::Top),
    };

    // 6. Field rotation must fold to 0 in `field_effective_style` so the
    //    rendered text is always horizontal regardless of symbol rotation.
    let field_rotation = (360.0 - symbol.rotation).rem_euclid(360.0);

    for (i, prop) in fields.iter_mut().enumerate() {
        prop.position.x = anchor_x;
        prop.position.y = anchor_y_first + i as f64 * line_height;
        prop.justify_h = justify_h;
        prop.justify_v = justify_v;
        prop.rotation = field_rotation;
    }

    symbol.fields_autoplaced = true;
}

/// Apply a symbol instance's position, rotation, and mirror to a local
/// library-space point, returning the world-space coordinates. Mirrors
/// `signex_render::instance_transform`'s convention so engine geometry
/// matches the renderer.
fn transform_local_point(sym: &signex_types::schematic::Symbol, lx: f64, ly: f64) -> (f64, f64) {
    let x = lx;
    let y = -ly; // library Y-up → schematic Y-down
    let rad = -sym.rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let mut rx = x * cos - y * sin;
    let mut ry = x * sin + y * cos;
    if sym.mirror_y {
        rx = -rx;
    }
    if sym.mirror_x {
        ry = -ry;
    }
    (rx + sym.position.x, ry + sym.position.y)
}

/// Return all extreme points of a library graphic for bounding-box
/// computation. For closed shapes this returns the corners; for open
/// shapes the vertices.
fn graphic_extent_points(g: &signex_types::schematic::Graphic) -> Vec<(f64, f64)> {
    use signex_types::schematic::Graphic;
    match g {
        Graphic::Polyline { points, .. } | Graphic::Bezier { points, .. } => {
            points.iter().map(|p| (p.x, p.y)).collect()
        }
        Graphic::Rectangle { start, end, .. } => vec![
            (start.x, start.y),
            (end.x, start.y),
            (end.x, end.y),
            (start.x, end.y),
        ],
        Graphic::Circle { center, radius, .. } => vec![
            (center.x - *radius, center.y - *radius),
            (center.x + *radius, center.y - *radius),
            (center.x - *radius, center.y + *radius),
            (center.x + *radius, center.y + *radius),
        ],
        Graphic::Arc { start, mid, end, .. } => {
            vec![(start.x, start.y), (mid.x, mid.y), (end.x, end.y)]
        }
        Graphic::Text { position, .. } | Graphic::TextBox { position, .. } => {
            vec![(position.x, position.y)]
        }
    }
}

/// Rotate `(x, y)` around `(cx, cy)` by `angle_deg` using the same screen
/// convention as `signex_render::instance_transform` (Y-down schematic
/// coordinates, positive `angle_deg` = visual CCW rotation as seen by the
/// user, matching KiCad's rotate command).
///
/// In a Y-down coordinate space the standard rotation matrix gives a visual
/// CW rotation, so we negate the angle to recover the expected visual CCW.
#[allow(dead_code)]
fn rotate_point_around(x: f64, y: f64, cx: f64, cy: f64, angle_deg: f64) -> (f64, f64) {
    let rx = x - cx;
    let ry = y - cy;
    let rad = -angle_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    (cx + rx * cos - ry * sin, cy + rx * sin + ry * cos)
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

fn inverse_field_display_delta(dx: f64, dy: f64) -> (f64, f64) {
    (dx, dy)
}

fn point_on_wire_interior(
    point: signex_types::schematic::Point,
    wire: &signex_types::schematic::Wire,
    tolerance: f64,
) -> bool {
    let (ax, ay) = (wire.start.x, wire.start.y);
    let (bx, by) = (wire.end.x, wire.end.y);
    let (px, py) = (point.x, point.y);
    let (abx, aby) = (bx - ax, by - ay);
    let (apx, apy) = (px - ax, py - ay);
    let len_sq = abx * abx + aby * aby;

    if len_sq < tolerance * tolerance {
        return false;
    }

    let cross = abx * apy - aby * apx;
    if (cross * cross) > tolerance * tolerance * len_sq {
        return false;
    }

    let t = (apx * abx + apy * aby) / len_sq;
    let margin = tolerance / len_sq.sqrt();
    t > margin && t < 1.0 - margin
}

pub(crate) fn needed_junction(
    point: signex_types::schematic::Point,
    document: &SchematicSheet,
    tolerance: f64,
) -> Option<signex_types::schematic::Junction> {
    let already_present = document.junctions.iter().any(|junction| {
        (junction.position.x - point.x).abs() < tolerance
            && (junction.position.y - point.y).abs() < tolerance
    });
    if already_present {
        return None;
    }

    let on_wire_interior = document
        .wires
        .iter()
        .any(|wire| point_on_wire_interior(point, wire, tolerance));
    if on_wire_interior {
        return Some(signex_types::schematic::Junction {
            uuid: uuid::Uuid::new_v4(),
            position: point,
            diameter: 0.0,
        });
    }

    let endpoint_count = document
        .wires
        .iter()
        .filter(|wire| {
            let at_start = (wire.start.x - point.x).abs() < tolerance
                && (wire.start.y - point.y).abs() < tolerance;
            let at_end = (wire.end.x - point.x).abs() < tolerance
                && (wire.end.y - point.y).abs() < tolerance;
            at_start || at_end
        })
        .count();
    if endpoint_count >= 3 {
        return Some(signex_types::schematic::Junction {
            uuid: uuid::Uuid::new_v4(),
            position: point,
            diameter: 0.0,
        });
    }

    None
}

// ---------------------------------------------------------------------------
// UUID-based collection helpers
// ---------------------------------------------------------------------------

fn remove_by_uuid<T>(items: &mut Vec<T>, uuid: uuid::Uuid) -> bool
where
    T: HasUuid,
{
    let original_len = items.len();
    items.retain(|item| item.uuid() != uuid);
    original_len != items.len()
}

trait HasUuid {
    fn uuid(&self) -> uuid::Uuid;
}

macro_rules! impl_has_uuid {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl HasUuid for $ty {
                fn uuid(&self) -> uuid::Uuid {
                    self.uuid
                }
            }
        )+
    };
}

impl_has_uuid!(
    signex_types::schematic::Wire,
    signex_types::schematic::Bus,
    signex_types::schematic::Label,
    signex_types::schematic::Junction,
    signex_types::schematic::NoConnect,
    signex_types::schematic::Symbol,
    signex_types::schematic::TextNote,
);
