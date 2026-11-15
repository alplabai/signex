//! How wide a side dock may get (#2347): the canvas keeps a minimum width,
//! and a dock never gets so narrow that a property row's field has no room.

use crate::panels::{PROPERTY_LABEL_WIDTH, PROPERTY_ROW_PAD_X};
use crate::styles::{Spacing, TypeScale};

const BODY: f32 = TypeScale::DEFAULT.body;

/// Resize handle drawn beside an open dock.
pub const DOCK_HANDLE: f32 = 5.0;

/// Narrowest the canvas may get from widening a side dock: 30 UI-font ems.
pub const MIN_CANVAS_WIDTH: f32 = BODY * 30.0;

/// Narrowest field a property row keeps: 6 UI-font ems.
const MIN_FIELD_WIDTH: f32 = BODY * 6.0;

/// Narrowest a side dock may be: a property row's fixed label column, its
/// padding on both sides, the widest label-to-field gap (`Spacing::m`) and
/// the narrowest field.
pub const MIN_SIDE_DOCK_WIDTH: f32 =
    PROPERTY_LABEL_WIDTH + 2.0 * PROPERTY_ROW_PAD_X as f32 + Spacing::DEFAULT.m + MIN_FIELD_WIDTH;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_narrowest_dock_still_leaves_the_field_its_minimum() {
        let row_chrome = PROPERTY_LABEL_WIDTH + 2.0 * PROPERTY_ROW_PAD_X as f32;
        let field = MIN_SIDE_DOCK_WIDTH - row_chrome - Spacing::DEFAULT.m;
        assert!(field >= BODY * 6.0 - 0.01, "field gets {field} px");
    }
}
