use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_menu_file_command(&mut self, msg: &MenuMessage) -> Option<Task<Message>> {
        match msg {
            MenuMessage::OpenProject => Some(Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Open Project or Schematic")
                        .add_filter("Signex Project", &["snxprj"])
                        .add_filter("Signex Schematic", &["snxsch"])
                        .add_filter("Signex Library", &["snxlib"])
                        .add_filter("All Supported", &["snxprj", "snxsch", "snxlib"])
                        .add_filter("All files", &["*"])
                        .pick_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                Message::FileOpened,
            )),
            MenuMessage::Save => Some(self.update(Message::SaveFile)),
            MenuMessage::SaveAs => Some(Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Save Schematic As")
                        .add_filter("Signex Schematic", &["snxsch"])
                        .save_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                |path| path.map(Message::SaveFileAs).unwrap_or(Message::Noop),
            )),
            MenuMessage::NewProject => Some(Task::none()),
            MenuMessage::PrintPreview => Some(self.update(Message::PrintPreviewRequested)),
            MenuMessage::ExportPdf => Some(self.update(Message::ExportPdfOpenDialog)),
            MenuMessage::ExportNetlist => self.handle_export_netlist_requested(),
            MenuMessage::ExportBom => Some(self.handle_bom_preview_open()),
            _ => None,
        }
    }
}
