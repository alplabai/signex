use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_document_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FileOpened(path) => {
                self.handle_document_file_opened(path);
                self.finish_update()
            }
            Message::DeleteSelected => {
                self.handle_selection_delete_requested();
                self.finish_update()
            }
            Message::Undo => {
                self.handle_undo_requested();
                self.finish_update()
            }
            Message::Redo => {
                self.handle_redo_requested();
                self.finish_update()
            }
            Message::RotateSelected => {
                self.handle_selection_rotate_requested();
                self.finish_update()
            }
            Message::MirrorSelectedX => {
                self.handle_selection_mirror_x_requested();
                self.finish_update()
            }
            Message::MirrorSelectedY => {
                self.handle_selection_mirror_y_requested();
                self.finish_update()
            }
            Message::Cut => self.handle_selection_cut_requested(),
            Message::Copy => {
                self.handle_selection_copy_requested();
                self.finish_update()
            }
            Message::Paste => {
                self.handle_clipboard_paste_requested();
                self.finish_update()
            }
            Message::SmartPaste => {
                self.handle_clipboard_smart_paste_requested();
                self.finish_update()
            }
            Message::Duplicate => {
                self.handle_selection_duplicate_requested();
                self.finish_update()
            }
            Message::SaveFile => {
                self.handle_active_document_save_requested();
                self.finish_update()
            }
            Message::SaveFileAs(path) => {
                self.handle_active_document_save_as_requested(path);
                self.finish_update()
            }
            Message::SchematicLoaded(sheet) => {
                self.load_schematic_into_active_tab(*sheet);
                self.finish_update()
            }
            _ => unreachable!("dispatch_document_message received non-document message"),
        }
    }
}