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
            Message::ExportPdfFinished(result) => {
                let task = self.handle_export_pdf_finished(result);
                iced::Task::batch([task, self.finish_update()])
            }
            Message::ExportNetlistFinished(result) => {
                let task = self.handle_export_netlist_finished(result);
                iced::Task::batch([task, self.finish_update()])
            }
            Message::PrintPreviewRequested => {
                let task = self.handle_print_preview_requested();
                iced::Task::batch([task, self.finish_update()])
            }
            Message::PrintPreviewSelectPage(idx) => {
                self.handle_print_preview_select_page(idx);
                self.finish_update()
            }
            Message::PrintPreviewSetColourMode(mode) => {
                self.handle_print_preview_set_colour_mode(mode);
                self.finish_update()
            }
            Message::PrintPreviewSetPageRangeAll => {
                self.handle_print_preview_set_page_range_all();
                self.finish_update()
            }
            Message::PrintPreviewSetPageRangeCurrent => {
                self.handle_print_preview_set_page_range_current();
                self.finish_update()
            }
            Message::PrintPreviewSetPageRangeSpecific => {
                self.handle_print_preview_set_page_range_specific();
                self.finish_update()
            }
            Message::PrintPreviewSetSpecificPageInput(value) => {
                self.handle_print_preview_set_specific_page_input(value);
                self.finish_update()
            }
            Message::PrintPreviewSetFitToPage(fit) => {
                self.handle_print_preview_set_fit_to_page(fit);
                self.finish_update()
            }
            Message::PrintPreviewSetIncludeTitleBlock(include) => {
                self.handle_print_preview_set_include_title_block(include);
                self.finish_update()
            }
            Message::PrintPreviewExport => {
                let task = self
                    .handle_print_preview_export()
                    .unwrap_or_else(iced::Task::none);
                iced::Task::batch([task, self.finish_update()])
            }
            Message::PrintPreviewClose => {
                let task = self.handle_print_preview_close();
                iced::Task::batch([task, self.finish_update()])
            }
            Message::ExportPdfOpenDialog => {
                let task = self.handle_export_pdf_open_dialog();
                iced::Task::batch([task, self.finish_update()])
            }
            Message::DismissExportError => {
                self.handle_dismiss_export_error();
                self.finish_update()
            }
            Message::ExportBomRequested => {
                let task = self.handle_bom_preview_open();
                iced::Task::batch([task, self.finish_update()])
            }
            Message::ExportBomFinished(result) => {
                let task = self.handle_export_bom_finished(result);
                iced::Task::batch([task, self.finish_update()])
            }
            Message::BomPreviewSetGrouping(g) => {
                self.handle_bom_preview_set_grouping(g);
                self.finish_update()
            }
            Message::BomPreviewSetFormat(f) => {
                self.handle_bom_preview_set_format(f);
                self.finish_update()
            }
            Message::BomPreviewSetIncludeDnp(b) => {
                self.handle_bom_preview_set_include_dnp(b);
                self.finish_update()
            }
            Message::BomPreviewSetIncludeNotFitted(b) => {
                self.handle_bom_preview_set_include_not_fitted(b);
                self.finish_update()
            }
            Message::BomPreviewToggleColumn(col) => {
                self.handle_bom_preview_toggle_column(col);
                self.finish_update()
            }
            Message::BomPreviewSetVariant(v) => {
                self.handle_bom_preview_set_variant(v);
                self.finish_update()
            }
            Message::BomPreviewExport => {
                let task = self
                    .handle_bom_preview_export()
                    .unwrap_or_else(iced::Task::none);
                iced::Task::batch([task, self.finish_update()])
            }
            Message::BomPreviewClose => {
                let task = self.handle_bom_preview_close();
                iced::Task::batch([task, self.finish_update()])
            }
            _ => unreachable!("dispatch_document_message received non-document message"),
        }
    }
}
