//! Where the schematic canvas sits in the window, and where a floating card
//! may go inside it. Pure geometry, so the placement is tested without a
//! renderer.

use iced::Rectangle;

/// Menu row, 1 px separator and tab strip above the canvas. The tab strip is
/// reserved even with no tab open (`view` pushes a placeholder).
pub(super) const CHROME_TOP: f32 =
    crate::menu_bar::MENU_BAR_HEIGHT + CHROME_SEPARATOR + crate::tab_bar::TAB_STRIP_HEIGHT;
const CHROME_SEPARATOR: f32 = 1.0;
/// Room taken by the status bar under the docks; its height follows its
/// content, so this is the measured default.
const STATUS_BAR: f32 = 24.0;
/// Resize handle drawn beside an open dock.
const HANDLE: f32 = 5.0;
/// A collapsed dock keeps this strip.
const COLLAPSED: f32 = 28.0;
/// Gap between the pointer and a card, so the card never sits under it.
const POINTER_GAP: f32 = 16.0;

/// One dock's footprint beside the canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app) struct DockExtent {
    pub present: bool,
    pub collapsed: bool,
    pub size: f32,
}

impl DockExtent {
    /// Width (or height) the dock and its resize handle take.
    pub fn footprint(self) -> f32 {
        match (self.present, self.collapsed) {
            (false, _) => 0.0,
            (true, true) => COLLAPSED,
            (true, false) => self.size + HANDLE,
        }
    }
}

/// The canvas rectangle in window coordinates.
pub(in crate::app) fn canvas_bounds(
    window: (f32, f32),
    left: DockExtent,
    right: DockExtent,
    bottom: DockExtent,
) -> Rectangle {
    let x = left.footprint();
    let width = (window.0 - x - right.footprint()).max(0.0);
    let height = (window.1 - CHROME_TOP - STATUS_BAR - bottom.footprint()).max(0.0);
    Rectangle {
        x,
        y: CHROME_TOP,
        width,
        height,
    }
}

/// Top-left of a `size` card shown beside `pointer`, kept inside `bounds`:
/// below-right of the pointer, flipped to the other side at an edge, then
/// clamped when it fits on neither.
pub(in crate::app) fn place_card(
    pointer: (f32, f32),
    size: (f32, f32),
    bounds: Rectangle,
) -> (f32, f32) {
    (
        place_axis(pointer.0, size.0, bounds.x, bounds.x + bounds.width),
        place_axis(pointer.1, size.1, bounds.y, bounds.y + bounds.height),
    )
}

fn place_axis(pointer: f32, extent: f32, min: f32, max: f32) -> f32 {
    let mut at = pointer + POINTER_GAP;
    if at + extent > max {
        at = pointer - POINTER_GAP - extent;
    }
    at.min(max - extent).max(min)
}

/// Window position of the inline editor for a point at `world` mm, given the
/// camera `(offset_x, offset_y, scale)` and the canvas rectangle.
pub(in crate::app) fn world_to_window(
    world: (f64, f64),
    camera: (f32, f32, f32),
    bounds: Rectangle,
) -> (f32, f32) {
    (
        bounds.x + world.0 as f32 * camera.2 + camera.0,
        bounds.y + world.1 as f32 * camera.2 + camera.1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dock(size: f32) -> DockExtent {
        DockExtent {
            present: true,
            collapsed: false,
            size,
        }
    }
    const NONE: DockExtent = DockExtent {
        present: false,
        collapsed: false,
        size: 0.0,
    };

    fn bounds() -> Rectangle {
        canvas_bounds((1400.0, 900.0), dock(240.0), dock(220.0), NONE)
    }

    #[test]
    fn bounds_exclude_both_docks_and_their_handles() {
        let b = bounds();
        assert_eq!(b.x, 245.0);
        assert_eq!(b.width, 1400.0 - 245.0 - 225.0);
        assert_eq!(b.y, 63.0);
        assert_eq!(b.height, 900.0 - 63.0 - STATUS_BAR);
    }

    #[test]
    fn a_card_near_the_right_edge_flips_left_of_the_pointer() {
        let b = bounds();
        let right = b.x + b.width;
        let pointer = (1100.0, 300.0);
        assert!(pointer.0 < right);
        let (x, _) = place_card(pointer, (260.0, 100.0), b);
        assert!(x + 260.0 <= right, "card spills past the canvas: x={x}");
        assert!(x < pointer.0, "card should sit left of the pointer");
    }

    #[test]
    fn a_card_in_open_space_sits_below_right_of_the_pointer() {
        let (x, y) = place_card((500.0, 300.0), (260.0, 100.0), bounds());
        assert_eq!((x, y), (516.0, 316.0));
    }

    #[test]
    fn a_card_near_the_bottom_edge_flips_above_the_pointer() {
        let b = bounds();
        let (_, y) = place_card((500.0, b.y + b.height - 10.0), (260.0, 100.0), b);
        assert!(y + 100.0 <= b.y + b.height);
    }

    #[test]
    fn a_card_wider_than_the_canvas_is_clamped_to_its_left_edge() {
        let b = canvas_bounds((500.0, 400.0), dock(240.0), dock(220.0), NONE);
        let (x, _) = place_card((b.x + 10.0, 200.0), (260.0, 50.0), b);
        assert_eq!(x, b.x);
    }

    #[test]
    fn a_label_maps_to_its_window_point_under_the_default_dock_layout() {
        // Default docks: 240 px left + 5 px handle, so the canvas starts at
        // x = 245; menu 36 + separator 1 + tab strip 26 put it at y = 63.
        let at = world_to_window((10.0, 20.0), (5.0, 7.0, 3.0), bounds());
        assert_eq!(at, (245.0 + 35.0, 63.0 + 67.0));
    }
}
