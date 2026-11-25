//! Home View (mockup row 194): the Home button by the zoom pill, View ▸
//! Home View and `home_view`. View state only, like pan and zoom: no undo
//! step and no dirty mark.

use iced::Task;

use crate::app::{Message, PcbViewMsg, Signex, ViewRotationMsg};
use crate::canvas::CanvasEvent;

impl Signex {
    /// Bring the main window's view home: on a board, rotation 0° and Flip
    /// off, then Fit (Fit keeps the turn and the flip; Home does not). A
    /// sheet never turns, so there it is Fit.
    pub(crate) fn go_home_view(&mut self) -> Task<Message> {
        let mut turned = Task::none();
        if self.has_active_pcb() {
            if self.pcb_layer_view_of_window(None).flipped() {
                self.apply_pcb_view_event(None, PcbViewMsg::BoardFlipToggled);
            }
            turned = self.apply_view_rotation(None, ViewRotationMsg::AngleSet(0.0));
        }
        let fit = self.update(Message::CanvasEvent(CanvasEvent::FitAll));
        Task::batch([turned, fit])
    }
}
