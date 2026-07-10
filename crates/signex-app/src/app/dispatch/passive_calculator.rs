use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn handle_open_passive_calculator(&mut self) -> Task<Message> {
        if let Some(id) = self.ui_state.windows.iter().find_map(|(id, kind)| {
            matches!(kind, crate::app::state::WindowKind::PassiveCalculator).then_some(*id)
        }) {
            return iced::window::gain_focus(id);
        }

        let (_id, open_task) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(1120.0, 720.0),
            min_size: Some(iced::Size::new(860.0, 560.0)),
            ..Default::default()
        });
        open_task.map(Message::PassiveCalculatorOpened)
    }
}
