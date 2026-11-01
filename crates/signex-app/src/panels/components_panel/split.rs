//! The divider between the component list and the details section.
//!
//! The list height lives in `ComponentsPanelState::list_height`; the details
//! section takes the rest. Sizes derive from the row text size.

use super::ROW_TEXT_SIZE;

/// Smallest list height: a few rows stay visible.
pub const MIN_LIST_HEIGHT: f32 = ROW_TEXT_SIZE * 8.0;
/// Largest list height, so the details section never vanishes.
pub const MAX_LIST_HEIGHT: f32 = ROW_TEXT_SIZE * 60.0;
/// List height before the user drags the divider.
pub const DEFAULT_LIST_HEIGHT: f32 = ROW_TEXT_SIZE * 25.0;
/// Thickness of the draggable divider.
pub const HANDLE_HEIGHT: f32 = ROW_TEXT_SIZE * 0.5;

/// `height` limited to the allowed range.
pub fn clamp_list_height(height: f32) -> f32 {
    height.clamp(MIN_LIST_HEIGHT, MAX_LIST_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_clamp_to_the_range() {
        assert_eq!(clamp_list_height(0.0), MIN_LIST_HEIGHT);
        assert_eq!(clamp_list_height(1.0e6), MAX_LIST_HEIGHT);
        assert_eq!(clamp_list_height(DEFAULT_LIST_HEIGHT), DEFAULT_LIST_HEIGHT);
    }
}
