//! Free math / coordinate helpers for the symbol canvas — text and
//! stroke sizing at zoom, angle unwrapping, screen↔world conversion
//! with the canvas snap grid, colour packing, circle tessellation,
//! and selection-anchor lookup. Pure code motion out of `mod.rs`;
//! `pub(super)` so both the parent `canvas` module and its `input` /
//! `draw` submodules can reach them.

use super::*;

pub(super) fn text_size_px_from_mm(size_mm: f32, scale: f32) -> f32 {
    let em_mm = size_mm.max(0.1) / MM_PER_EM;
    (em_mm * scale).clamp(2.0, 96.0)
}

pub(super) fn stroke_px_at_zoom(base_width_px_at_100: f32, _scale: f32) -> f32 {
    base_width_px_at_100
}

/// Unwrap a raw `atan2` angle (in degrees, range `[-180, 180]`) so that
/// the result stays within 180° of `prev`. This removes the ±180° branch
/// cut when tracking a continuously-moving cursor angle.
///
/// Example: prev = 170°, raw = -170° → returns 190° (not -170°).
pub(super) fn unwrap_angle(prev: f64, raw: f64) -> f64 {
    let mut delta = raw - prev;
    // Bring delta into (-180, 180] so we always take the short arc.
    if delta > 180.0 {
        delta -= 360.0;
    }
    if delta <= -180.0 {
        delta += 360.0;
    }
    prev + delta
}

pub(super) fn stroke_world_mm(base_width_px_at_100: f32, scale: f32) -> f32 {
    (base_width_px_at_100 / scale.max(0.001))
        .max(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM as f32)
}

pub(super) fn screen_px_to_world_mm(px: f32, scale: f32) -> f32 {
    (px / scale.max(0.001)).max(0.01)
}

pub(super) fn to_rgba(color: Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

pub(super) fn circle_vertices(center: [f64; 2], radius: f32, segments: usize) -> Vec<[f32; 2]> {
    let segment_count = segments.max(12);
    let cx = center[0] as f32;
    let cy = center[1] as f32;
    let r = radius.max(0.01);

    (0..segment_count)
        .map(|step| {
            let theta = (step as f32 / segment_count as f32) * std::f32::consts::TAU;
            [cx + theta.cos() * r, cy + theta.sin() * r]
        })
        .collect()
}

/// Convert screen coords → world-mm via the camera, then snap to
/// the symbol-canvas grid. The canvas's Standard y-flip happens at
/// the world↔screen boundary inside `world_to_screen` /
/// `screen_to_world`; we mirror it here so screen-down → world-up.
pub(super) fn world_for(
    canvas: &SymbolCanvas<'_>,
    sx: f32,
    sy: f32,
    bounds: Rectangle,
) -> (f64, f64) {
    // The camera's screen_to_world doesn't know about y-flip — it
    // assumes screen and world share the same y-axis direction.
    // Symbol coords are Standard y-up; mirror by negating after.
    let world = canvas
        .camera
        .screen_to_world(iced::Point::new(sx, sy), bounds);
    let wx = world.x as f64;
    let wy = -world.y as f64;
    (
        (wx / SNAP_GRID_MM).round() * SNAP_GRID_MM,
        (wy / SNAP_GRID_MM).round() * SNAP_GRID_MM,
    )
}

/// Same as `world_for` but without the snap — used by the cursor
/// readout so the status footer shows the unsnapped position the
/// user actually pointed at.
pub(super) fn world_unsnapped(
    canvas: &SymbolCanvas<'_>,
    sx: f32,
    sy: f32,
    bounds: Rectangle,
) -> (f64, f64) {
    let world = canvas
        .camera
        .screen_to_world(iced::Point::new(sx, sy), bounds);
    (world.x as f64, -world.y as f64)
}

pub(super) fn selection_anchor(symbol: &Symbol, selection: &SymbolSelection) -> Option<(f64, f64)> {
    match selection {
        SymbolSelection::Pin(idx) => symbol
            .pins
            .get(*idx)
            .map(|pin| (pin.position[0], pin.position[1])),
        SymbolSelection::Graphic(idx) => {
            symbol
                .graphics
                .get(*idx)
                .map(|graphic| match &graphic.kind {
                    SymbolGraphicKind::Rectangle { from, .. }
                    | SymbolGraphicKind::Line { from, .. } => (from[0], from[1]),
                    SymbolGraphicKind::Circle { center, .. }
                    | SymbolGraphicKind::Arc { center, .. } => (center[0], center[1]),
                    SymbolGraphicKind::Text { position, .. } => (position[0], position[1]),
                })
        }
        SymbolSelection::Field(_) | SymbolSelection::All | SymbolSelection::Multiple { .. } => None,
    }
}
