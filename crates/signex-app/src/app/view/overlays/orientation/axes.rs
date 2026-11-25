//! Where a drawing's +X and +Y point on screen, and the caption under the
//! arrows (mockup row 194). Screen y grows down.

use signex_gfx::view_rotation::ViewRotation;
use signex_i18n::{I18n, menu_ids, t};
use signex_types::schematic::PageOrigin;

use crate::app::handlers::pcb_view_rotation::view_angle_text;

/// Unit screen directions of the drawing's +X and +Y.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app::view) struct ScreenAxes {
    pub x: [f32; 2],
    pub y: [f32; 2],
}

impl ScreenAxes {
    /// A board under `rotation`, the same turn and mirror the board canvas
    /// draws with. The board's +Y counts up from its origin (the model's y
    /// grows down the screen), so at 0° it points up.
    pub fn of_board(rotation: ViewRotation) -> Self {
        Self {
            x: rotation.apply([1.0, 0.0]),
            y: rotation.apply([0.0, -1.0]),
        }
    }

    /// A sheet whose shown Y counts from `origin`: up from a lower-left
    /// origin, down from an upper-left one. A sheet never turns.
    pub fn of_sheet(origin: PageOrigin) -> Self {
        let y = match origin {
            PageOrigin::LowerLeft => [0.0, -1.0],
            PageOrigin::UpperLeft => [0.0, 1.0],
        };
        Self { x: [1.0, 0.0], y }
    }
}

/// The caption row of a board view turned `degrees` and seen from below
/// when `flipped`: "90°", "Bottom", "Bottom · 90°"; none at rotation 0°
/// with Flip off.
pub(in crate::app::view) fn board_caption(
    i18n: &I18n,
    degrees: f32,
    flipped: bool,
) -> Option<String> {
    let angle = view_angle_text(degrees);
    let turned = angle != "0";
    let caption = match (flipped, turned) {
        (false, false) => return None,
        (false, true) => t!(i18n, menu_ids::ORIENTATION_TILE_ANGLE, "angle" => angle),
        (true, false) => t!(i18n, menu_ids::ORIENTATION_TILE_BOTTOM),
        (true, true) => t!(i18n, menu_ids::ORIENTATION_TILE_BOTTOM_ANGLE, "angle" => angle),
    };
    Some(caption.into_string())
}
