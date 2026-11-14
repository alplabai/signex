//! Where the schematic canvas sits in the window, and where a floating card
//! may go inside it. Pure geometry, so the placement is tested without a
//! renderer.

use crate::dock::{MIN_CANVAS_WIDTH, MIN_SIDE_DOCK_WIDTH};
use iced::Rectangle;
use signex_types::theme::ThemeTokens;

/// Menu row, 1 px separator and tab strip above the canvas. The tab strip is
/// reserved even with no tab open (`view` pushes a placeholder). Its height
/// follows the theme: Graphite is taller.
pub(super) fn chrome_top(tokens: &ThemeTokens) -> f32 {
    crate::menu_bar::MENU_BAR_HEIGHT + CHROME_SEPARATOR + crate::tab_bar::tab_strip_height(tokens)
}
const CHROME_SEPARATOR: f32 = 1.0;
/// Resize handle drawn beside an open dock.
const HANDLE: f32 = crate::dock::DOCK_HANDLE;
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

/// Width a side dock may take in a `window_width` window beside the `other`
/// side dock: the other dock keeps its footprint and the canvas keeps
/// [`MIN_CANVAS_WIDTH`]. Never below [`MIN_SIDE_DOCK_WIDTH`].
pub(in crate::app) fn clamp_side_dock_width(
    width: f32,
    window_width: f32,
    other: DockExtent,
) -> f32 {
    let max =
        (window_width - other.footprint() - HANDLE - MIN_CANVAS_WIDTH).max(MIN_SIDE_DOCK_WIDTH);
    width.clamp(MIN_SIDE_DOCK_WIDTH, max)
}

/// Side dock widths `(left, right)` that leave the canvas its minimum in a
/// `window_width` window. The wider open dock gives up width first, so the
/// dock the user did not widen keeps its width; neither drops below
/// [`MIN_SIDE_DOCK_WIDTH`]. A closed or collapsed dock keeps its stored width.
pub(in crate::app) fn fit_side_docks(
    window_width: f32,
    left: DockExtent,
    right: DockExtent,
) -> (f32, f32) {
    let open = [is_open(left), is_open(right)];
    let mut sizes = [left, right].map(|dock| {
        if is_open(dock) {
            dock.size.max(MIN_SIDE_DOCK_WIDTH)
        } else {
            dock.size
        }
    });
    let footprint = |side: usize, dock: DockExtent| DockExtent {
        size: sizes[side],
        ..dock
    };
    let mut overflow =
        footprint(0, left).footprint() + footprint(1, right).footprint() + MIN_CANVAS_WIDTH
            - window_width;
    let order = if sizes[1] > sizes[0] { [1, 0] } else { [0, 1] };
    for side in order {
        if overflow <= 0.0 || !open[side] {
            continue;
        }
        let give = overflow.min((sizes[side] - MIN_SIDE_DOCK_WIDTH).max(0.0));
        sizes[side] -= give;
        overflow -= give;
    }
    (sizes[0], sizes[1])
}

fn is_open(dock: DockExtent) -> bool {
    dock.present && !dock.collapsed
}

/// The canvas rectangle in window coordinates.
pub(in crate::app) fn canvas_bounds(
    window: (f32, f32),
    left: DockExtent,
    right: DockExtent,
    bottom: DockExtent,
    tokens: &ThemeTokens,
) -> Rectangle {
    let top = chrome_top(tokens);
    let status = crate::status_bar::status_bar_height(tokens);
    let x = left.footprint();
    let width = (window.0 - x - right.footprint()).max(0.0);
    let height = (window.1 - top - status - bottom.footprint()).max(0.0);
    Rectangle {
        x,
        y: top,
        width,
        height,
    }
}

