//! Hit-testing for schematic elements -- determines what the user clicked.
//!
//! All functions operate in world coordinates (mm). The app layer converts
//! the screen click position to world coords before calling these.

use signex_types::schematic::*;

use super::{SchematicRenderSnapshot, field_display_pos, field_effective_style};

/// Threshold distance in mm for considering a click "on" a thin element.
const HIT_TOLERANCE: f64 = 1.5;

/// Find the topmost element at the given world position.
/// Elements are tested in reverse z-order (top first) so the first hit wins.
pub fn hit_test(sheet: &SchematicRenderSnapshot, wx: f64, wy: f64) -> Option<SelectedItem> {
    // Labels (topmost in z-order)
    for lbl in &sheet.labels {
        if hit_label(lbl, wx, wy) {
            return Some(SelectedItem::new(lbl.uuid, SelectedKind::Label));
        }
    }

    // Junctions
    for j in &sheet.junctions {
        if hit_junction(j, wx, wy) {
            return Some(SelectedItem::new(j.uuid, SelectedKind::Junction));
        }
    }

    // No-connects
    for nc in &sheet.no_connects {
        if hit_no_connect(nc, wx, wy) {
            return Some(SelectedItem::new(nc.uuid, SelectedKind::NoConnect));
        }
    }

    // Text notes
    for tn in &sheet.text_notes {
        if hit_text_note(tn, wx, wy) {
            return Some(SelectedItem::new(tn.uuid, SelectedKind::TextNote));
        }
    }

    // Child sheets
    for cs in &sheet.child_sheets {
        if hit_child_sheet(cs, wx, wy) {
            return Some(SelectedItem::new(cs.uuid, SelectedKind::ChildSheet));
        }
    }

    // Symbol field texts (tested before symbol body so clicking a label
    // selects the field, not the whole symbol). Power ports are an exception:
    // their val_text is rendered as part of the port by the built-in power
    // renderer, so clicking anywhere on the glyph should select the whole
    // port — not a phantom val_text hit region tracking KiCad's stored text
    // position (often offset below the visible body).
    for sym in &sheet.symbols {
        if sym.is_power {
            continue;
        }
        if let Some(ref ref_text) = sym.ref_text
            && !ref_text.hidden
            && hit_text_prop(&sym.reference, ref_text, sym, wx, wy)
        {
            return Some(SelectedItem::new(sym.uuid, SelectedKind::SymbolRefField));
        }
        if let Some(ref val_text) = sym.val_text
            && !val_text.hidden
            && hit_text_prop(&sym.value, val_text, sym, wx, wy)
        {
            return Some(SelectedItem::new(sym.uuid, SelectedKind::SymbolValField));
        }
    }

    // Symbols
    for sym in &sheet.symbols {
        if let Some(lib_sym) = sheet.lib_symbols.get(&sym.lib_id)
            && hit_symbol(sym, lib_sym, wx, wy)
        {
            return Some(SelectedItem::new(sym.uuid, SelectedKind::Symbol));
        }
        // Power ports always rely on the built-in renderer for their glyph,
        // even when a matching lib_sym exists in the sheet's `lib_symbols`.
        // So the hit region has to come from the built-in geometry — without
        // this second pass, a power port whose Style was swapped (lib_id now
        // points at a non-existent library symbol) would be unselectable.
        if sym.is_power && hit_power_symbol(sym, wx, wy) {
            return Some(SelectedItem::new(sym.uuid, SelectedKind::Symbol));
        }
    }

    // Wires
    for w in &sheet.wires {
        if hit_wire(w, wx, wy) {
            return Some(SelectedItem::new(w.uuid, SelectedKind::Wire));
        }
    }

    // Buses
    for b in &sheet.buses {
        if hit_bus(b, wx, wy) {
            return Some(SelectedItem::new(b.uuid, SelectedKind::Bus));
        }
    }

    // Bus entries
    for be in &sheet.bus_entries {
        if hit_bus_entry(be, wx, wy) {
            return Some(SelectedItem::new(be.uuid, SelectedKind::BusEntry));
        }
    }

    None
}

