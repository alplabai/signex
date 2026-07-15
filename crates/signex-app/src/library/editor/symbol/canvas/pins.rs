//! Pin-rendering helpers — the sheet-derived colour palette, the pin
//! text layout constants, and the per-pin render geometry. Pure code
//! motion out of `mod.rs`; consumed by `build_symbol_renderer_snapshot`
//! and `SymbolCanvas::new` (both still in the parent `canvas` module),
//! so the types carry `pub(super)` visibility.

use super::*;
use signex_types::anchor2d::rotate_vec;
use signex_types::rotation2d::Vec2d;

/// Palette derived from the active sheet colour — picks a content
/// foreground that reads correctly on the sheet bg. Two flavours:
/// dark-on-light (Cream / White / LightGray) and light-on-dark
/// (Black / DarkGray). Mirrors Altium's per-sheet contrast rule.
pub(super) struct SymbolPalette {
    pub(super) body: Color,
    pub(super) pin: Color,
    pub(super) text: Color,
    pub(super) grid: Color,
    /// Slight stroke for axis lines through (0, 0).
    pub(super) axis: Color,
}

impl SymbolPalette {
    pub(super) fn for_sheet(sheet: Color) -> Self {
        // Rec. 601 luma — perceptually-weighted brightness.
        let luma = 0.299 * sheet.r + 0.587 * sheet.g + 0.114 * sheet.b;
        if luma > 0.5 {
            // Light sheet: dark text + the Altium signature blue body.
            Self {
                body: Color::from_rgb(0.10, 0.20, 0.55),
                pin: Color::from_rgb(0.10, 0.10, 0.10),
                text: Color::from_rgb(0.10, 0.10, 0.10),
                grid: Color::from_rgba(0.00, 0.00, 0.00, 0.18),
                axis: Color::from_rgba(0.00, 0.00, 0.00, 0.45),
            }
        } else {
            // Dark sheet: keep the warm yellow body + light text.
            Self {
                body: Color::from_rgb(0.95, 0.78, 0.30),
                pin: Color::from_rgb(0.85, 0.88, 0.92),
                text: Color::from_rgb(0.85, 0.88, 0.92),
                grid: Color::from_rgba(1.0, 1.0, 1.0, 0.12),
                axis: Color::from_rgba(1.0, 1.0, 1.0, 0.35),
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PinTextLayout {
    // Pin number (physical number) on/above the pin line.
    pub(super) number_size_mm: f32,
    pub(super) pin_pitch_mm: f32,
    pub(super) number_offset_ratio_of_pitch: f32,
    pub(super) number_along_ratio: f32,
    // Pin name label near the symbol body edge.
    pub(super) name_size_mm: f32,
    pub(super) name_offset_x_mm: f32,
    pub(super) name_offset_y_mm: f32,
}

pub(super) const PIN_TEXT_LAYOUT: PinTextLayout = PinTextLayout {
    number_size_mm: 1.27,
    pin_pitch_mm: 2.54,
    number_offset_ratio_of_pitch: 0.10,
    number_along_ratio: 0.18,
    name_size_mm: 1.27,
    name_offset_x_mm: 0.50,
    name_offset_y_mm: 0.00,
};

/// Approximate glyph advance as a fraction of the text size — used to
/// estimate a label's rendered width for hit-testing. Rough but
/// generous; combined with `LABEL_HIT_PAD_MM` it errs toward an
/// easy-to-grab box without being wildly oversized.
const CHAR_W_RATIO: f64 = 0.62;

/// Padding added on every side of a label hit-box (world mm) so
/// grabbing a pin by its text is forgiving.
const LABEL_HIT_PAD_MM: f64 = 0.4;

/// A world-mm axis-aligned bounding box used for label hit-testing.
/// A degenerate box (an empty label) sets `min > max` so it can never
/// contain any point.
pub(super) struct Aabb {
    pub min: [f64; 2],
    pub max: [f64; 2],
}

impl Aabb {
    /// True when `(x, y)` lies inside (or on the edge of) the box.
    pub(super) fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.min[0] && x <= self.max[0] && y >= self.min[1] && y <= self.max[1]
    }

    /// A degenerate, never-containing box — the guard for empty labels.
    fn empty() -> Self {
        Self {
            min: [f64::INFINITY, f64::INFINITY],
            max: [f64::NEG_INFINITY, f64::NEG_INFINITY],
        }
    }
}

/// Pre-computed render geometry for one pin.
///
/// All positions are in world-mm. Derived once per frame from the pin's
/// `position`, `orientation`, and `length` so that
/// `build_symbol_renderer_snapshot` contains only push calls.
pub(super) struct PinRenderGeometry {
    pub(super) tip: Vec2d,
    pub(super) body_end: Vec2d,
    pub(super) number_pos: Vec2d,
    pub(super) name_pos: Vec2d,
    pub(super) text_rotation: f32,
    pub(super) name_h_align: HAlign,
}

impl PinRenderGeometry {
    pub(super) fn compute(pin: &SymbolPin) -> Self {
        use signex_library::PinOrientation;
        use std::f64::consts::FRAC_PI_2;

        // Orientation → angle (CCW from +x axis), tip → body direction.
        let angle_rad: f64 = match pin.orientation {
            PinOrientation::Right => 0.0,
            PinOrientation::Up => FRAC_PI_2,
            PinOrientation::Left => std::f64::consts::PI,
            PinOrientation::Down => -FRAC_PI_2,
            _ => std::f64::consts::PI,
        };

        let tip = Vec2d::new(pin.position[0], pin.position[1]);
        let unit = rotate_vec(Vec2d::new(1.0, 0.0), angle_rad);
        let body_end = Vec2d::new(tip.x + unit.x * pin.length, tip.y + unit.y * pin.length);

        // Outer normal: 90° CCW from unit = (-unit.y, unit.x).
        // Pick the side that is visually "outer": prefer +y, break ties with -x.
        let n_ccw = rotate_vec(unit, FRAC_PI_2);
        let n_cw = Vec2d::new(-n_ccw.x, -n_ccw.y);
        let normal = if (n_ccw.y - n_cw.y).abs() > f64::EPSILON {
            if n_ccw.y > n_cw.y { n_ccw } else { n_cw }
        } else if n_ccw.x < n_cw.x {
            n_ccw
        } else {
            n_cw
        };

        // Text rotation for iced screen space.
        //
        // World coordinates are Y-up; iced canvas is Y-down. The world→screen
        // transform is: screen_y = oy − world_y × scale, which negates the Y
        // component. To make text align with the pin direction in screen space
        // we must negate unit.y: atan2(−uy, ux).
        //
        // Normalize to (−π/2, π/2] so text is never upside-down.
        // Use strict `<` for the lower bound so that Up-pin angle (exactly
        // −π/2) is kept as-is — it makes text flow upward on screen, which
        // is the correct readable direction for Up pins.
        let mut text_rotation = (-(unit.y as f32)).atan2(unit.x as f32);
        let flipped: bool;
        if text_rotation > std::f32::consts::FRAC_PI_2 {
            text_rotation -= std::f32::consts::PI;
            flipped = true;
        } else if text_rotation < -std::f32::consts::FRAC_PI_2 {
            text_rotation += std::f32::consts::PI;
            flipped = true;
        } else {
            flipped = false;
        }
        // When flipped (only Left-facing pins after normalization), the text's
        // local +x axis points opposite the pin direction — reverse h_align so
        // the name still extends away from the tip.
        let name_h_align = if flipped { HAlign::Right } else { HAlign::Left };

        let number_offset_mm = PIN_TEXT_LAYOUT.pin_pitch_mm as f64
            * PIN_TEXT_LAYOUT.number_offset_ratio_of_pitch as f64;
        let along_mm = pin.length * PIN_TEXT_LAYOUT.number_along_ratio as f64;

        let number_pos = Vec2d::new(
            tip.x + unit.x * along_mm + normal.x * number_offset_mm,
            tip.y + unit.y * along_mm + normal.y * number_offset_mm,
        );
        let name_pos = Vec2d::new(
            tip.x + unit.x * (pin.length + PIN_TEXT_LAYOUT.name_offset_x_mm as f64),
            tip.y + unit.y * (pin.length + PIN_TEXT_LAYOUT.name_offset_x_mm as f64),
        );

        Self {
            tip,
            body_end,
            number_pos,
            name_pos,
            text_rotation,
            name_h_align,
        }
    }

    /// Axis-aligned world-mm hit-boxes for the pin's NUMBER and NAME
    /// labels (in that order), so a pin can be grabbed by its text and
    /// not only by its tip. Reuses `number_pos` / `name_pos` from
    /// `compute` — no offset math is duplicated here.
    ///
    /// Pins are only oriented Up/Down/Left/Right, so the text runs
    /// horizontally (Left/Right pins) or vertically (Up/Down pins). The
    /// box is centred on the label anchor and grown by half its extent
    /// on each axis, swapping width/height for vertical text. An empty
    /// label yields a degenerate box that never hits.
    pub(super) fn label_hit_boxes(&self, pin: &SymbolPin) -> [Aabb; 2] {
        use signex_library::PinOrientation;

        let horizontal = matches!(
            pin.orientation,
            PinOrientation::Left | PinOrientation::Right
        );
        let number_box = Self::text_box(
            &pin.number,
            self.number_pos,
            PIN_TEXT_LAYOUT.number_size_mm as f64,
            horizontal,
        );
        let name_box = Self::text_box(
            &pin.name,
            self.name_pos,
            PIN_TEXT_LAYOUT.name_size_mm as f64,
            horizontal,
        );
        [number_box, name_box]
    }

    /// One label's hit-box: estimate the rendered width from the glyph
    /// count, floor it at `size_mm` so a single-char label stays
    /// grabbable, add padding, then centre on `anchor`. `horizontal`
    /// selects which axis the text width runs along.
    fn text_box(text: &str, anchor: Vec2d, size_mm: f64, horizontal: bool) -> Aabb {
        if text.is_empty() {
            return Aabb::empty();
        }
        let chars = text.chars().count() as f64;
        let width = (chars * size_mm * CHAR_W_RATIO).max(size_mm);
        let height = size_mm;
        // Half-extents along world x / y. Text width runs along x for
        // horizontal pins, along y for vertical pins.
        // Full text width as the along-run half-extent (not width/2):
        // the name is drawn to ONE side of its anchor (h_align
        // Left/Right), so a centered half-width box would miss the far
        // half of a long name. The extra coverage falls toward the pin
        // body — harmless, still the same pin.
        let (half_x, half_y) = if horizontal {
            (width, height / 2.0)
        } else {
            (height / 2.0, width)
        };
        Aabb {
            min: [
                anchor.x - half_x - LABEL_HIT_PAD_MM,
                anchor.y - half_y - LABEL_HIT_PAD_MM,
            ],
            max: [
                anchor.x + half_x + LABEL_HIT_PAD_MM,
                anchor.y + half_y + LABEL_HIT_PAD_MM,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_boxes_grab_their_anchor_and_reject_far_points() {
        // Right-facing pin (default) with a name + number.
        let pin = SymbolPin::new("1", "VCC");
        let geom = PinRenderGeometry::compute(&pin);
        let [number_box, name_box] = geom.label_hit_boxes(&pin);

        // The name box grabs the name anchor…
        assert!(name_box.contains(geom.name_pos.x, geom.name_pos.y));
        // …but not a point 50 mm away.
        assert!(!name_box.contains(geom.name_pos.x + 50.0, geom.name_pos.y));
        // The number box grabs the number anchor.
        assert!(number_box.contains(geom.number_pos.x, geom.number_pos.y));
    }

    #[test]
    fn empty_label_box_never_hits() {
        // A pin with no name — the name box must be un-clickable.
        let pin = SymbolPin::new("1", "");
        let geom = PinRenderGeometry::compute(&pin);
        let [_, name_box] = geom.label_hit_boxes(&pin);
        assert!(!name_box.contains(geom.name_pos.x, geom.name_pos.y));
    }
}