/// Where hover cards may go: the canvas in the main window (its docks are
/// the ones laid out), the whole window anywhere else.
pub(in crate::app) fn card_bounds(
    is_main_window: bool,
    canvas: Rectangle,
    window: (f32, f32),
) -> Rectangle {
    if is_main_window {
        canvas
    } else {
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: window.0,
            height: window.1,
        }
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

    /// The canvas must end where the status bar begins: the reserved
    /// height may not fall short of the real bar (the canvas would reach
    /// into it) nor overshoot it by a visible gap.
    #[test]
    fn status_bar_reservation_matches_the_laid_out_bar() {
        use signex_types::theme::ThemeId;
        for &id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(id);
            status_bar_reservation_matches(&format!("{id:?}"), tokens);
        }
    }

    /// The chrome sizes these layout pins are written against.
    fn graphite() -> ThemeTokens {
        signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark)
    }

    fn status_bar_reservation_matches(id: &str, tokens: ThemeTokens) {
        let reserved = crate::status_bar::status_bar_height(&tokens);
        let bar = crate::status_bar::view(
            0.0,
            0.0,
            true,
            true,
            100.0,
            signex_types::coord::Unit::Mm,
            crate::app::Tool::Select.to_string(),
            1.27,
            &[],
            &tokens,
            0,
            None,
            None,
            Default::default(),
            &signex_i18n::I18n::en_us(),
        );
        let real = crate::layout_probe::root_size::<crate::app::StatusBarRequest>(
            bar,
            iced::Size::new(1400.0, 900.0),
        )
        .height;
        assert!(
            (reserved - real).abs() < 0.5,
            "{id:?}: reserved {reserved} px but the status bar lays out {real} px"
        );
    }

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
        canvas_bounds((1400.0, 900.0), dock(240.0), dock(220.0), NONE, &graphite())
    }

    #[test]
    fn bounds_exclude_both_docks_and_their_handles() {
        let b = bounds();
        assert_eq!(b.x, 245.0);
        assert_eq!(b.width, 1400.0 - 245.0 - 225.0);
    }

    /// #2347: a side dock grows past the old 500 px cap until the canvas
    /// keeps its minimum beside the other dock.
    #[test]
    fn a_side_dock_grows_until_the_canvas_keeps_its_minimum() {
        let max = 2560.0 - (220.0 + HANDLE) - HANDLE - MIN_CANVAS_WIDTH;
        assert!(max > 500.0);
        assert_eq!(clamp_side_dock_width(5000.0, 2560.0, dock(220.0)), max);
        assert_eq!(clamp_side_dock_width(900.0, 2560.0, dock(220.0)), 900.0);
        let lone = clamp_side_dock_width(5000.0, 2560.0, NONE);
        assert_eq!(lone, 2560.0 - HANDLE - MIN_CANVAS_WIDTH);
    }

    #[test]
    fn a_side_dock_never_drops_below_its_minimum() {
        let min = MIN_SIDE_DOCK_WIDTH;
        assert_eq!(clamp_side_dock_width(10.0, 1400.0, dock(220.0)), min);
        assert_eq!(clamp_side_dock_width(400.0, 300.0, dock(220.0)), min);
    }

    /// #2347: a smaller window takes width from the wider dock first, so the
    /// dock the user left alone keeps its width.
    #[test]
    fn fitting_shrinks_the_wider_dock_first() {
        let (left, right) = fit_side_docks(1200.0, dock(240.0), dock(1500.0));
        assert_eq!(left, 240.0);
        assert_eq!(right, 1200.0 - (240.0 + HANDLE) - HANDLE - MIN_CANVAS_WIDTH);
        let (left, right) = fit_side_docks(1200.0, dock(1500.0), dock(220.0));
        assert_eq!(right, 220.0);
        assert_eq!(left, 1200.0 - (220.0 + HANDLE) - HANDLE - MIN_CANVAS_WIDTH);
    }

    #[test]
    fn fitting_takes_the_rest_from_the_other_dock_down_to_its_minimum() {
        let (left, right) = fit_side_docks(900.0, dock(400.0), dock(600.0));
        assert_eq!(right, MIN_SIDE_DOCK_WIDTH);
        assert!(left > MIN_SIDE_DOCK_WIDTH && left < 400.0, "left {left}");
        let canvas = 900.0 - (left + HANDLE) - (right + HANDLE);
        assert!((canvas - MIN_CANVAS_WIDTH).abs() < 0.01, "canvas {canvas}");
        let (left, right) = fit_side_docks(300.0, dock(400.0), dock(600.0));
        assert_eq!((left, right), (MIN_SIDE_DOCK_WIDTH, MIN_SIDE_DOCK_WIDTH));
    }

    #[test]
    fn fitting_leaves_a_window_with_room_and_a_collapsed_dock_alone() {
        assert_eq!(
            fit_side_docks(2560.0, dock(240.0), dock(900.0)),
            (240.0, 900.0)
        );
        let collapsed = DockExtent {
            collapsed: true,
            ..dock(1500.0)
        };
        let (left, right) = fit_side_docks(1200.0, collapsed, dock(220.0));
        assert_eq!((left, right), (1500.0, 220.0));
    }

    /// An open dock stored narrower than the minimum (an older layout) is
    /// widened to it, so its field never gets 0 px.
    #[test]
    fn fitting_widens_a_dock_below_the_minimum() {
        let (left, _) = fit_side_docks(2560.0, dock(100.0), dock(220.0));
        assert_eq!(left, MIN_SIDE_DOCK_WIDTH);
    }

    /// A Graphite theme reserves the taller 34 px strip and 26 px footer.
    #[test]
    fn graphite_bounds_use_the_graphite_strip_and_footer() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let b = canvas_bounds((1400.0, 900.0), dock(240.0), dock(220.0), NONE, &tokens);
        let m = crate::styles::ChromeMetrics::DEFAULT;
        assert_eq!(b.y, m.menu + 1.0 + m.tab_strip);
        assert_eq!(b.height, 900.0 - b.y - m.status);
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
        let b = canvas_bounds((500.0, 400.0), dock(240.0), dock(220.0), NONE, &graphite());
        let (x, _) = place_card((b.x + 10.0, 200.0), (260.0, 50.0), b);
        assert_eq!(x, b.x);
    }

    #[test]
    fn cards_are_kept_on_the_canvas_in_the_main_window_only() {
        let canvas = bounds();
        assert_eq!(card_bounds(true, canvas, (1400.0, 900.0)), canvas);
        let other = card_bounds(false, canvas, (800.0, 600.0));
        assert_eq!(
            other,
            Rectangle {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0
            }
        );
        // Trunk's clamp: a card near the right edge of a 800 px window.
        let (x, _) = place_card((700.0, 300.0), (260.0, 100.0), other);
        assert_eq!(x, 700.0 - POINTER_GAP - 260.0);
    }

    #[test]
    fn a_label_maps_to_its_window_point_under_the_default_dock_layout() {
        // Default docks: 240 px left + 5 px handle, so the canvas starts at
        // x = 245; menu bar + separator 1 + the tab strip put it at y below.
        let top = crate::styles::ChromeMetrics::DEFAULT.menu
            + 1.0
            + crate::tab_bar::tab_strip_height(&graphite());
        let at = world_to_window((10.0, 20.0), (5.0, 7.0, 3.0), bounds());
        assert_eq!(at, (245.0 + 35.0, top + 67.0));
    }
}
