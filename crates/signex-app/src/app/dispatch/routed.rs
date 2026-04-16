use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_routed_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Menu(msg) => self.handle_menu_message(msg),
            Message::Tab(msg) => {
                self.handle_document_tab_message(msg);
                self.finish_update()
            }
            Message::Dock(msg) => self.handle_dock_message(msg),
            Message::Selection(msg) => self.handle_selection_message(msg),
            _ => unreachable!("dispatch_routed_message received non-routed message"),
        }
    }
}