/// Find all elements within a rectangular region (rubber-band selection).
pub fn hit_test_rect(sheet: &SchematicRenderSnapshot, rect: &Aabb) -> Vec<SelectedItem> {
    let mut result = Vec::new();

    for sym in &sheet.symbols {
        if rect.contains(sym.position.x, sym.position.y) {
            result.push(SelectedItem::new(sym.uuid, SelectedKind::Symbol));
        }
    }
    for w in &sheet.wires {
        if rect.contains(w.start.x, w.start.y) && rect.contains(w.end.x, w.end.y) {
            result.push(SelectedItem::new(w.uuid, SelectedKind::Wire));
        }
    }
    for b in &sheet.buses {
        if rect.contains(b.start.x, b.start.y) && rect.contains(b.end.x, b.end.y) {
            result.push(SelectedItem::new(b.uuid, SelectedKind::Bus));
        }
    }
    for j in &sheet.junctions {
        if rect.contains(j.position.x, j.position.y) {
            result.push(SelectedItem::new(j.uuid, SelectedKind::Junction));
        }
    }
    for nc in &sheet.no_connects {
        if rect.contains(nc.position.x, nc.position.y) {
            result.push(SelectedItem::new(nc.uuid, SelectedKind::NoConnect));
        }
    }
    for lbl in &sheet.labels {
        if rect.contains(lbl.position.x, lbl.position.y) {
            result.push(SelectedItem::new(lbl.uuid, SelectedKind::Label));
        }
    }
    for tn in &sheet.text_notes {
        if rect.contains(tn.position.x, tn.position.y) {
            result.push(SelectedItem::new(tn.uuid, SelectedKind::TextNote));
        }
    }
    for cs in &sheet.child_sheets {
        let cx = cs.position.x + cs.size.0 / 2.0;
        let cy = cs.position.y + cs.size.1 / 2.0;
        if rect.contains(cx, cy) {
            result.push(SelectedItem::new(cs.uuid, SelectedKind::ChildSheet));
        }
    }

    result
}

fn hit_wire(w: &Wire, wx: f64, wy: f64) -> bool {
    point_to_segment_dist(wx, wy, w.start.x, w.start.y, w.end.x, w.end.y) < HIT_TOLERANCE
}

fn hit_bus(b: &Bus, wx: f64, wy: f64) -> bool {
    point_to_segment_dist(wx, wy, b.start.x, b.start.y, b.end.x, b.end.y) < HIT_TOLERANCE * 1.5
}

fn hit_bus_entry(be: &BusEntry, wx: f64, wy: f64) -> bool {
    let ex = be.position.x + be.size.0;
    let ey = be.position.y + be.size.1;
    point_to_segment_dist(wx, wy, be.position.x, be.position.y, ex, ey) < HIT_TOLERANCE
}

fn hit_junction(j: &Junction, wx: f64, wy: f64) -> bool {
    let r = if j.diameter > 0.0 {
        j.diameter / 2.0
    } else {
        0.5
    };
    let dx = wx - j.position.x;
    let dy = wy - j.position.y;
    (dx * dx + dy * dy).sqrt() < r + HIT_TOLERANCE
}

fn hit_no_connect(nc: &NoConnect, wx: f64, wy: f64) -> bool {
    let dx = (wx - nc.position.x).abs();
    let dy = (wy - nc.position.y).abs();
    dx < HIT_TOLERANCE * 1.5 && dy < HIT_TOLERANCE * 1.5
}

fn hit_label(lbl: &Label, wx: f64, wy: f64) -> bool {
    // Small hit tolerance so clicks near the glyph edges still register,
    // without bloating the visible selection rectangle.
    super::label::label_text_aabb(lbl).expand(0.3).contains(wx, wy)
}

fn hit_text_note(tn: &TextNote, wx: f64, wy: f64) -> bool {
    let text_width = tn.text.chars().count() as f64 * tn.font_size.max(1.27) * 0.7;
    let text_height = tn.font_size.max(1.27) * 1.5;
    let aabb = Aabb::new(
        tn.position.x,
        tn.position.y - text_height,
        tn.position.x + text_width,
        tn.position.y + text_height * 0.5,
    )
    .expand(0.5);
    aabb.contains(wx, wy)
}

