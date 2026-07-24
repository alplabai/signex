use iced::Task;

use super::*;

impl Signex {
    pub(crate) fn open_pcb_trace_calculator(&mut self) -> Task<Message> {
        if self
            .ui_state
            .windows
            .values()
            .any(|kind| matches!(kind, super::state::WindowKind::PcbTraceCalculator))
        {
            return Task::none();
        }

        let (id, open_task) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(760.0, 680.0),
            resizable: true,
            decorations: true,
            ..Default::default()
        });
        self.ui_state
            .windows
            .insert(id, super::state::WindowKind::PcbTraceCalculator);
        open_task.map(|id| Message::Window(WindowMsg::PcbTraceCalculatorOpened(id)))
    }

    pub(crate) fn dispatch_pcb_trace_calculator_message(
        &mut self,
        message: crate::pcb_trace_calculator::PcbTraceCalculatorMessage,
    ) -> Task<Message> {
        self.ui_state.pcb_trace_calculator.update(message);
        Task::none()
    }
}
