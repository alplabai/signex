/// Kind of z-order picker currently armed. Drives the first-click
/// resolve in `handle_canvas_left_click`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReorderPicker {
    /// Move selection to render just above the clicked reference.
    Above,
    /// Move selection to render just below the clicked reference.
    Below,
}