fn hit_child_sheet(cs: &ChildSheet, wx: f64, wy: f64) -> bool {
    Aabb::new(
        cs.position.x,
        cs.position.y,
        cs.position.x + cs.size.0,
        cs.position.y + cs.size.1,
    )
    .contains(wx, wy)
}

/// Hit-test a text property (reference or value field).
/// Approximates the text bounding box from character count and font size.
/// Uses `field_display_pos` so the hit region matches where the text is rendered.
fn hit_text_prop(content: &str, prop: &TextProp, sym: &Symbol, wx: f64, wy: f64) -> bool {
    let font_h = prop.font_size.max(1.27);
    let char_count = content.chars().count() as f64;
    // Iosevka is roughly 0.6× monospace: each char ≈ 0.6 × font_h wide.
    let text_w = char_count * font_h * 0.6;
    let half_h = font_h * 0.6;
    let margin = 0.5;

    // Use display position (TRANSFORM applied), not raw stored position.
    let (disp_x, disp_y) = field_display_pos(&prop.position, sym);
    let (draw_rotation, justify_h, _justify_v) = field_effective_style(prop, sym);

    // Click relative to text anchor
    let dx = wx - disp_x;
    let dy = wy - disp_y;

    // Undo the render rotation to map the click into unrotated text-local
    // space. `draw_text_prop` rotates by `-draw_rotation.to_radians()` (Iced
    // Y-down), so the inverse is the same sign: rotate the click-to-anchor
    // vector by `-(-θ) = +θ` in lyon math = `-θ` in Iced-visual. Either way,
    // `(ldx, ldy) = R(-θ) · (dx, dy)`.
    let (ldx, ldy) = if draw_rotation.abs() > 0.1 {
        let rad = -draw_rotation.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        (dx * cos + dy * sin, -dx * sin + dy * cos)
    } else {
        (dx, dy)
    };

    // Text-local bounding box (depends on justification)
    let (x_lo, x_hi) = match justify_h {
        HAlign::Left   => (-margin, text_w + margin),
        HAlign::Right  => (-(text_w + margin), margin),
        HAlign::Center => (-(text_w / 2.0 + margin), text_w / 2.0 + margin),
    };

    ldx >= x_lo && ldx <= x_hi && ldy >= -(half_h + margin) && ldy <= half_h + margin
}

/// Bounding-box hit test for built-in power ports (no lib_sym required).
/// Matches the rough extents drawn by `draw_builtin_power` — pin stub + bar.
fn hit_power_symbol(sym: &Symbol, wx: f64, wy: f64) -> bool {
    // Direction mirrors draw_builtin_power's logic.
    let id = sym.lib_id.to_lowercase();
    let is_gnd_like = id.contains("gnd");
    let dir: f64 = if is_gnd_like { -1.0 } else { 1.0 };
    let pin_len = 1.27;
    // Generous bounding region — a power port is a small glyph, and Altium
    // picks it even with a coarse click; we match that feel.
    let body_extent = 2.8;
    let half_w = 2.2;
    // In symbol-local coords: the body extends from 0 to (pin_len + body_extent) * dir
    // on the Y axis (after Y-flip), and ±half_w on X.
    let (lx, ly) = world_to_lib_space(sym, wx, wy);
    let y_min = 0.0_f64.min((pin_len + body_extent) * dir);
    let y_max = 0.0_f64.max((pin_len + body_extent) * dir);
    Aabb::new(-half_w, y_min, half_w, y_max)
        .expand(0.4)
        .contains(lx, ly)
}

