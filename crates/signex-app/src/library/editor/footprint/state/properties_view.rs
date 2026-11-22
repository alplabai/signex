//! View state of the footprint Properties panel: the keys a refused pad
//! field keeps its text under. Not document data: changing it never
//! dirties the tab.

/// The pad form fields whose refused text is kept in
/// `FootprintEditorState::numeric_buffers`, so the field shows what was
/// typed with the reason it was not taken.
pub mod pad_field {
    pub const SIZE_X: &str = "size_x";
    pub const SIZE_Y: &str = "size_y";
    pub const ROTATION: &str = "rotation";
    pub const CORNER_RADIUS: &str = "corner_radius";
    pub const DRILL_DIAMETER: &str = "drill_diameter";
    pub const DRILL_SLOT_LENGTH: &str = "drill_slot_length";
    pub const HOLE_TOLERANCE_PLUS: &str = "hole_tolerance_plus";
    pub const HOLE_TOLERANCE_MINUS: &str = "hole_tolerance_minus";
    pub const COPPER_OFFSET_X: &str = "copper_offset_x";
    pub const COPPER_OFFSET_Y: &str = "copper_offset_y";
    pub const PASTE_TOP: &str = "paste_top";
    pub const PASTE_BOTTOM: &str = "paste_bottom";
    pub const MASK_TOP: &str = "mask_top";
    pub const MASK_BOTTOM: &str = "mask_bottom";
}

/// The `numeric_buffers` key of the footprint's refused Height text.
pub const FOOTPRINT_HEIGHT_BUFFER: &str = "footprint.height";

/// The `numeric_buffers` key of `field` on the selected pad `idx`, or on
/// the next pad placed when `idx` is `None`.
pub fn pad_field_buffer_key(idx: Option<usize>, field: &str) -> String {
    match idx {
        Some(idx) => format!("selected.{idx}.{field}"),
        None => format!("next.{field}"),
    }
}

/// The `numeric_buffers` key of `field` (a `pad_field` name) on the sketch
/// pad entity `id`.
pub fn sketch_pad_field_buffer_key(id: signex_sketch::id::SketchEntityId, field: &str) -> String {
    format!("sketch_pad.{}.{field}", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The selected pad and the next pad keep separate entries, and the
    /// drill key matches the one the pad form already read.
    #[test]
    fn selected_and_next_pad_fields_have_their_own_keys() {
        assert_eq!(
            pad_field_buffer_key(Some(3), pad_field::DRILL_DIAMETER),
            "selected.3.drill_diameter"
        );
        assert_eq!(
            pad_field_buffer_key(None, pad_field::DRILL_DIAMETER),
            "next.drill_diameter"
        );
        assert_ne!(
            pad_field_buffer_key(Some(0), pad_field::SIZE_X),
            pad_field_buffer_key(Some(1), pad_field::SIZE_X)
        );
    }
}
