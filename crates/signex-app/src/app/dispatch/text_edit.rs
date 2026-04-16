use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_text_edit_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TextEditChanged(text) => {
                if let Some(ref mut state) = self.interaction_state.editing_text {
                    state.text = text;
                }
                self.finish_update()
            }
            Message::TextEditSubmit => {
                if let Some(state) = self.interaction_state.editing_text.take()
                    && state.text != state.original_text
                {
                    let engine_command = match state.kind {
                        signex_types::schematic::SelectedKind::Label => signex_engine::Command::UpdateText {
                            target: signex_engine::TextTarget::Label(state.uuid),
                            value: state.text.clone(),
                        },
                        signex_types::schematic::SelectedKind::TextNote => signex_engine::Command::UpdateText {
                            target: signex_engine::TextTarget::TextNote(state.uuid),
                            value: state.text.clone(),
                        },
                        _ => return Task::none(),
                    };
                    self.apply_engine_command(engine_command, false, true);
                }
                self.finish_update()
            }
            _ => unreachable!("dispatch_text_edit_message received non-text-edit message"),
        }
    }
}