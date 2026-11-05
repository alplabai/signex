//! Classification of canvas events by origin.

use super::CanvasEvent;

impl CanvasEvent {
    /// Whether the event is the user acting on the canvas with the pointer:
    /// a click, a box select, a drag-move, a wheel zoom or a pan. A hover,
    /// and the fit / centre commands a key or the menu sends, are not.
    pub fn is_user_gesture(&self) -> bool {
        matches!(
            self,
            Self::Clicked { .. }
                | Self::CtrlClicked { .. }
                | Self::DoubleClicked { .. }
                | Self::BoxSelect { .. }
                | Self::MoveSelected { .. }
                | Self::MovePicked { .. }
                | Self::ViewMoved
        )
    }
}
