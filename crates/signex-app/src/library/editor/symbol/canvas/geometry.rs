//! Free math / coordinate helpers for the symbol canvas — text and
//! stroke sizing at zoom, angle unwrapping, screen↔world conversion
//! with the canvas snap grid, colour packing, circle tessellation,
//! and selection-anchor lookup. Pure code motion out of `mod.rs`;
//! `pub(super)` so both the parent `canvas` module and its `input` /
//! `draw` submodules can reach them.

use super::*;

/// Rendered em size for hit-testing — the same rule the draw path uses.
///
/// This used to be a second copy of the sizing arithmetic with the clamp
/// bounds written out again. The two agreed, so nothing was visibly broken;
/// what they were was two places to edit. Change the draw path's bounds and
/// the hit box silently keeps the old ones, and a click near a pin label
/// starts landing where the text no longer is.
pub(super) fn text_size_px_from_mm(size_mm: f32, scale: f32) -> f32 {
    signex_gfx::primitive::text::text_px(size_mm, scale, SYMBOL_TEXT_SIZE)
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

/// World width (mm) a symbol stroke stored as `width_mm` draws at, thickened
/// by [`SELECTED_STROKE_FACTOR`] while selected. The schematic's law (#1858);
/// the thickening applies after the floor, so a selected stroke stays wider
/// when zoomed out.
pub(super) fn symbol_stroke_mm(width_mm: f64, selected: bool, scale: f32) -> f32 {
    let mm = signex_types::schematic::effective_stroke_mm(width_mm) as f32;
    if selected {
        stroke_px(mm, scale) * SELECTED_STROKE_FACTOR / scale.max(f32::MIN_POSITIVE)
    } else {
        mm
    }
}

/// Screen width of a stroke drawn straight to the frame (previews), under
/// the same law the scene replay applies.
pub(super) fn stroke_px(width_mm: f32, scale: f32) -> f32 {
    signex_types::schematic::stroke_px(
        width_mm,
        scale,
        signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    )
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

pub(super) fn selection_anchor(
    symbol: &Symbol,
    selection: &SymbolSelection,
    view: state::PartView,
) -> Option<(f64, f64)> {
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
                    SymbolGraphicKind::Polygon { vertices } => {
                        let c = state::polygon_centroid(vertices);
                        (c[0], c[1])
                    }
                })
        }
        SymbolSelection::Field(key) => {
            let [x, y] = state::field_pos(symbol, *key, view);
            Some((x, y))
        }
        SymbolSelection::All | SymbolSelection::Multiple { .. } => None,
    }
}
