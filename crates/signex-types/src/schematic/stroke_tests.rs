use super::*;
use crate::schematic::SCHEMATIC_ZOOM_100_SCALE;

const ZOOMS_PERCENT: [f32; 7] = [25.0, 50.0, 100.0, 200.0, 400.0, 800.0, 1600.0];

#[test]
fn wire_pin_body_ratio_is_the_same_at_every_zoom_above_the_floor() {
    let (wire, pin, body) = (0.15_f32, 0.254_f32, 0.254_f32);
    for zoom in ZOOMS_PERCENT {
        let scale = SCHEMATIC_ZOOM_100_SCALE * zoom / 100.0;
        let px = |mm| stroke_px(mm, scale, UNSCALED_DEVICE_PX_PER_LOGICAL_PX);
        let floor = min_stroke_logical_px(UNSCALED_DEVICE_PX_PER_LOGICAL_PX);
        if wire * scale >= floor {
            assert_eq!(px(wire), wire * scale, "wire at {zoom}%");
            assert_eq!(px(pin), pin * scale, "pin at {zoom}%");
            assert_eq!(px(body), body * scale, "body at {zoom}%");
        }
    }
}

#[test]
fn the_floor_is_one_value_for_every_kind() {
    let scale = SCHEMATIC_ZOOM_100_SCALE * 0.25;
    let floored: Vec<f32> = [
        SCHEMATIC_RENDER_MIN_STROKE_MM,
        SCHEMATIC_NO_CONNECT_STROKE_MM,
        SCHEMATIC_LABEL_MARKER_STROKE_MM,
        SCHEMATIC_SHEET_FRAME_STROKE_MM,
        0.0,
    ]
    .into_iter()
    .map(|mm| stroke_px(mm as f32, scale, UNSCALED_DEVICE_PX_PER_LOGICAL_PX))
    .collect();
    assert!(
        floored
            .iter()
            .all(|px| *px == SCHEMATIC_MIN_STROKE_DEVICE_PX)
    );
}

#[test]
fn the_floor_is_in_device_pixels() {
    assert_eq!(
        min_stroke_logical_px(2.0),
        SCHEMATIC_MIN_STROKE_DEVICE_PX / 2.0
    );
    assert_eq!(
        stroke_px(0.0, 3.0, 2.0) * 2.0,
        SCHEMATIC_MIN_STROKE_DEVICE_PX
    );
    assert_eq!(min_stroke_logical_px(0.0), SCHEMATIC_MIN_STROKE_DEVICE_PX);
    assert_eq!(
        min_stroke_logical_px(f32::NAN),
        SCHEMATIC_MIN_STROKE_DEVICE_PX
    );
}

#[test]
fn effective_width_defaults_a_zero_or_broken_width() {
    assert_eq!(effective_stroke_mm(0.0), SCHEMATIC_RENDER_MIN_STROKE_MM);
    assert_eq!(
        effective_stroke_mm(f64::NAN),
        SCHEMATIC_RENDER_MIN_STROKE_MM
    );
    assert_eq!(effective_stroke_mm(0.254), 0.254);
}

#[test]
fn a_symbol_strokes_at_its_most_common_library_width() {
    assert_eq!(symbol_stroke_mm([0.254, 0.254, 0.5]), 0.254);
    assert_eq!(symbol_stroke_mm([0.254, 0.5]), 0.5, "a tie takes the wider");
    assert_eq!(
        symbol_stroke_mm([0.0, 0.0, 0.254]),
        SCHEMATIC_RENDER_MIN_STROKE_MM
    );
    assert_eq!(symbol_stroke_mm([]), SCHEMATIC_RENDER_MIN_STROKE_MM);
}