fn hit_symbol(sym: &Symbol, lib_sym: &LibSymbol, wx: f64, wy: f64) -> bool {
    // Body-only AABB. Pins extend outward from the body; including them in
    // the hit region lets clicks on wires (which share the pin endpoints)
    // select the whole symbol. We test pins separately with line-distance so
    // clicking *on* a pin still selects the symbol, but clicking on a wire
    // beyond the pin tip falls through to wire hit-testing.
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    let mut has_body = false;

    for lg in &lib_sym.graphics {
        if lg.unit != 0 && lg.unit != sym.unit {
            continue;
        }
        if lg.body_style != 0 && lg.body_style != 1 {
            continue;
        }
        match &lg.graphic {
            Graphic::Rectangle { start, end, .. } => {
                ext(&mut min_x, &mut min_y, &mut max_x, &mut max_y, start.x, start.y);
                ext(&mut min_x, &mut min_y, &mut max_x, &mut max_y, end.x, end.y);
                has_body = true;
            }
            Graphic::Polyline { points, .. } => {
                for p in points {
                    ext(&mut min_x, &mut min_y, &mut max_x, &mut max_y, p.x, p.y);
                    has_body = true;
                }
            }
            Graphic::Circle { center, radius, .. } => {
                ext(&mut min_x, &mut min_y, &mut max_x, &mut max_y, center.x - radius, center.y - radius);
                ext(&mut min_x, &mut min_y, &mut max_x, &mut max_y, center.x + radius, center.y + radius);
                has_body = true;
            }
            Graphic::Arc { start, mid, end, .. } => {
                ext(&mut min_x, &mut min_y, &mut max_x, &mut max_y, start.x, start.y);
                ext(&mut min_x, &mut min_y, &mut max_x, &mut max_y, mid.x, mid.y);
                ext(&mut min_x, &mut min_y, &mut max_x, &mut max_y, end.x, end.y);
                has_body = true;
            }
            _ => {}
        }
    }

    // No body graphics at all (rare): fall back to small AABB around anchor.
    if !has_body {
        let aabb = Aabb::new(
            sym.position.x - 2.54,
            sym.position.y - 2.54,
            sym.position.x + 2.54,
            sym.position.y + 2.54,
        );
        return aabb.contains(wx, wy);
    }

    // Click in lib-local space.
    let (lx, ly) = world_to_lib_space(sym, wx, wy);

    // Primary: click inside the body rectangle (minimal padding).
    if Aabb::new(min_x, min_y, max_x, max_y).expand(0.25).contains(lx, ly) {
        return true;
    }

    // Secondary: click near a pin line (stub between pin body-side point and
    // pin tip). This keeps pin selection working without swallowing wires
    // that simply share the pin endpoint.
    for lp in &lib_sym.pins {
        if lp.unit != 0 && lp.unit != sym.unit {
            continue;
        }
        let p = &lp.pin;
        if !p.visible {
            continue;
        }
        let angle_rad = p.rotation.to_radians();
        let end_x = p.position.x + p.length * angle_rad.cos();
        let end_y = p.position.y + p.length * angle_rad.sin();
        if point_to_segment_dist(lx, ly, p.position.x, p.position.y, end_x, end_y) <= 0.6 {
            return true;
        }
    }

    false
}

fn point_to_segment_dist(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-9 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let cx = ax + t * dx;
    let cy = ay + t * dy;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

/// Transform a world point into symbol library-local coordinate space.
/// Exact inverse of `instance_transform` in symbol.rs:
///   forward:  Y-flip → rotate(-θ) → mirror → translate
///   inverse:  un-translate → un-mirror → rotate(+θ) → un-Y-flip
fn world_to_lib_space(sym: &Symbol, wx: f64, wy: f64) -> (f64, f64) {
    // Step 1: Un-translate
    let mut dx = wx - sym.position.x;
    let mut dy = wy - sym.position.y;

    // Step 2: Un-mirror (mirrors are self-inverse, applied in reverse order)
    if sym.mirror_y {
        dx = -dx;
    }
    if sym.mirror_x {
        dy = -dy;
    }

    // Step 3: Rotate by +θ (inverse of rotate by -θ)
    let rad = sym.rotation.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();
    let rx = dx * cos_a - dy * sin_a;
    let ry = dx * sin_a + dy * cos_a;

    // Step 4: Un-Y-flip
    (rx, -ry)
}

fn ext(min_x: &mut f64, min_y: &mut f64, max_x: &mut f64, max_y: &mut f64, x: f64, y: f64) {
    *min_x = min_x.min(x);
    *min_y = min_y.min(y);
    *max_x = max_x.max(x);
    *max_y = max_y.max(y);
}
